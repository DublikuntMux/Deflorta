//! Debug launcher console. Shares the game's GPU and JavaScript realm.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{Color32, Key, RichText};
use log::{Level, Log, Metadata, Record};
use num_traits::AsPrimitive;
use winit::event::WindowEvent;
use winit::window::Window;

const MAX_ENTRIES: usize = 2_000;
const MAX_HISTORY: usize = 100;
const REFRESH_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone)]
struct Entry {
    level: Level,
    text: String,
}

#[derive(Default)]
struct LogBuffer {
    entries: VecDeque<Entry>,
    revision: u64,
}

impl LogBuffer {
    fn push(&mut self, level: Level, mut text: String) {
        // Keep a noisy script from consuming unbounded memory.
        if let Some((end, _)) = text.char_indices().nth(16_384) {
            text.truncate(end);
            text.push_str("… [truncated]");
        }
        if self.entries.len() == MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { level, text });
        self.revision = self.revision.wrapping_add(1);
    }
}

fn logs() -> &'static Mutex<LogBuffer> {
    static LOGS: OnceLock<Mutex<LogBuffer>> = OnceLock::new();
    LOGS.get_or_init(|| Mutex::new(LogBuffer::default()))
}

fn append(level: Level, text: String) {
    logs()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(level, text);
}

struct ConsoleLogger {
    stderr: pretty_env_logger::env_logger::Logger,
    started: Instant,
}

impl Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.stderr.enabled(metadata) || metadata.target() == "deflorta::js"
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            append(
                record.level(),
                format!(
                    "{:.3} {} {}: {}",
                    self.started.elapsed().as_secs_f64(),
                    record.level(),
                    record.target(),
                    record.args()
                ),
            );
        }
        self.stderr.log(record);
    }

    fn flush(&self) {
        self.stderr.flush();
    }
}

pub fn init_logging(filter: &str) {
    let stderr = pretty_env_logger::formatted_timed_builder()
        .parse_filters(filter)
        .build();
    let max_level = stderr.filter().max(log::LevelFilter::Debug);
    log::set_boxed_logger(Box::new(ConsoleLogger {
        stderr,
        started: Instant::now(),
    }))
    .expect("logger already initialized");
    log::set_max_level(max_level);
}

pub struct DevConsole {
    context: egui::Context,
    input: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    open: bool,
    focus_prompt: bool,
    command: String,
    history: VecDeque<String>,
    history_index: Option<usize>,
    draft: String,
    filter: String,
    follow: bool,
    revision: u64,
    next_refresh: Instant,
    paint_jobs: Vec<egui::ClippedPrimitive>,
    textures: egui::TexturesDelta,
    pixels_per_point: f32,
    next_repaint: Option<Instant>,
}

impl DevConsole {
    pub fn new(window: &Window, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let context = egui::Context::default();
        let input = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor().as_()),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d.as_()),
        );
        Self {
            context,
            input,
            renderer: egui_wgpu::Renderer::new(
                device,
                format,
                egui_wgpu::RendererOptions::default(),
            ),
            open: false,
            focus_prompt: false,
            command: String::new(),
            history: VecDeque::new(),
            history_index: None,
            draft: String::new(),
            filter: String::new(),
            follow: true,
            revision: 0,
            next_refresh: Instant::now(),
            paint_jobs: Vec::new(),
            textures: egui::TexturesDelta::default(),
            pixels_per_point: 1.0,
            next_repaint: None,
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.focus_prompt = self.open;
        if !self.open {
            self.context
                .memory_mut(|memory| memory.surrender_focus(egui::Id::new("dev-console-prompt")));
        }
    }

    pub fn on_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.input.on_window_event(window, event);
        if response.repaint {
            window.request_redraw();
        }
        self.open && response.consumed
    }

    pub fn sync_ime(&mut self, window: &Window, game_text_input: bool) {
        // egui controls IME while its text fields have focus. Otherwise restore
        // the game's setting and keep egui-winit's cached OS state in sync.
        if self.open && self.context.egui_wants_keyboard_input() {
            return;
        }
        if self.input.allow_ime() != game_text_input {
            window.set_ime_allowed(game_text_input);
            self.input.set_allow_ime(game_text_input);
        }
    }

    /// Refresh asynchronous log messages without continuously redrawing the game.
    pub fn refresh(&mut self, window: &Window) -> Option<Instant> {
        if !self.open {
            return None;
        }
        let now = Instant::now();
        if self.next_repaint.is_some_and(|due| due <= now) {
            window.request_redraw();
            self.next_repaint = None;
        }
        if now >= self.next_refresh {
            let revision = logs()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .revision;
            if revision != self.revision {
                window.request_redraw();
            }
            self.next_refresh = now + REFRESH_INTERVAL;
        }
        Some(
            self.next_repaint
                .map_or(self.next_refresh, |due| due.min(self.next_refresh)),
        )
    }

    pub fn show(&mut self, window: &Window) -> Option<String> {
        let input = self.input.take_egui_input(window);
        let context = self.context.clone();
        let mut command = None;
        let mut open = self.open;
        let output = context.run_ui(input, |root| {
            if !self.open {
                return;
            }
            egui::Window::new("Developer console · F12")
                .id(egui::Id::new("dev-console"))
                .open(&mut open)
                .default_pos([16.0, 16.0])
                .default_size([680.0, 380.0])
                .min_size([280.0, 180.0])
                .show(root.ctx(), |ui| {
                    self.show_logs(ui);
                    ui.separator();
                    let prompt_id = egui::Id::new("dev-console-prompt");
                    let focused = ui.memory(|memory| memory.has_focus(prompt_id));
                    let submit = focused
                        && ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, Key::Enter)
                        });
                    if focused && !self.command.contains('\n') {
                        if ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                        }) {
                            self.history_up();
                        }
                        if ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                        }) {
                            self.history_down();
                        }
                    }
                    let prompt = ui.add(
                        egui::TextEdit::multiline(&mut self.command)
                            .id(prompt_id)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(2)
                            .char_limit(65_536)
                            .return_key(Some(egui::KeyboardShortcut::new(
                                egui::Modifiers::SHIFT,
                                Key::Enter,
                            )))
                            .hint_text("JavaScript…"),
                    );
                    if self.focus_prompt {
                        prompt.request_focus();
                        self.focus_prompt = false;
                    }
                    ui.horizontal(|ui| {
                        if (ui.button("Run").clicked() || submit) && !self.command.trim().is_empty()
                        {
                            command = Some(std::mem::take(&mut self.command));
                            prompt.request_focus();
                        }
                        ui.small("Enter: run · Shift+Enter: newline · PageUp/PageDown: history");
                    });
                });
        });
        self.open = open;
        self.next_repaint = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|output| Instant::now().checked_add(output.repaint_delay));
        self.input
            .handle_platform_output(window, output.platform_output);
        self.pixels_per_point = output.pixels_per_point;
        self.paint_jobs = context.tessellate(output.shapes, output.pixels_per_point);
        self.textures.append(output.textures_delta);
        if let Some(source) = &command {
            if self.history.back() != Some(source) {
                if self.history.len() == MAX_HISTORY {
                    self.history.pop_front();
                }
                self.history.push_back(source.clone());
            }
            self.history_index = None;
            self.draft.clear();
            append(Level::Info, format!("> {source}"));
            window.request_redraw();
        }
        command
    }

    fn show_logs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Clear").clicked() {
                let mut logs = logs()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                logs.entries.clear();
                logs.revision = logs.revision.wrapping_add(1);
            }
            ui.checkbox(&mut self.follow, "Follow logs");
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filter logs"));
        });
        ui.label("Live game realm · deflorta.store, deflorta.config, deflorta.jump(…)");
        let filter = self.filter.to_lowercase();
        let entries = {
            let logs = logs()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.revision = logs.revision;
            logs.entries
                .iter()
                .filter(|entry| filter.is_empty() || entry.text.to_lowercase().contains(&filter))
                .cloned()
                .collect::<Vec<_>>()
        };
        egui::ScrollArea::vertical()
            .stick_to_bottom(self.follow)
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 100.0).max(60.0))
            .show(ui, |ui| {
                for entry in entries {
                    let color = match entry.level {
                        Level::Error => Color32::from_rgb(255, 115, 115),
                        Level::Warn => Color32::from_rgb(255, 205, 100),
                        Level::Debug | Level::Trace => Color32::GRAY,
                        Level::Info => ui.visuals().text_color(),
                    };
                    ui.add(
                        egui::Label::new(RichText::new(entry.text).monospace().color(color))
                            .selectable(true)
                            .wrap(),
                    );
                }
            });
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let index = self.history_index.map_or_else(
            || {
                self.draft.clone_from(&self.command);
                self.history.len() - 1
            },
            |index| index.saturating_sub(1),
        );
        self.history_index = Some(index);
        self.command.clone_from(&self.history[index]);
    }

    fn history_down(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 < self.history.len() {
            self.history_index = Some(index + 1);
            self.command.clone_from(&self.history[index + 1]);
        } else {
            self.history_index = None;
            self.command.clone_from(&self.draft);
        }
    }

    pub fn result(result: anyhow::Result<String>) {
        match result {
            Ok(value) => append(Level::Info, format!("< {value}")),
            Err(error) => append(Level::Error, format!("{error:#}")),
        }
    }

    pub fn upload_textures(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        for (id, deltas) in self.textures.set.drain() {
            for delta in deltas {
                self.renderer.update_texture(device, queue, id, &delta);
            }
        }
    }

    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        size: [u32; 2],
    ) -> Vec<wgpu::CommandBuffer> {
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: self.pixels_per_point,
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dev console"),
        });
        let mut commands =
            self.renderer
                .update_buffers(device, queue, &mut encoder, &self.paint_jobs, &screen);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("dev console"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &self.paint_jobs, &screen);
        }
        commands.push(encoder.finish());
        for id in self.textures.free.drain() {
            self.renderer.free_texture(&id);
        }
        commands
    }
}

impl Drop for DevConsole {
    fn drop(&mut self) {
        // A skipped/occluded final frame can still have pending texture frees.
        self.textures.clear();
    }
}
