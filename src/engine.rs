//! Platform-independent engine core: owns the script runtime, UI, assets,
//! audio and timers, and turns input into script events. Windowed and
//! headless front ends drive it and render its draw lists.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use log::{debug, error, info, trace, warn};
use num_traits::{AsPrimitive, ToPrimitive};
use serde_json::{Value, json};

use crate::assets::Assets;
use crate::audio::Audio;
use crate::script::ScriptHost;
use crate::ui::desc::{AnimDesc, Color, Command, GameConfig, NodeDesc, PumpOutput};
use crate::ui::{DrawItem, InputEvent, Nav, Ui};
use crate::util::math::clamp_to_u32;

/// Upper bound on timers fired per tick, so a zero-delay timer loop in
/// script code cannot freeze the engine.
const MAX_TIMERS_PER_TICK: usize = 64;

/// Longest a new screen waits for its images to finish decoding.
const IMAGE_WAIT_LIMIT: Duration = Duration::from_millis(1500);

/// Width of save thumbnails in pixels.
const THUMBNAIL_WIDTH: u32 = 384;

#[derive(Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureTiming {
    /// Capture the frame currently on screen; hold back new trees until then.
    Now,
    /// Capture once the pending tree has been shown.
    AfterCommit,
    /// The tree is shown; capture when its transitions have finished.
    Settling(Instant),
}

/// Longest a capture waits for transitions to finish.
const SETTLE_LIMIT: Duration = Duration::from_secs(3);

/// Requests the front end must carry out (window state, quitting, redraws).
#[derive(Default)]
pub struct PlatformRequests {
    pub fullscreen: Option<bool>,
    pub title: Option<String>,
    pub quit: bool,
    pub redraw: bool,
    /// Render the current frame and pass it to `Engine::set_thumbnail`.
    pub capture: bool,
    /// Enable IME/text input while a text field is focused.
    pub text_input: Option<bool>,
}

/// A tree waiting for its images (or for a thumbnail capture) before it is shown.
struct PendingTree {
    tree: NodeDesc,
    instant: bool,
    exits: HashMap<String, Option<AnimDesc>>,
    since: Instant,
}

pub struct Engine {
    script: ScriptHost,
    pub assets: Assets,
    pub ui: Ui,
    audio: Option<Audio>,
    config: GameConfig,
    data_dir: PathBuf,
    timers: Vec<(Instant, u64)>,
    requests: PlatformRequests,
    pending: Option<PendingTree>,
    /// A thumbnail capture was requested and has not arrived yet.
    capture: Option<CaptureTiming>,
    thumbnail: Option<image::RgbaImage>,
    /// Thumbnails to write once the pending capture arrives.
    thumbnail_names: Vec<String>,
    text_input: bool,
}

impl Engine {
    pub fn new(script: ScriptHost, assets: Assets, ui: Ui, audio: Option<Audio>) -> Self {
        let mut engine = Self {
            script,
            assets,
            ui,
            audio,
            config: GameConfig {
                id: "deflorta-game".into(),
                title: "Deflorta".into(),
                width: 1280.0,
                height: 720.0,
                font: String::new(),
                version: None,
                clear_color: None,
            },
            data_dir: PathBuf::from("."),
            timers: Vec::new(),
            requests: PlatformRequests::default(),
            pending: None,
            capture: None,
            thumbnail: None,
            thumbnail_names: Vec::new(),
            text_input: false,
        };
        engine.pump();
        engine
    }

    pub const fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn take_requests(&mut self) -> PlatformRequests {
        let text_input = self.ui.focused_input().is_some();
        if text_input != self.text_input {
            self.text_input = text_input;
            self.requests.text_input = Some(text_input);
        }
        std::mem::take(&mut self.requests)
    }

    // -----------------------------------------------------------------------
    // Script bridge
    // -----------------------------------------------------------------------

    fn dispatch(&mut self, event: &Value) {
        if let Err(err) = self.script.dispatch(&event.to_string()) {
            error!("Script event failed: {err:#}");
        }
        self.pump();
    }

    fn pump(&mut self) {
        let json = match self.script.pump() {
            Ok(Some(json)) => json,
            Ok(None) => return,
            Err(err) => {
                error!("Reading script output failed: {err:#}");
                return;
            }
        };
        match serde_json::from_str::<PumpOutput>(&json) {
            Ok(output) => self.apply(output),
            Err(err) => error!("Invalid output from the script runtime: {err}"),
        }
    }

    fn apply(&mut self, output: PumpOutput) {
        let now = Instant::now();
        // Only the last configuration in a batch matters (the runtime's defaults
        // and the game's configure() usually arrive together).
        let last_config = output
            .cmds
            .iter()
            .rposition(|c| matches!(c, Command::Config { .. }));
        for (index, command) in output.cmds.into_iter().enumerate() {
            match command {
                Command::Config { config } if Some(index) == last_config => self.set_config(config),
                Command::Config { .. } => {}
                Command::Timer { id, ms } => self
                    .timers
                    .push((now + Duration::from_secs_f64(ms / 1000.0), id)),
                Command::CancelTimer { id } => self.timers.retain(|(_, t)| *t != id),
                Command::Music {
                    file,
                    r#loop,
                    volume,
                    fade_in,
                    fade_out,
                } => {
                    if let Some(audio) = &mut self.audio {
                        audio.play_music(
                            &self.assets,
                            file.as_deref(),
                            r#loop,
                            volume,
                            fade_in,
                            fade_out,
                        );
                    }
                }
                Command::Sound { file, volume } => {
                    if let Some(audio) = &mut self.audio {
                        audio.play_sound(&self.assets, &file, volume);
                    }
                }
                Command::Voice { file } => {
                    if let Some(audio) = &mut self.audio {
                        audio.play_voice(&self.assets, file.as_deref());
                    }
                }
                Command::Volume { channel, value } => {
                    if let Some(audio) = &mut self.audio {
                        audio.set_volume(&channel, value);
                    }
                }
                Command::RevealSkip => {
                    self.ui.reveal_skip(now);
                    self.requests.redraw = true;
                }
                Command::Preload { images } => {
                    debug!("Preloading {} images", images.len());
                    for src in &images {
                        self.assets.request(src);
                    }
                }
                Command::CaptureThumbnail { after } => self.request_capture(after),
                Command::SaveThumbnail { name } => {
                    if self.capture.is_some() {
                        self.thumbnail_names.push(name);
                    } else {
                        self.save_thumbnail(&name);
                    }
                }
                Command::DeleteThumbnail { name } => {
                    // A save waiting for its capture may be deleted before it arrives.
                    self.thumbnail_names.retain(|n| *n != name);
                    self.delete_thumbnail(&name);
                }
                Command::Fullscreen { on } => self.requests.fullscreen = Some(on),
                Command::Quit => {
                    info!("Game requested quit");
                    self.requests.quit = true;
                }
            }
        }
        if let Some(tree) = output.tree {
            self.stage_tree(tree, output.instant, output.exits, now);
        }
        match self.capture {
            // Nothing new to show: capture what is on screen.
            Some(CaptureTiming::AfterCommit) if self.pending.is_none() => {
                self.capture = Some(CaptureTiming::Now);
                self.requests.capture = true;
            }
            Some(CaptureTiming::Now) => self.requests.capture = true,
            _ => {}
        }
        self.try_commit();
    }

    fn request_capture(&mut self, after: bool) {
        debug!(
            "Thumbnail capture requested ({})",
            if after {
                "after the next screen"
            } else {
                "now"
            }
        );
        self.capture = Some(if after {
            CaptureTiming::AfterCommit
        } else {
            CaptureTiming::Now
        });
    }

    fn stage_tree(
        &mut self,
        tree: NodeDesc,
        instant: bool,
        exits: HashMap<String, Option<AnimDesc>>,
        now: Instant,
    ) {
        let mut images = Vec::new();
        Ui::collect_images(&tree, &mut images);
        for src in &images {
            self.assets.request(src);
        }
        // Successive trees replace each other; transitions of all of them still apply.
        let pending = match self.pending.take() {
            Some(mut previous) => {
                previous.tree = tree;
                previous.instant |= instant;
                previous.exits.extend(exits);
                previous
            }
            None => PendingTree {
                tree,
                instant,
                exits,
                since: now,
            },
        };
        self.pending = Some(pending);
    }

    /// Shows the pending tree once its images are decoded (or after a timeout),
    /// so transitions never start with missing pictures.
    fn try_commit(&mut self) {
        // The screen must stay as it is until the requested capture is taken.
        if self.capture == Some(CaptureTiming::Now) {
            return;
        }
        let Some(pending) = &self.pending else { return };
        let mut images = Vec::new();
        Ui::collect_images(&pending.tree, &mut images);
        self.assets.poll();
        let ready = images.iter().all(|src| self.assets.is_settled(src));
        if !ready && pending.since.elapsed() < IMAGE_WAIT_LIMIT {
            return;
        }
        if !ready {
            warn!(
                "Showing the next screen before its images finished loading (waited {:.1?})",
                pending.since.elapsed()
            );
        }
        let pending = self.pending.take().unwrap();
        trace!(
            "Committing UI tree (waited {:.0?} for images)",
            pending.since.elapsed()
        );
        self.ui.commit(
            pending.tree,
            pending.instant,
            &pending.exits,
            &self.assets,
            Instant::now(),
        );
        if let Some(audio) = &mut self.audio {
            audio.sync_videos(&self.assets, self.ui.video_sources());
        }
        self.requests.redraw = true;
        if self.capture == Some(CaptureTiming::AfterCommit) {
            self.capture = Some(CaptureTiming::Settling(Instant::now()));
        }
        self.check_settled();
    }

    /// Requests a deferred capture once transitions have played out.
    fn check_settled(&mut self) {
        let Some(CaptureTiming::Settling(since)) = self.capture else {
            return;
        };
        let now = Instant::now();
        if !self.ui.is_transitioning(now) || now.duration_since(since) >= SETTLE_LIMIT {
            self.capture = Some(CaptureTiming::Now);
            self.requests.capture = true;
        }
    }

    /// True while background work (image decoding) may change what is shown.
    pub const fn is_loading(&self) -> bool {
        self.pending.is_some() || matches!(self.capture, Some(CaptureTiming::Settling(_)))
    }

    /// Advances background work; call regularly while `is_loading`.
    pub fn poll(&mut self) {
        self.try_commit();
        self.check_settled();
    }

    fn set_config(&mut self, config: GameConfig) {
        self.ui
            .set_config(config.width, config.height, &config.font);
        let id: String = config
            .id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("deflorta")
            .join(id);
        ScriptHost::set_data_dir(self.data_dir.clone());
        self.assets.set_user_dir(self.data_dir.clone());
        self.requests.title = Some(config.title.clone());
        info!(
            "Game '{}' ({}), version {}, resolution {}x{}, font '{}'",
            config.title,
            config.id,
            config
                .version
                .as_ref()
                .map_or_else(|| "unset".to_owned(), ToString::to_string),
            config.width,
            config.height,
            config.font
        );
        self.config = config;
    }

    /// The front end delivers the frame captured for a `CaptureThumbnail` request.
    pub fn set_thumbnail(&mut self, image: Option<image::RgbaImage>) {
        // Keep only the game area, without letterbox bars.
        let area = self.ui.viewport();
        self.thumbnail = image.map(|img| {
            let x = clamp_to_u32(area.x, img.width().saturating_sub(1));
            let y = clamp_to_u32(area.y, img.height().saturating_sub(1));
            let w = clamp_to_u32(area.w, img.width() - x).max(1);
            let h = clamp_to_u32(area.h, img.height() - y).max(1);
            let game = image::imageops::crop_imm(&img, x, y, w, h).to_image();
            let height = (u64::from(h) * u64::from(THUMBNAIL_WIDTH) / u64::from(w))
                .to_u32()
                .unwrap_or(u32::MAX);
            image::imageops::thumbnail(&game, THUMBNAIL_WIDTH, height.max(1))
        });
        match &self.thumbnail {
            Some(t) => debug!("Captured {}x{} thumbnail", t.width(), t.height()),
            None => warn!("Thumbnail capture failed; saves keep their previous thumbnail"),
        }
        self.capture = None;
        for name in std::mem::take(&mut self.thumbnail_names) {
            self.save_thumbnail(&name);
        }
        self.try_commit();
    }

    /// `<data dir>/<name>.png`, or None for names that are not plain identifiers.
    fn thumbnail_path(&self, name: &str) -> Option<PathBuf> {
        let valid = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            warn!("Invalid thumbnail name '{name}'");
            return None;
        }
        Some(self.data_dir.join(format!("{name}.png")))
    }

    fn delete_thumbnail(&self, name: &str) {
        let Some(path) = self.thumbnail_path(name) else {
            return;
        };
        match std::fs::remove_file(&path) {
            Ok(()) => debug!("Deleted thumbnail {}", path.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => warn!("Cannot delete thumbnail {}: {err}", path.display()),
        }
    }

    fn save_thumbnail(&self, name: &str) {
        let Some(thumbnail) = self.thumbnail.as_ref() else {
            return;
        };
        let Some(path) = self.thumbnail_path(name) else {
            return;
        };
        let result = std::fs::create_dir_all(&self.data_dir)
            .map_err(anyhow::Error::from)
            .and_then(|()| {
                let tmp = path.with_extension("png.tmp");
                thumbnail.save_with_format(&tmp, image::ImageFormat::Png)?;
                std::fs::rename(&tmp, &path)?;
                Ok(())
            });
        match result {
            Ok(()) => debug!("Saved thumbnail {}", path.display()),
            Err(err) => warn!("Cannot save thumbnail '{name}': {err:#}"),
        }
    }

    // -----------------------------------------------------------------------
    // Input
    // -----------------------------------------------------------------------

    pub fn boot(&mut self) {
        info!("Booting the game");
        self.dispatch(&json!({ "type": "boot" }));
    }

    pub fn quit(&mut self) {
        info!("Shutting down the game");
        self.dispatch(&json!({ "type": "quit" }));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.ui.set_surface_size(width.as_(), height.as_());
        self.requests.redraw = true;
    }

    fn revealing(&self) -> bool {
        self.ui.is_revealing(Instant::now())
    }

    fn handle_ui_events(&mut self, events: Vec<InputEvent>, button: &str) {
        for event in events {
            let json = match event {
                InputEvent::Click { h } => {
                    json!({ "type": "click", "h": h, "button": button, "revealing": self.revealing() })
                }
                InputEvent::Change { h, value } => {
                    json!({ "type": "handler", "h": h, "value": value })
                }
                InputEvent::Input { h, value } | InputEvent::Submit { h, value } => {
                    json!({ "type": "handler", "h": h, "value": value })
                }
                InputEvent::Tooltip(text) => json!({ "type": "tooltip", "text": text }),
            };
            self.dispatch(&json);
            self.requests.redraw = true;
        }
    }

    /// Pointer position in physical pixels; `None` when it left the window.
    pub fn pointer_moved(&mut self, position: Option<(f32, f32)>) {
        let (redraw, events) = self.ui.pointer_moved(position);
        self.requests.redraw |= redraw;
        self.handle_ui_events(events, "left");
    }

    pub fn mouse_down(&mut self, button: &str) {
        // Only the primary button drags sliders and focuses fields; others just
        // report the element clicked (e.g. right click opens the game menu).
        let events = if button == "left" {
            self.ui.mouse_down()
        } else {
            self.ui
                .click_target()
                .map(|h| InputEvent::Click { h })
                .into_iter()
                .collect()
        };
        if events.is_empty() {
            let revealing = self.revealing();
            self.dispatch(
                &json!({ "type": "click", "h": null, "button": button, "revealing": revealing }),
            );
        } else {
            self.handle_ui_events(events, button);
        }
        self.requests.redraw = true;
    }

    pub fn mouse_up(&mut self) {
        self.ui.mouse_up();
    }

    /// Wheel movement in lines; positive scrolls down.
    pub fn wheel(&mut self, dy: f32) {
        if self.ui.scroll_at(dy) {
            self.requests.redraw = true;
            return;
        }
        let revealing = self.revealing();
        self.dispatch(&json!({ "type": "wheel", "dy": dy, "revealing": revealing }));
    }

    /// Typed text (after IME composition) for the focused text field.
    pub fn text_input(&mut self, text: &str) {
        if let Some(event) = self.ui.type_text(text) {
            self.handle_ui_events(vec![event], "left");
        }
    }

    pub fn key(&mut self, key: &str, down: bool, repeat: bool, modifiers: &KeyModifiers) {
        if down {
            // A focused text field consumes editing keys.
            if self.ui.focused_input().is_some() {
                let event = match key {
                    "Backspace" => self.ui.backspace(),
                    "Enter" => self.ui.activate(),
                    "Escape" => {
                        self.ui.clear_focus();
                        None
                    }
                    _ => None,
                };
                self.handle_ui_events(event.into_iter().collect(), "left");
                self.requests.redraw = true;
                return;
            }
            let nav = match key {
                "ArrowUp" => Some(Nav::Up),
                "ArrowDown" => Some(Nav::Down),
                "ArrowLeft" => Some(Nav::Left),
                "ArrowRight" => Some(Nav::Right),
                _ => None,
            };
            if let Some(nav) = nav {
                let (consumed, events) = self.ui.navigate(nav);
                if consumed {
                    self.requests.redraw = true;
                    self.handle_ui_events(events, "left");
                    return;
                }
            }
            if matches!(key, "Enter" | " ") && self.ui.has_focus() {
                let event = self.ui.activate();
                self.handle_ui_events(event.into_iter().collect(), "left");
                return;
            }
        }
        let revealing = self.revealing();
        self.dispatch(&json!({
            "type": "key",
            "key": key,
            "down": down,
            "repeat": repeat,
            "ctrl": modifiers.ctrl,
            "shift": modifiers.shift,
            "alt": modifiers.alt,
            "revealing": revealing,
        }));
    }

    // -----------------------------------------------------------------------
    // Time
    // -----------------------------------------------------------------------

    pub fn fire_timers(&mut self) {
        for _ in 0..MAX_TIMERS_PER_TICK {
            let now = Instant::now();
            let due = self
                .timers
                .iter()
                .enumerate()
                .filter(|(_, (due, _))| *due <= now)
                .min_by_key(|(_, (due, _))| *due)
                .map(|(i, _)| i);
            let Some(pos) = due else { break };
            let (_, id) = self.timers.swap_remove(pos);
            self.dispatch(&json!({ "type": "timer", "id": id }));
        }
    }

    pub fn next_timer(&self) -> Option<Instant> {
        self.timers.iter().map(|(due, _)| *due).min()
    }

    /// Lets the JS engine collect garbage while idle.
    pub fn idle(&mut self) {
        self.script.maybe_gc();
    }

    /// Builds the draw list for a frame.
    pub fn frame(&mut self, now: Instant) -> Vec<DrawItem> {
        self.ui.draw(&mut self.assets, now)
    }

    pub fn clear_color(&self) -> Color {
        self.config
            .clear_color
            .unwrap_or(Color([0.0, 0.0, 0.0, 1.0]))
    }

    /// Call after presenting a frame. Returns true if another frame is needed.
    pub fn after_frame(&mut self, now: Instant) -> bool {
        let (hover_changed, events) = self.ui.refresh_hover();
        self.handle_ui_events(events, "left");
        let animating = self.ui.is_animating(now);
        self.ui.prune(now);
        if self.ui.take_revealed_event(now) {
            self.dispatch(&json!({ "type": "revealed" }));
        }
        for h in self.ui.take_ended_videos() {
            self.dispatch(&json!({ "type": "handler", "h": h }));
        }
        hover_changed || animating || self.assets.has_pending()
    }
}
