//! Platform-independent engine core: owns the script runtime, UI, audio and
//! timers, and turns input into script events. Windowed and headless front
//! ends drive it and render its draw lists.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::assets::Assets;
use crate::audio::Audio;
use crate::script::ScriptHost;
use crate::ui::desc::{Color, Command, GameConfig, PumpOutput};
use crate::ui::{DrawItem, Ui};

/// Upper bound on timers fired per tick, so a zero-delay timer loop in
/// script code cannot freeze the engine.
const MAX_TIMERS_PER_TICK: usize = 64;

/// Requests the front end must carry out (window state, quitting, redraws).
#[derive(Default)]
pub struct PlatformRequests {
    pub fullscreen: Option<bool>,
    pub title: Option<String>,
    pub quit: bool,
    pub redraw: bool,
}

pub struct Engine {
    script: ScriptHost,
    pub assets: Assets,
    pub ui: Ui,
    audio: Option<Audio>,
    config: GameConfig,
    timers: Vec<(Instant, u64)>,
    cursor: Option<(f32, f32)>,
    requests: PlatformRequests,
}

impl Engine {
    pub fn new(script: ScriptHost, assets: Assets, ui: Ui, audio: Option<Audio>) -> Self {
        let mut engine = Engine {
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
                clear_color: None,
            },
            timers: Vec::new(),
            cursor: None,
            requests: PlatformRequests::default(),
        };
        engine.pump();
        engine
    }

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn take_requests(&mut self) -> PlatformRequests {
        std::mem::take(&mut self.requests)
    }

    // -----------------------------------------------------------------------
    // Script bridge
    // -----------------------------------------------------------------------

    fn dispatch(&mut self, event: serde_json::Value) {
        if let Err(err) = self.script.dispatch(&event.to_string()) {
            eprintln!("[deflorta] dispatch failed: {err:#}");
        }
        self.pump();
    }

    fn pump(&mut self) {
        let json = match self.script.pump() {
            Ok(Some(json)) => json,
            Ok(None) => return,
            Err(err) => {
                eprintln!("[deflorta] pump failed: {err:#}");
                return;
            }
        };
        match serde_json::from_str::<PumpOutput>(&json) {
            Ok(output) => self.apply(output),
            Err(err) => eprintln!("[deflorta] invalid output from script runtime: {err}"),
        }
    }

    fn apply(&mut self, output: PumpOutput) {
        let now = Instant::now();
        for command in output.cmds {
            match command {
                Command::Config { config } => self.set_config(config),
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
                Command::Volume { channel, value } => {
                    if let Some(audio) = &mut self.audio {
                        audio.set_volume(&channel, value);
                    }
                }
                Command::RevealAll => {
                    self.ui.reveal_all();
                    self.requests.redraw = true;
                }
                Command::Fullscreen { on } => self.requests.fullscreen = Some(on),
                Command::Quit => self.requests.quit = true,
            }
        }
        if let Some(tree) = output.tree {
            self.ui.commit(tree, output.instant, &output.exits, now);
            self.requests.redraw = true;
        }
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
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("deflorta")
            .join(id);
        self.script.set_data_dir(data_dir);
        self.requests.title = Some(config.title.clone());
        self.config = config;
    }

    // -----------------------------------------------------------------------
    // Input
    // -----------------------------------------------------------------------

    pub fn boot(&mut self) {
        self.dispatch(json!({ "type": "boot" }));
    }

    pub fn quit(&mut self) {
        self.dispatch(json!({ "type": "quit" }));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.ui.set_surface_size(width as f32, height as f32);
        self.requests.redraw = true;
    }

    fn revealing(&self) -> bool {
        self.ui.is_revealing(Instant::now())
    }

    /// Pointer position in physical pixels; `None` when it left the window.
    pub fn pointer_moved(&mut self, position: Option<(f32, f32)>) {
        self.cursor = position;
        let (x, y) = position.unwrap_or((f32::NEG_INFINITY, f32::NEG_INFINITY));
        if self.ui.pointer_moved(x, y) {
            self.requests.redraw = true;
        }
    }

    pub fn mouse_down(&mut self, button: &str) {
        let Some((x, y)) = self.cursor else { return };
        let handler = self.ui.click_target(x, y);
        let revealing = self.revealing();
        self.dispatch(
            json!({ "type": "click", "h": handler, "button": button, "revealing": revealing }),
        );
    }

    /// Wheel movement in lines; positive scrolls down.
    pub fn wheel(&mut self, dy: f32) {
        let revealing = self.revealing();
        self.dispatch(json!({ "type": "wheel", "dy": dy, "revealing": revealing }));
    }

    pub fn key(&mut self, key: &str, down: bool, repeat: bool, ctrl: bool, shift: bool, alt: bool) {
        let revealing = self.revealing();
        self.dispatch(json!({
            "type": "key",
            "key": key,
            "down": down,
            "repeat": repeat,
            "ctrl": ctrl,
            "shift": shift,
            "alt": alt,
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
            self.dispatch(json!({ "type": "timer", "id": id }));
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
        let hover_changed = self.ui.refresh_hover();
        let animating = self.ui.is_animating(now);
        self.ui.prune(now);
        if self.ui.take_revealed_event(now) {
            self.dispatch(json!({ "type": "revealed" }));
        }
        hover_changed || animating
    }
}
