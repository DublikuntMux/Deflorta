use num_traits::ToPrimitive as _;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::commands::Inspector;
use super::diagnostics::{self, INSPECTOR_INTERVAL, LoadedAsset, ProcessSampler};
use crate::engine::Engine;
use crate::render::Renderer;

struct Frames {
    count: u64,
    total: Duration,
    last: Duration,
    peak: Duration,
    sample_count: u64,
    sample_time: Instant,
}

impl Default for Frames {
    fn default() -> Self {
        Self {
            count: 0,
            total: Duration::ZERO,
            last: Duration::ZERO,
            peak: Duration::ZERO,
            sample_count: 0,
            sample_time: Instant::now(),
        }
    }
}

pub struct Inspectors {
    open: [bool; 3],
    requested: [bool; 3],
    dirty: [bool; 3],
    next_update: Instant,
    tree: Option<accesskit::TreeUpdate>,
    assets: Vec<LoadedAsset>,
    stats: Vec<(String, String)>,
    asset_filter: String,
    process: ProcessSampler,
    frames: Frames,
}

impl Default for Inspectors {
    fn default() -> Self {
        Self {
            open: [false; 3],
            requested: [false; 3],
            dirty: [false; 3],
            next_update: Instant::now(),
            tree: None,
            assets: Vec::new(),
            stats: Vec::new(),
            asset_filter: String::new(),
            process: ProcessSampler::default(),
            frames: Frames::default(),
        }
    }
}

impl Inspectors {
    pub fn any_open(&self) -> bool {
        self.open.iter().any(|&open| open)
    }

    pub fn next_refresh(&self) -> Option<Instant> {
        self.any_open().then_some(self.next_update)
    }

    pub const fn request(&mut self, kind: Inspector, window: bool) {
        let index = kind.index();
        self.open[index] |= window;
        self.requested[index] |= !window;
        self.dirty[index] = true;
    }

    pub fn record_frame(&mut self, elapsed: Duration) {
        self.frames.count += 1;
        self.frames.total += elapsed;
        self.frames.last = elapsed;
        self.frames.peak = self.frames.peak.max(elapsed);
    }

    pub fn update(&mut self, engine: &mut Engine, renderer: &Renderer, now: Instant) -> bool {
        let periodic = now >= self.next_update;
        let mut repaint = false;
        for kind in Inspector::ALL {
            let index = kind.index();
            if !(self.dirty[index] || self.open[index] && periodic) {
                continue;
            }
            match kind {
                Inspector::Accessibility => {
                    let title = engine.config().title.clone();
                    self.tree = Some(engine.ui.accessibility_update(&title));
                }
                Inspector::Assets => {
                    self.assets = engine.loaded_assets();
                    self.assets.extend(renderer.loaded_assets());
                    self.assets
                        .sort_by(|a, b| (a.kind, &a.source).cmp(&(b.kind, &b.source)));
                }
                Inspector::Stats => self.update_stats(engine, renderer, now),
            }
            if self.requested[index] {
                let report = match kind {
                    Inspector::Accessibility => {
                        diagnostics::tree_report(self.tree.as_ref().unwrap())
                    }
                    Inspector::Assets => diagnostics::asset_report(&self.assets),
                    Inspector::Stats => diagnostics::stats_report(&self.stats),
                };
                super::print_report(&report);
            }
            repaint |= self.open[index];
            self.requested[index] = false;
            self.dirty[index] = false;
        }
        if periodic {
            self.next_update = now + INSPECTOR_INTERVAL;
        }
        repaint
    }

    fn update_stats(&mut self, engine: &Engine, renderer: &Renderer, now: Instant) {
        self.process.sample(now);
        self.stats.clone_from(&self.process.values);
        self.stats.extend(engine.diagnostic_stats());
        let gpu = renderer.diagnostic_stats();
        let seconds = now.duration_since(self.frames.sample_time).as_secs_f64();
        let rate = (self.frames.count - self.frames.sample_count)
            .to_f64()
            .unwrap_or(0.0)
            / seconds.max(0.001);
        self.frames.sample_count = self.frames.count;
        self.frames.sample_time = now;
        let average = self.frames.total.as_secs_f64() * 1000.0
            / self.frames.count.to_f64().unwrap_or(1.0).max(1.0);
        let (w, h) = renderer.size();
        self.stats.extend([
            (
                "Rendered redraws (includes inspectors)".into(),
                self.frames.count.to_string(),
            ),
            (
                "Redraws per second (event-driven)".into(),
                format!("{rate:.1}"),
            ),
            (
                "Redraw wall time: last / average / peak".into(),
                format!(
                    "{:.2} / {average:.2} / {:.2} ms",
                    self.frames.last.as_secs_f64() * 1000.0,
                    self.frames.peak.as_secs_f64() * 1000.0
                ),
            ),
            ("Surface resolution".into(), format!("{w}×{h}")),
            ("GPU adapter".into(), gpu.adapter),
            (
                "Resident image/video GPU textures".into(),
                gpu.textures.to_string(),
            ),
            (
                "Image/video texture bytes (RGBA8 estimate)".into(),
                diagnostics::bytes(gpu.texture_bytes),
            ),
            (
                "GPU buffers (wgpu counter)".into(),
                gpu.buffer_bytes.map_or_else(
                    || "Unavailable on this backend".into(),
                    |bytes| format!("{bytes} bytes"),
                ),
            ),
            (
                "GPU textures including atlases (wgpu counter)".into(),
                gpu.all_texture_bytes.map_or_else(
                    || "Unavailable on this backend".into(),
                    |bytes| format!("{bytes} bytes"),
                ),
            ),
            (
                "GPU allocated / reserved memory".into(),
                gpu.allocations.map_or_else(
                    || "Unavailable on this backend".into(),
                    |(used, reserved)| {
                        format!(
                            "{} / {}",
                            diagnostics::bytes(used),
                            diagnostics::bytes(reserved)
                        )
                    },
                ),
            ),
        ]);
    }

    pub fn show(&mut self, context: &egui::Context) {
        for kind in Inspector::ALL {
            let mut open = self.open[kind.index()];
            if !open {
                continue;
            }
            egui::Window::new(kind.title())
                .id(egui::Id::new(("dev-inspector", kind.index())))
                .open(&mut open)
                .default_pos([
                    40.0_f32.mul_add(kind.index().to_f32().unwrap_or(0.0), 80.0),
                    80.0,
                ])
                .default_size([560.0, 420.0])
                .min_size([260.0, 160.0])
                .show(context, |ui| {
                    ui.small("Live game snapshot · refreshes every 500 ms");
                    ui.separator();
                    match kind {
                        Inspector::Accessibility => show_tree(ui, self.tree.as_ref()),
                        Inspector::Assets => show_assets(ui, &self.assets, &mut self.asset_filter),
                        Inspector::Stats => show_stats(ui, &self.stats),
                    }
                });
            self.open[kind.index()] = open;
        }
    }
}

fn show_stats(ui: &mut egui::Ui, values: &[(String, String)]) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("resource-stats")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for (label, value) in values {
                        ui.label(label);
                        ui.label(value);
                        ui.end_row();
                    }
                });
        });
}

fn show_assets(ui: &mut egui::Ui, assets: &[LoadedAsset], filter: &mut String) {
    ui.add(egui::TextEdit::singleline(filter).hint_text("Filter source, kind or state"));
    let needle = filter.to_lowercase();
    let visible = assets
        .iter()
        .filter(|asset| {
            format!("{} {} {}", asset.kind, asset.source, asset.state)
                .to_lowercase()
                .contains(&needle)
        })
        .collect::<Vec<_>>();
    ui.label(format!(
        "{} / {} records · CPU images and GPU textures are separate allocations",
        visible.len(),
        assets.len()
    ));
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show_rows(
            ui,
            ui.text_style_height(&egui::TextStyle::Body) + 6.0,
            visible.len(),
            |ui, range| {
                for &asset in &visible[range] {
                    ui.horizontal(|ui| {
                        ui.strong(asset.kind);
                        ui.monospace(&asset.source);
                        ui.label(&asset.state);
                        ui.label(&asset.detail);
                        if let Some(bytes) = asset.bytes {
                            ui.label(diagnostics::bytes(bytes));
                        }
                    });
                }
            },
        );
}

fn show_tree(ui: &mut egui::Ui, tree: Option<&accesskit::TreeUpdate>) {
    let Some(tree) = tree else {
        ui.label("Waiting for the first snapshot…");
        return;
    };
    ui.label(format!(
        "{} nodes · focused node #{}",
        tree.nodes.len(),
        tree.focus.0
    ));
    let nodes: HashMap<_, _> = tree.nodes.iter().map(|(id, node)| (*id, node)).collect();
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(root) = &tree.tree {
                show_tree_node(ui, root.root, &nodes, tree.focus, true);
            }
        });
}

fn show_tree_node(
    ui: &mut egui::Ui,
    id: accesskit::NodeId,
    nodes: &HashMap<accesskit::NodeId, &accesskit::Node>,
    focus: accesskit::NodeId,
    root: bool,
) {
    let Some(node) = nodes.get(&id) else {
        return;
    };
    let label = node
        .label()
        .unwrap_or("")
        .chars()
        .take(100)
        .collect::<String>();
    let title = format!(
        "#{} {:?} {}{}",
        id.0,
        node.role(),
        label,
        if id == focus { " [focused]" } else { "" }
    );
    egui::CollapsingHeader::new(title)
        .id_salt(("accesskit-node", id.0))
        .default_open(root)
        .show(ui, |ui| {
            egui::CollapsingHeader::new("Properties")
                .id_salt(("accesskit-properties", id.0))
                .show(ui, |ui| {
                    ui.monospace(format!("{node:#?}"));
                });
            for &child in node.children() {
                show_tree_node(ui, child, nodes, focus, false);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_reports_are_independent_and_egui_can_draw_them() {
        let mut inspectors = Inspectors::default();
        assert!(!inspectors.any_open());
        assert!(inspectors.next_refresh().is_none());
        inspectors.request(Inspector::Assets, false);
        assert!(!inspectors.any_open());
        assert!(inspectors.requested[Inspector::Assets.index()]);
        for kind in Inspector::ALL {
            inspectors.request(kind, true);
        }
        assert!(inspectors.open.into_iter().all(|open| open));
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |root| {
            inspectors.show(root.ctx());
        });
        assert_ne!(output.shapes.len(), 0);
        output.textures_delta.clear();
        inspectors.open = [false; 3];
        assert!(inspectors.next_refresh().is_none());
    }

    #[test]
    #[ignore = "Requires a native GPU adapter"]
    fn live_snapshots_refresh_on_schedule_and_stop_when_closed() {
        // Isolate SpiderMonkey, which cannot be reinitialized in this process.
        if std::env::var_os("DEFLORTA_INSPECTOR_TEST_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "dev_console::inspectors::tests::live_snapshots_refresh_on_schedule_and_stop_when_closed",
                    "--ignored",
                ])
                .env("DEFLORTA_INSPECTOR_TEST_CHILD", "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let files = crate::GameFiles::open(&crate::workspace_dir().join("tests/ui-hover")).unwrap();
        let mut script = crate::script::ScriptHost::new(files.clone()).unwrap();
        script.run_main().unwrap();
        script.enable_console().unwrap();
        let ui = crate::ui::Ui::new(crate::ui::text::TextSystem::new(&files));
        let mut engine = Engine::new(script, crate::assets::Assets::new(files), ui, None);
        let mut renderer = pollster::block_on(Renderer::offscreen(1280, 720)).unwrap();
        engine.resize(1280, 720);
        let now = Instant::now();
        let items = engine.frame(now);
        let clear = engine.clear_color();
        renderer
            .render(items, &mut engine.ui, &mut engine.assets, clear, None)
            .unwrap();
        let mut inspectors = Inspectors::default();
        for kind in Inspector::ALL {
            inspectors.request(kind, true);
        }
        assert!(inspectors.update(&mut engine, &renderer, now));
        assert!(
            inspectors
                .assets
                .iter()
                .any(|asset| asset.source == "main.js")
        );
        assert!(
            inspectors
                .stats
                .iter()
                .any(|(label, value)| label == "GPU adapter" && !value.is_empty())
        );
        let first_tree = inspectors.tree.clone();
        engine
            .evaluate_console("deflorta.configure({title: 'Changed title'})")
            .unwrap();
        engine.frame(now);
        assert!(!inspectors.update(&mut engine, &renderer, now));
        assert_eq!(inspectors.tree, first_tree);
        std::thread::sleep(INSPECTOR_INTERVAL);
        assert!(inspectors.update(&mut engine, &renderer, Instant::now()));
        assert_ne!(inspectors.tree, first_tree);
        assert!(
            diagnostics::tree_report(inspectors.tree.as_ref().unwrap()).contains("Changed title")
        );

        inspectors.open = [false; 3];
        engine
            .evaluate_console("deflorta.configure({title: 'Closed'})")
            .unwrap();
        engine.frame(Instant::now());
        let last_tree = inspectors.tree.clone();
        assert!(!inspectors.update(&mut engine, &renderer, Instant::now() + INSPECTOR_INTERVAL));
        assert_eq!(inspectors.tree, last_tree);
        assert!(inspectors.next_refresh().is_none());
        inspectors.request(Inspector::Accessibility, false);
        assert!(!inspectors.update(&mut engine, &renderer, Instant::now() + INSPECTOR_INTERVAL));
        assert!(diagnostics::tree_report(inspectors.tree.as_ref().unwrap()).contains("Closed"));
        assert!(!inspectors.any_open());
        assert!(!inspectors.requested[Inspector::Accessibility.index()]);
    }
}
