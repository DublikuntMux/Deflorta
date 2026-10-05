//! Headless front end: plays a scripted sequence of inputs against a game and
//! saves screenshots, for automated tests and documentation.
//!
//! The test script is a JSON array of steps:
//!
//! ```json
//! [
//!   { "wait": 500 },
//!   { "click": [640, 360] },
//!   { "click": [640, 360], "button": "right" },
//!   { "key": "Enter" },
//!   { "key": "Control", "down": true },
//!   { "wheel": -1 },
//!   { "shot": "out/title.png" }
//! ]
//! ```
//!
//! Coordinates are in the game's virtual resolution.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::engine::Engine;
use crate::render::Renderer;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Milliseconds to keep running (timers and animations advance in real time).
    wait: Option<u64>,
    click: Option<[f32; 2]>,
    button: Option<String>,
    /// Presses and releases a key, or only one edge when `down` is given.
    key: Option<String>,
    down: Option<bool>,
    wheel: Option<f32>,
    shot: Option<PathBuf>,
}

const FRAME: Duration = Duration::from_millis(16);

pub fn run(mut engine: Engine, script: &Path) -> Result<()> {
    let text = std::fs::read_to_string(script)
        .with_context(|| format!("cannot read {}", script.display()))?;
    let steps: Vec<Step> = serde_json::from_str(&text)
        .with_context(|| format!("invalid test script {}", script.display()))?;
    let (width, height) = (engine.config().width as u32, engine.config().height as u32);
    let mut renderer = pollster::block_on(Renderer::offscreen(width, height))?;
    engine.resize(width, height);
    engine.boot();
    tick(&mut engine);

    for step in steps {
        if let Some(ms) = step.wait {
            let until = Instant::now() + Duration::from_millis(ms);
            while Instant::now() < until {
                tick(&mut engine);
                std::thread::sleep(FRAME.min(until.saturating_duration_since(Instant::now())));
            }
        }
        if let Some([x, y]) = step.click {
            engine.pointer_moved(Some((x, y)));
            tick(&mut engine);
            engine.mouse_down(step.button.as_deref().unwrap_or("left"));
        }
        if let Some(key) = &step.key {
            match step.down {
                Some(down) => engine.key(key, down, false, false, false, false),
                None => {
                    engine.key(key, true, false, false, false, false);
                    engine.key(key, false, false, false, false, false);
                }
            }
        }
        if let Some(dy) = step.wheel {
            engine.wheel(dy);
        }
        tick(&mut engine);
        if let Some(path) = &step.shot {
            let now = Instant::now();
            let items = engine.frame(now);
            let clear = engine.clear_color();
            renderer.render(items, &mut engine.ui, &engine.assets, clear)?;
            engine.after_frame(now);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            renderer
                .capture()?
                .save(path)
                .with_context(|| format!("cannot save {}", path.display()))?;
            println!("saved {}", path.display());
        }
        if engine.take_requests().quit {
            break;
        }
    }
    engine.quit();
    Ok(())
}

/// Advances timers and builds a frame so hit testing and reveal events stay current.
fn tick(engine: &mut Engine) {
    engine.fire_timers();
    let now = Instant::now();
    engine.frame(now);
    engine.after_frame(now);
}
