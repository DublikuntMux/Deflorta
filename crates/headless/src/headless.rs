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

///
/// # Errors
///
/// Returns an error for unreadable or invalid test scripts, invalid game dimensions, GPU initialization failures, or screenshot write failures.
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
    let quit_at_boot = tick(&mut engine, &mut renderer);

    'steps: for (index, step) in steps.into_iter().enumerate() {
        if quit_at_boot {
            break;
        }
        trace!("Step {}", index + 1);
        if let Some(ms) = step.wait {
            let until = Instant::now() + Duration::from_millis(ms);
            while Instant::now() < until {
                if tick(&mut engine, &mut renderer) {
                    break 'steps;
                }
                std::thread::sleep(FRAME.min(until.saturating_duration_since(Instant::now())));
            }
        }
        if let Some(pos) = step.r#move {
            engine.pointer_moved(Some(pos.into()));
        }
        if let Some(pos) = step.click {
            engine.pointer_moved(Some(pos.into()));
            if tick(&mut engine, &mut renderer) {
                break;
            }
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
        if tick(&mut engine, &mut renderer) {
            break;
        }
        if let Some(path) = &step.shot {
            // Let pending images finish decoding so screenshots are complete.
            let deadline = Instant::now() + Duration::from_secs(2);
            while engine.is_loading() && Instant::now() < deadline {
                if tick(&mut engine, &mut renderer) {
                    break 'steps;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            if save_screenshot(&mut engine, &mut renderer, path)? {
                break;
            }
        }
        if handle_requests(&mut engine, &mut renderer) {
            info!("The game quit at step {}", index + 1);
            break;
        }
    }
    engine.quit();
    handle_requests(&mut engine, &mut renderer);
    engine.flush_storage();
    info!("Test script finished in {:.1?}", started.elapsed());
    Ok(())
}

/// Draws and saves a screenshot, returning true if frame events asked to quit.
fn save_screenshot(engine: &mut Engine, renderer: &mut Renderer, path: &Path) -> Result<bool> {
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
    if handle_requests(engine, renderer) {
        return Ok(true);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    renderer
        .capture()?
        .save(path)
        .with_context(|| format!("cannot save {}", path.display()))?;
    info!("Saved screenshot {}", path.display());
    Ok(false)
}

/// Serves engine requests; returns true when the game asked to quit.
fn handle_requests(engine: &mut Engine, renderer: &mut Renderer) -> bool {
    let requests = engine.take_requests();
    for source in &requests.unload_textures {
        renderer.unload_texture(source);
    }
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
fn tick(engine: &mut Engine, renderer: &mut Renderer) -> bool {
    engine.fire_timers();
    engine.poll();
    engine.collect_unused_resources(renderer, Instant::now());
    if handle_requests(engine, renderer) {
        return true;
    }
    let now = Instant::now();
    engine.frame(now);
    engine.after_frame(now);
    handle_requests(engine, renderer)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::script::ScriptHost;

    #[test]
    #[ignore = "requires an offscreen GPU"]
    fn quit_at_boot_during_wait_and_after_input_stops_headless_steps() {
        let name = "headless::tests::quit_at_boot_during_wait_and_after_input_stops_headless_steps";
        if std::env::var_os("DEFLORTA_HEADLESS_TEST_MODE").is_none() {
            let temp =
                std::env::temp_dir().join(format!("deflorta-headless-{}", std::process::id()));
            for mode in ["boot", "wait", "input"] {
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", name, "--ignored"])
                    .env("DEFLORTA_HEADLESS_TEST_MODE", mode)
                    .env("XDG_DATA_HOME", temp.join(mode))
                    .status()
                    .unwrap();
                assert!(status.success(), "{mode}");
            }
            let _ = std::fs::remove_dir_all(temp);
            return;
        }
        let mode = std::env::var("DEFLORTA_HEADLESS_TEST_MODE").unwrap();
        let temp = deflorta_common::data_dir();
        let game = temp.join("game");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::copy(
            crate::workspace_dir().join("tests/headless-regressions/main.js"),
            game.join("main.js"),
        )
        .unwrap();
        std::fs::write(game.join("mode.txt"), &mode).unwrap();
        let script = temp.join("steps.json");
        let shot = temp.join("after-quit.png");
        let mut steps = Vec::new();
        if mode == "input" {
            steps.push(serde_json::json!({"key": "q"}));
        }
        steps.push(serde_json::json!({"wait": 1000, "shot": shot}));
        std::fs::write(&script, serde_json::to_vec(&steps).unwrap()).unwrap();
        let files = crate::GameFiles::open(&game).unwrap();
        let mut host = ScriptHost::new(files.clone()).unwrap();
        host.run_main().unwrap();
        let ui = crate::ui::Ui::new(crate::ui::text::TextSystem::new(&files));
        let engine = Engine::new(host, crate::assets::Assets::new(files), ui, None);
        run(engine, &script).unwrap();
        let data = temp.join("deflorta/headless-regressions");
        assert_eq!(
            std::fs::read_to_string(data.join("quit-count.json")).unwrap(),
            "1"
        );
        assert!(!data.join("after-quit.json").exists());
        assert!(!shot.exists());
    }
}
