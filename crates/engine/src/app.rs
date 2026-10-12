use std::sync::Arc;
use std::time::Instant;

use deflorta_common::notification::{
    NotificationId, NotificationOptions, NotificationState, next_notification_id,
};
use deflorta_common::worker::runtime;
use log::{debug, error, info};
use num_traits::AsPrimitive;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{
    ElementState, Ime, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::ModifiersState;
#[cfg(feature = "dev-console")]
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::engine::{Engine, KeyModifiers};
use crate::render::Renderer;

#[cfg(not(target_os = "android"))]
#[path = "gamepad.rs"]
mod gamepad;

#[path = "keyboard.rs"]
mod keyboard;

use keyboard::key_name;

#[path = "touch.rs"]
mod touch;

use touch::{TouchAction, TouchState};

pub enum AppEvent {
    Accessibility(accesskit_winit::Event),
    WorkerReady,
    #[cfg(not(target_os = "android"))]
    Gamepad {
        key: &'static str,
        down: bool,
        repeat: bool,
    },
}

impl From<accesskit_winit::Event> for AppEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        Self::Accessibility(event)
    }
}

pub struct App {
    engine: Engine,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    touch: Option<TouchState>,
    #[cfg(target_os = "android")]
    android_ime: crate::android_ime::AndroidIme,
    modifiers: ModifiersState,
    fullscreen: bool,
    booted: bool,
    error: Option<anyhow::Error>,
    accessibility: Option<accesskit_winit::Adapter>,
    accessibility_active: bool,
    accessibility_tree: Option<(u64, String)>,
    proxy: EventLoopProxy<AppEvent>,
    self_voicing: crate::self_voicing::SelfVoicing,
    self_voicing_notification: Option<NotificationId>,
    #[cfg(not(target_os = "android"))]
    discord: crate::discord::Discord,
    #[cfg(feature = "dev-console")]
    console: Option<crate::dev_console::DevConsole>,
    #[cfg(feature = "dev-console")]
    game_keys: std::collections::HashSet<String>,
}

impl App {
    pub fn new(engine: Engine, proxy: EventLoopProxy<AppEvent>) -> Self {
        let worker_proxy = proxy.clone();
        engine.set_waker(Arc::new(move || {
            let _ = worker_proxy.send_event(AppEvent::WorkerReady);
        }));
        let self_voicing = crate::self_voicing::SelfVoicing::default();
        let speech_proxy = proxy.clone();
        self_voicing.set_waker(Arc::new(move || {
            let _ = speech_proxy.send_event(AppEvent::WorkerReady);
        }));
        #[cfg(not(target_os = "android"))]
        gamepad::start(proxy.clone());
        Self {
            engine,
            renderer: None,
            window: None,
            touch: None,
            #[cfg(target_os = "android")]
            android_ime: crate::android_ime::AndroidIme::default(),
            modifiers: ModifiersState::empty(),
            fullscreen: false,
            booted: false,
            error: None,
            accessibility: None,
            accessibility_active: false,
            accessibility_tree: None,
            proxy,
            self_voicing,
            self_voicing_notification: None,
            #[cfg(not(target_os = "android"))]
            discord: crate::discord::Discord::default(),
            #[cfg(feature = "dev-console")]
            console: None,
            #[cfg(feature = "dev-console")]
            game_keys: std::collections::HashSet::new(),
        }
    }

    /// The error that stopped the event loop, if any.
    pub const fn take_error(&mut self) -> Option<anyhow::Error> {
        self.error.take()
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let config = self.engine.config();
        let attributes = Window::default_attributes()
            .with_visible(false)
            .with_title(&config.title)
            .with_inner_size(LogicalSize::new(
                f64::from(config.width),
                f64::from(config.height),
            ))
            .with_min_inner_size(LogicalSize::new(320.0, 180.0))
            .with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None)));
        let window = Arc::new(event_loop.create_window(attributes)?);
        self.accessibility = Some(accesskit_winit::Adapter::with_event_loop_proxy(
            event_loop,
            &window,
            self.proxy.clone(),
        ));
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
        let renderer = runtime().block_on(Renderer::for_window(&instance, window.clone()))?;
        #[cfg(feature = "dev-console")]
        {
            self.engine.enable_console()?;
            self.console = Some(renderer.create_console(&window));
        }
        let (w, h) = renderer.size();
        self.engine.resize(w, h);
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.window.as_ref().unwrap().set_visible(true);
        Ok(())
    }

    /// Carries out window changes, captures and redraw/quit requests made by the engine.
    fn handle_requests(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(not(target_os = "android"))]
        self.discord.sync(
            self.engine.config().discord.as_ref(),
            self.engine.discord_activity().as_ref(),
        );
        #[cfg(target_os = "android")]
        if self.window.is_some() {
            use winit::platform::android::ActiveEventLoopExtAndroid;
            self.android_ime
                .sync(event_loop.android_app(), &mut self.engine);
        }
        let requests = self.engine.take_requests();
        if let Some(renderer) = &mut self.renderer {
            for source in &requests.unload_textures {
                renderer.unload_texture(source);
            }
        }
        if let Some(on) = requests.self_voicing {
            self.self_voicing.set_enabled(on);
            self.sync_self_voicing_notification();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
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
        #[cfg(not(any(feature = "dev-console", target_os = "android")))]
        if let Some(on) = requests.text_input {
            window.set_ime_allowed(on);
        }
        #[cfg(feature = "dev-console")]
        if let Some(console) = &mut self.console {
            console.sync_ime(window, self.engine.ui.focused_input().is_some());
        }
        if requests.redraw || requests.capture {
            window.request_redraw();
        }
        if requests.quit {
            info!("Closing the window");
            event_loop.exit();
        }
    }

    fn sync_self_voicing_notification(&mut self) {
        if self.self_voicing.initializing() && self.self_voicing_notification.is_none() {
            let id = next_notification_id();
            self.engine.ui.show_notification(
                id,
                "Enabling self-voicing…".into(),
                NotificationOptions {
                    state: NotificationState::Loading,
                    duration: None,
                },
                Instant::now(),
            );
            self.self_voicing_notification = Some(id);
        } else if !self.self_voicing.initializing()
            && let Some(id) = self.self_voicing_notification.take()
        {
            self.engine.ui.dismiss_notification(id);
        }
    }

    fn redraw(&mut self) {
        #[cfg(feature = "dev-console")]
        let frame_started = Instant::now();
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        #[cfg(feature = "dev-console")]
        if let (Some(console), Some(window)) = (&mut self.console, &self.window)
            && let Some(source) = console.show(window)
        {
            crate::dev_console::DevConsole::result(self.engine.evaluate_console(&source));
        }
        #[cfg(feature = "dev-console")]
        if let Some(console) = &mut self.console {
            for source in console.take_unloads() {
                crate::dev_console::DevConsole::result(self.engine.unload_asset(renderer, &source));
            }
        }
        let now = Instant::now();
        let items = self.engine.frame(now);
        #[cfg(feature = "dev-console")]
        if let Some(console) = &mut self.console
            && console.update_inspectors(&mut self.engine, renderer, now)
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
        let clear = self.engine.clear_color();
        let size = renderer.size();
        let rendered_frame = renderer.render(
            items,
            &mut self.engine.ui,
            &mut self.engine.assets,
            clear,
            #[cfg(feature = "dev-console")]
            self.console.as_mut(),
        );
        match rendered_frame {
            Ok(true) => self.engine.presented_frame(Instant::now()),
            Ok(false) => {}
            Err(err) => error!("Render failed: {err:#}"),
        }
        #[cfg(feature = "dev-console")]
        if let Some(console) = &mut self.console {
            console.record_frame(frame_started.elapsed());
        }
        if self.accessibility_active
            && let Some(adapter) = &mut self.accessibility
        {
            let revision = self.engine.ui.accessibility_revision();
            if self
                .accessibility_tree
                .as_ref()
                .is_none_or(|(previous, title)| {
                    *previous != revision || title != &self.engine.config().title
                })
            {
                let update = self.engine.accessibility_update();
                self.accessibility_tree = Some((revision, self.engine.config().title.clone()));
                adapter.update_if_active(|| update);
            }
        }
        if self.self_voicing.enabled() {
            self.self_voicing.update(self.engine.ui.speech_snapshot());
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

    fn touch_event(&mut self, touch: Touch) {
        // Track one finger, so a second touch cannot release a drag.
        if touch.phase == TouchPhase::Started && self.touch.is_none() {
            self.engine
                .pointer_moved(Some((touch.location.x.as_(), touch.location.y.as_())));
            let (native, scroll) = self.engine.ui.pointer_interaction();
            self.touch = Some(TouchState::new(
                touch.id,
                (touch.location.x, touch.location.y),
                native,
                scroll,
                Instant::now(),
            ));
            // Sliders and fields need the press immediately. Other taps wait
            // until release, so a menu gesture cannot also advance the story.
            if native {
                self.engine.mouse_down("left");
            }
        } else if let Some(state) = &mut self.touch
            && state.id == touch.id
        {
            let scale = self
                .window
                .as_ref()
                .map_or(1.0, |window| window.scale_factor());
            let scroll_delta = state.move_to((touch.location.x, touch.location.y), scale);
            self.engine
                .pointer_moved(Some((touch.location.x.as_(), touch.location.y.as_())));
            if state.scroll && !state.native && state.consumed {
                // Scroll directly: at a container boundary, never fall through
                // to the story's wheel/rollback handler.
                if self.engine.ui.scroll_at(scroll_delta.as_())
                    && let Some(window) = &self.window
                {
                    window.request_redraw();
                }
            }
            if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                match state.finish(touch.phase == TouchPhase::Cancelled) {
                    TouchAction::Rollback => {
                        self.engine
                            .key("PageUp", true, false, &KeyModifiers::default());
                        self.engine
                            .key("PageUp", false, false, &KeyModifiers::default());
                    }
                    TouchAction::Tap => self.engine.mouse_down("left"),
                    TouchAction::None => {}
                }
                self.engine.mouse_up();
                self.engine.pointer_moved(None);
                self.touch = None;
            }
        }
    }

    fn fire_long_press(&mut self, now: Instant) {
        if let Some(state) = &mut self.touch
            && state.take_long_press(now)
        {
            self.engine.mouse_down("right");
            self.engine.mouse_up();
        }
    }

    #[cfg(feature = "dev-console")]
    fn console_event(&mut self, event: &WindowEvent) -> bool {
        let (Some(console), Some(window)) = (&mut self.console, &self.window) else {
            return false;
        };
        if let WindowEvent::KeyboardInput { event, .. } = event
            && event.logical_key == Key::Named(NamedKey::F12)
        {
            if event.state == ElementState::Pressed && !event.repeat {
                console.toggle();
                window.request_redraw();
            }
            return true;
        }
        let consumed = console.on_event(window, event);
        // Releases of keys pressed in the game must reach it even if the
        // console gained focus in between (e.g. Ctrl to skip dialogue).
        let game_release = if let WindowEvent::KeyboardInput { event, .. } = event {
            event.state == ElementState::Released
                && key_name(&event.logical_key).is_some_and(|key| self.game_keys.remove(&key))
        } else {
            false
        };
        if consumed && !game_release {
            match event {
                WindowEvent::CursorMoved { .. } | WindowEvent::CursorLeft { .. } => {
                    self.engine.pointer_moved(None);
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                } => self.engine.mouse_up(),
                _ => {}
            }
            return true;
        }
        false
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        let event = match event {
            AppEvent::Accessibility(event) => event,
            AppEvent::WorkerReady => {
                self.engine.poll();
                self.handle_requests(event_loop);
                return;
            }
            #[cfg(not(target_os = "android"))]
            AppEvent::Gamepad { key, down, repeat } => {
                self.engine.key(key, down, repeat, &KeyModifiers::default());
                self.handle_requests(event_loop);
                return;
            }
        };
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != event.window_id)
        {
            return;
        }
        match event.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.accessibility_active = true;
                self.accessibility_tree = None;
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                self.engine.accessibility_action(request);
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.accessibility_active = false;
            }
        }
        self.handle_requests(event_loop);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        info!("Application resumed");
        #[cfg(target_os = "android")]
        self.engine.set_suspended(false);
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
        if let (Some(adapter), Some(window)) = (&mut self.accessibility, &self.window) {
            adapter.process_event(window, &event);
        }
        #[cfg(feature = "dev-console")]
        if self.console_event(&event) {
            self.handle_requests(event_loop);
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                info!("Window close requested");
                self.engine.quit();
                self.handle_requests(event_loop);
            }
            WindowEvent::Focused(focused) => {
                debug!("Window {}", if focused { "focused" } else { "unfocused" });
                if !focused && self.touch.take().is_some() {
                    self.engine.mouse_up();
                    self.engine.pointer_moved(None);
                }
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
                    .pointer_moved(Some((position.x.as_(), position.y.as_())));
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
                    MouseScrollDelta::PixelDelta(p) => {
                        let pixels: f32 = p.y.as_();
                        -pixels / 40.0
                    }
                };
                if dy != 0.0 {
                    self.engine.wheel(dy);
                }
            }
            WindowEvent::Touch(touch) => self.touch_event(touch),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::Ime(Ime::Commit(text)) => self.engine.text_input(&text),
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let Some(key) = key_name(&event.logical_key) {
                    #[cfg(feature = "dev-console")]
                    if down {
                        self.game_keys.insert(key.clone());
                    }
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
        #[cfg(target_os = "android")]
        {
            // Android invalidates the native surface on suspension. Drop all
            // references before returning; resumed() creates a new surface.
            self.engine.mouse_up();
            self.engine.pointer_moved(None);
            self.touch = None;
            self.android_ime = crate::android_ime::AndroidIme::default();
            self.self_voicing.suspend();
            self.engine.set_suspended(true);
            // Android may kill a suspended process without calling exiting().
            self.engine.flush_storage();
            self.accessibility = None;
            self.accessibility_tree = None;
            self.accessibility_active = false;
            self.renderer = None;
            self.window = None;
            #[cfg(feature = "dev-console")]
            {
                self.console = None;
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(not(target_os = "android"))]
        self.discord.shutdown();
        self.engine.flush_storage();
        info!("Event loop exiting");
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "android")]
        if self.window.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        if self.self_voicing.retry_initialization() {
            self.sync_self_voicing_notification();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        self.fire_long_press(Instant::now());
        self.engine.fire_timers();
        self.engine.poll();
        if let Some(renderer) = &mut self.renderer {
            self.engine
                .collect_unused_resources(renderer, Instant::now());
        }
        self.handle_requests(event_loop);
        self.engine.idle();
        #[cfg(feature = "dev-console")]
        let console_refresh =
            if let (Some(console), Some(window)) = (&mut self.console, &self.window) {
                console.refresh(window)
            } else {
                None
            };
        match [
            self.engine.next_timer(),
            self.engine.next_loading_deadline(),
            self.engine.ui.next_notification_deadline(),
            self.engine.next_resource_cleanup(),
            self.touch.as_ref().and_then(|touch| touch.long_press),
            self.self_voicing.next_retry(),
            #[cfg(feature = "dev-console")]
            console_refresh,
        ]
        .into_iter()
        .flatten()
        .min()
        {
            Some(due) => event_loop.set_control_flow(ControlFlow::WaitUntil(due)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
