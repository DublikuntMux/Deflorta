//! Windowed front end (winit + gilrs). The loop sleeps until input or a timer
//! arrives and only redraws continuously while something is animating.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gilrs::{Axis, Button, EventType, Gilrs};
use log::{debug, error, info, warn};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::engine::{Engine, KeyModifiers};
use crate::render::Renderer;

/// Polling interval while gamepads are connected or images are decoding.
const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Stick deflection that counts as a direction press.
const STICK_THRESHOLD: f32 = 0.6;

pub struct App {
    engine: Engine,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    gamepads: Option<Gilrs>,
    /// Last stick direction per axis, to turn deflection into key presses.
    stick: [i8; 2],
    modifiers: ModifiersState,
    fullscreen: bool,
    booted: bool,
    error: Option<anyhow::Error>,
}

impl App {
    pub fn new(engine: Engine) -> Self {
        let gamepads = Gilrs::new()
            .map_err(|e| warn!("Gamepads unavailable: {e}"))
            .ok();
        if let Some(gamepads) = &gamepads {
            let connected: Vec<_> = gamepads
                .gamepads()
                .map(|(_, g)| g.name().to_owned())
                .collect();
            info!(
                "Gamepad support ready, {} connected {:?}",
                connected.len(),
                connected
            );
        }
        Self {
            engine,
            renderer: None,
            window: None,
            gamepads,
            stick: [0; 2],
            modifiers: ModifiersState::empty(),
            fullscreen: false,
            booted: false,
            error: None,
        }
    }

    /// The error that stopped the event loop, if any.
    pub const fn take_error(&mut self) -> Option<anyhow::Error> {
        self.error.take()
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let config = self.engine.config();
        let attributes = Window::default_attributes()
            .with_title(&config.title)
            .with_inner_size(LogicalSize::new(
                f64::from(config.width),
                f64::from(config.height),
            ))
            .with_min_inner_size(LogicalSize::new(320.0, 180.0))
            .with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None)));
        let window = Arc::new(event_loop.create_window(attributes)?);
        let size = window.inner_size();
        info!(
            "Window created: {}x{} physical, scale factor {:.2}, monitor {:?}",
            size.width,
            size.height,
            window.scale_factor(),
            window.current_monitor().and_then(|m| m.name())
        );
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(event_loop.owned_display_handle()),
        ));
        let renderer = pollster::block_on(Renderer::for_window(&instance, window.clone()))?;
        let (w, h) = renderer.size();
        self.engine.resize(w, h);
        self.renderer = Some(renderer);
        self.window = Some(window);
        Ok(())
    }

    /// Carries out window changes, captures and redraw/quit requests made by the engine.
    fn handle_requests(&mut self, event_loop: &ActiveEventLoop) {
        let requests = self.engine.take_requests();
        if requests.capture {
            let image = self.renderer.as_mut().and_then(|renderer| {
                let items = self.engine.frame(Instant::now());
                let clear = self.engine.clear_color();
                renderer
                    .render_to_image(items, &mut self.engine.ui, &mut self.engine.assets, clear)
                    .map_err(|e| error!("Thumbnail capture failed: {e:#}"))
                    .ok()
            });
            self.engine.set_thumbnail(image);
        }
        if let Some(on) = requests.fullscreen {
            self.fullscreen = on;
        }
        let Some(window) = &self.window else { return };
        if let Some(title) = requests.title {
            window.set_title(&title);
        }
        if let Some(on) = requests.fullscreen
            && on != window.fullscreen().is_some()
        {
            info!("{} fullscreen", if on { "Entering" } else { "Leaving" });
            window.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
        }
        if let Some(on) = requests.text_input {
            window.set_ime_allowed(on);
        }
        if requests.redraw || requests.capture {
            window.request_redraw();
        }
        if requests.quit {
            info!("Closing the window");
            event_loop.exit();
        }
    }

    fn redraw(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let now = Instant::now();
        let items = self.engine.frame(now);
        let clear = self.engine.clear_color();
        let size = renderer.size();
        if let Err(err) =
            renderer.render(items, &mut self.engine.ui, &mut self.engine.assets, clear)
        {
            error!("Render failed: {err:#}");
        }
        // The renderer adopts the window's real size when the surface was suboptimal.
        let resized = renderer.size() != size;
        if resized {
            let (w, h) = renderer.size();
            self.engine.resize(w, h);
        }
        if (self.engine.after_frame(now) || resized)
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }

    fn press(&mut self, key: &str, down: bool) {
        self.engine.key(key, down, false, &KeyModifiers::default());
    }

    /// Maps gamepad input to the keyboard bindings.
    fn poll_gamepads(&mut self) {
        let Some(gamepads) = &mut self.gamepads else {
            return;
        };
        let mut presses: Vec<(&'static str, bool)> = Vec::new();
        while let Some(event) = gamepads.next_event() {
            let (button, down) = match event.event {
                EventType::Connected => {
                    info!("Gamepad connected: {}", gamepads.gamepad(event.id).name());
                    continue;
                }
                EventType::Disconnected => {
                    info!(
                        "Gamepad disconnected: {}",
                        gamepads.gamepad(event.id).name()
                    );
                    continue;
                }
                EventType::ButtonPressed(b, _) => (b, true),
                EventType::ButtonReleased(b, _) => (b, false),
                EventType::AxisChanged(axis, value, _) => {
                    let (slot, negative, positive) = match axis {
                        Axis::LeftStickX => (0, "ArrowLeft", "ArrowRight"),
                        // Stick up is positive.
                        Axis::LeftStickY => (1, "ArrowDown", "ArrowUp"),
                        _ => continue,
                    };
                    let direction = if value > STICK_THRESHOLD {
                        1
                    } else if value < -STICK_THRESHOLD {
                        -1
                    } else {
                        0
                    };
                    if direction != self.stick[slot] && direction != 0 {
                        let key = if direction > 0 { positive } else { negative };
                        presses.push((key, true));
                        presses.push((key, false));
                    }
                    self.stick[slot] = direction;
                    continue;
                }
                _ => continue,
            };
            let key = match button {
                Button::DPadUp => "ArrowUp",
                Button::DPadDown => "ArrowDown",
                Button::DPadLeft => "ArrowLeft",
                Button::DPadRight => "ArrowRight",
                Button::South => "Enter",
                Button::East | Button::Start => "Escape",
                Button::North => "h",
                Button::LeftTrigger => "PageUp",
                Button::RightTrigger2 => "Control",
                _ => continue,
            };
            presses.push((key, down));
        }
        for (key, down) in presses {
            self.press(key, down);
        }
    }

    fn has_gamepads(&self) -> bool {
        self.gamepads
            .as_ref()
            .is_some_and(|g| g.gamepads().next().is_some())
    }
}

fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Named(NamedKey::Space) => Some(" ".into()),
        Key::Named(named) => Some(format!("{named:?}")),
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        info!("Application resumed");
        if let Err(err) = self.create_window(event_loop) {
            error!("Cannot create the window: {err:#}");
            self.error = Some(err);
            event_loop.exit();
            return;
        }
        if !self.booted {
            self.booted = true;
            self.engine.boot();
        }
        self.handle_requests(event_loop);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                info!("Window close requested");
                self.engine.quit();
                event_loop.exit();
            }
            WindowEvent::Focused(focused) => {
                debug!("Window {}", if focused { "focused" } else { "unfocused" });
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                info!("Scale factor changed to {scale_factor:.2}");
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                self.engine.resize(size.width, size.height);
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::CursorMoved { position, .. } => {
                self.engine
                    .pointer_moved(Some((position.x as f32, position.y as f32)));
            }
            WindowEvent::CursorLeft { .. } => self.engine.pointer_moved(None),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => "left",
                    MouseButton::Right => "right",
                    MouseButton::Middle => "middle",
                    _ => return,
                };
                match state {
                    ElementState::Pressed => self.engine.mouse_down(button),
                    ElementState::Released => self.engine.mouse_up(),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 40.0,
                };
                if dy != 0.0 {
                    self.engine.wheel(dy);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::Ime(Ime::Commit(text)) => self.engine.text_input(&text),
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let Some(key) = key_name(&event.logical_key) {
                    let m = self.modifiers;
                    self.engine.key(
                        &key,
                        down,
                        event.repeat,
                        &KeyModifiers {
                            ctrl: m.control_key(),
                            shift: m.shift_key(),
                            alt: m.alt_key(),
                        },
                    );
                }
                if down
                    && !self.modifiers.control_key()
                    && let Some(text) = &event.text
                {
                    self.engine.text_input(text);
                }
            }
            _ => {}
        }
        self.handle_requests(event_loop);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Application suspended");
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Event loop exiting");
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.poll_gamepads();
        self.engine.fire_timers();
        self.engine.poll();
        self.handle_requests(event_loop);
        self.engine.idle();
        let polling = (self.engine.is_loading() || self.has_gamepads())
            .then(|| Instant::now() + POLL_INTERVAL);
        match [self.engine.next_timer(), polling]
            .into_iter()
            .flatten()
            .min()
        {
            Some(due) => event_loop.set_control_flow(ControlFlow::WaitUntil(due)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
