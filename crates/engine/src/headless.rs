//! Headless front end: plays a scripted sequence of inputs against a game and
//! saves screenshots, for automated tests and documentation.
//!
//! The test script is a JSON array of steps:
//!
//! ```json
//! [
//!   { "wait": 500 },
//!   { "move": [640, 360] },
//!   { "click": [640, 360] },
//!   { "click": [640, 360], "button": "right" },
//!   { "release": true },
//!   { "key": "Enter" },
//!   { "key": "Control", "down": true },
//!   { "type": "Alice" },
//!   { "wheel": -1 },
//!   { "shot": "out/title.png" }
//! ]
//! ```
//!
//! Coordinates are in the game's virtual resolution.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use log::{error, info, trace};
use num_traits::ToPrimitive;
use serde::Deserialize;

use crate::engine::{Engine, KeyModifiers};
use crate::render::Renderer;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Milliseconds to keep running (timers and animations advance in real time).
    wait: Option<u64>,
    /// Moves the pointer without clicking.
    r#move: Option<[f32; 2]>,
    click: Option<[f32; 2]>,
    button: Option<String>,
    /// Releases the mouse button (ends slider drags).
    release: Option<bool>,
    /// Presses and releases a key, or only one edge when `down` is given.
    key: Option<String>,
    down: Option<bool>,
    /// Types text into the focused text field.
    r#type: Option<String>,
    wheel: Option<f32>,
    shot: Option<PathBuf>,
}

const FRAME: Duration = Duration::from_millis(16);

pub fn run(mut engine: Engine, script: &Path) -> Result<()> {
    let text = std::fs::read_to_string(script)
        .with_context(|| format!("cannot read {}", script.display()))?;
    let steps: Vec<Step> = serde_json::from_str(&text)
        .with_context(|| format!("invalid test script {}", script.display()))?;
    info!(
        "Running test script {} ({} steps)",
        script.display(),
        steps.len()
    );
    let started = Instant::now();
    let width = engine
        .config()
        .width
        .to_u32()
        .filter(|&w| w > 0)
        .context("game width must fit in a positive u32 pixel count")?;
    let height = engine
        .config()
        .height
        .to_u32()
        .filter(|&h| h > 0)
        .context("game height must fit in a positive u32 pixel count")?;
    let mut renderer = pollster::block_on(Renderer::offscreen(width, height))?;
    engine.resize(width, height);
    engine.boot();
    tick(&mut engine, &mut renderer);

    for (index, step) in steps.into_iter().enumerate() {
        trace!("Step {}", index + 1);
        if let Some(ms) = step.wait {
            let until = Instant::now() + Duration::from_millis(ms);
            while Instant::now() < until {
                tick(&mut engine, &mut renderer);
                std::thread::sleep(FRAME.min(until.saturating_duration_since(Instant::now())));
            }
        }
        if let Some(pos) = step.r#move {
            engine.pointer_moved(Some(pos.into()));
        }
        if let Some(pos) = step.click {
            engine.pointer_moved(Some(pos.into()));
            tick(&mut engine, &mut renderer);
            engine.mouse_down(step.button.as_deref().unwrap_or("left"));
            if step.release != Some(false) {
                engine.mouse_up();
            }
        } else if step.release == Some(true) {
            engine.mouse_up();
        }
        if let Some(key) = &step.key {
            if let Some(down) = step.down {
                engine.key(key, down, false, &KeyModifiers::default());
            } else {
                engine.key(key, true, false, &KeyModifiers::default());
                engine.key(key, false, false, &KeyModifiers::default());
            }
        }
        if let Some(text) = &step.r#type {
            engine.text_input(text);
        }
        if let Some(dy) = step.wheel {
            engine.wheel(dy);
        }
        tick(&mut engine, &mut renderer);
        if let Some(path) = &step.shot {
            // Let pending images finish decoding so screenshots are complete.
            let deadline = Instant::now() + Duration::from_secs(2);
            while engine.is_loading() && Instant::now() < deadline {
                tick(&mut engine, &mut renderer);
                std::thread::sleep(Duration::from_millis(5));
            }
            let now = Instant::now();
            let items = engine.frame(now);
            let clear = engine.clear_color();
            renderer.render(
                items,
                &mut engine.ui,
                &mut engine.assets,
                clear,
                #[cfg(feature = "dev-console")]
                None,
            )?;
            engine.after_frame(now);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            renderer
                .capture()?
                .save(path)
                .with_context(|| format!("cannot save {}", path.display()))?;
            info!("Saved screenshot {}", path.display());
        }
        if handle_requests(&mut engine, &mut renderer) {
            info!("The game quit at step {}", index + 1);
            break;
        }
    }
    engine.quit();
    handle_requests(&mut engine, &mut renderer);
    info!("Test script finished in {:.1?}", started.elapsed());
    Ok(())
}

/// Serves engine requests; returns true when the game asked to quit.
fn handle_requests(engine: &mut Engine, renderer: &mut Renderer) -> bool {
    let requests = engine.take_requests();
    if requests.capture {
        let items = engine.frame(Instant::now());
        let clear = engine.clear_color();
        let image = renderer
            .render_to_image(items, &mut engine.ui, &mut engine.assets, clear)
            .map_err(|e| error!("Thumbnail capture failed: {e:#}"))
            .ok();
        engine.set_thumbnail(image);
    }
    requests.quit
}

/// Advances timers, background loading and a frame so hit testing and events stay current.
fn tick(engine: &mut Engine, renderer: &mut Renderer) {
    engine.fire_timers();
    engine.poll();
    handle_requests(engine, renderer);
    let now = Instant::now();
    engine.frame(now);
    engine.after_frame(now);
}
