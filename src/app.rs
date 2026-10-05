//! Windowed front end (winit). The loop sleeps until input or a timer arrives
//! and only redraws continuously while something is animating.

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::engine::Engine;
use crate::render::Renderer;

pub struct App {
    engine: Engine,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    modifiers: ModifiersState,
    fullscreen: bool,
    booted: bool,
    error: Option<anyhow::Error>,
}

impl App {
    pub fn new(engine: Engine) -> Self {
        App {
            engine,
            renderer: None,
            window: None,
            modifiers: ModifiersState::empty(),
            fullscreen: false,
            booted: false,
            error: None,
        }
    }

    /// The error that stopped the event loop, if any.
    pub fn take_error(&mut self) -> Option<anyhow::Error> {
        self.error.take()
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let config = self.engine.config();
        let attributes = Window::default_attributes()
            .with_title(&config.title)
            .with_inner_size(LogicalSize::new(config.width as f64, config.height as f64))
            .with_min_inner_size(LogicalSize::new(320.0, 180.0))
            .with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None)));
        let window = Arc::new(event_loop.create_window(attributes)?);
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

    /// Carries out window changes and redraw/quit requests made by the engine.
    fn handle_requests(&mut self, event_loop: &ActiveEventLoop) {
        let requests = self.engine.take_requests();
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
            window.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
        }
        if requests.redraw {
            window.request_redraw();
        }
        if requests.quit {
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
        if let Err(err) = renderer.render(items, &mut self.engine.ui, &self.engine.assets, clear) {
            eprintln!("[deflorta] render failed: {err:#}");
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
        if let Err(err) = self.create_window(event_loop) {
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
                self.engine.quit();
                event_loop.exit();
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
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                let button = match button {
                    MouseButton::Left => "left",
                    MouseButton::Right => "right",
                    MouseButton::Middle => "middle",
                    _ => return,
                };
                self.engine.mouse_down(button);
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
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(key) = key_name(&event.logical_key) {
                    let m = self.modifiers;
                    let down = event.state == ElementState::Pressed;
                    self.engine.key(
                        &key,
                        down,
                        event.repeat,
                        m.control_key(),
                        m.shift_key(),
                        m.alt_key(),
                    );
                }
            }
            _ => {}
        }
        self.handle_requests(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.engine.fire_timers();
        self.handle_requests(event_loop);
        self.engine.idle();
        match self.engine.next_timer() {
            Some(due) => event_loop.set_control_flow(ControlFlow::WaitUntil(due)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
