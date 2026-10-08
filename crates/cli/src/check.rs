use std::collections::{HashMap, HashSet};

use deflorta_assets::GameFiles;
use oxc::allocator::Allocator;

use crate::api::{self, FileKind, ImageUse};
use crate::graph::Graph;
use crate::project::{Project, sources};
use crate::report::Report;

const DEFAULT_ID: &str = "deflorta-game";

pub struct Outcome {
    pub report: Report,
    pub sources: HashMap<String, String>,
}

pub fn check(project: &Project, boot: bool) -> Outcome {
    let allocator = Allocator::default();
    let graph = project.graph(&allocator);
    let mut report = Report::default();
    // Analysis of a partially parsed program would report misleading errors.
    if !graph.modules.iter().any(|m| m.broken) {
        check_api(&graph, &project.files, &mut report);
    }
    check_assets(&project.files, &mut report);
    let sources = sources(&graph);
    report.items.splice(0..0, graph.report.items);
    if boot && !report.has_errors() {
        check_boot(&project.dir, &mut report);
    }
    Outcome { report, sources }
}

fn check_api(graph: &Graph, files: &GameFiles, report: &mut Report) {
    let facts = api::analyze(graph);
    let at = |loc: &api::Loc| (graph.module_id(loc.module).to_owned(), loc.span);

    for loc in &facts.dynamic_imports {
        let (module, span) = at(loc);
        report.error_at(
            &module,
            span,
            "dynamic import() is not supported; use a static import",
        );
    }

    check_labels(graph, &facts, report);

    for (name, kind, loc) in &facts.image_uses {
        let mut words = name.split(' ').filter(|w| !w.is_empty());
        let tag = words.next().unwrap_or_default();
        let (module, span) = at(loc);
        if *kind != ImageUse::Background
            && let Some(attributes) = facts.layered.get(tag)
        {
            let Some(attributes) = attributes else {
                continue;
            };
            for word in words {
                let attribute = word.strip_prefix('-').unwrap_or(word);
                if !attributes.contains(attribute) {
                    report.error_at(
                        &module,
                        span,
                        format!("layered image '{tag}' has no attribute '{attribute}'"),
                    );
                }
            }
            continue;
        }
        if facts.images.contains_key(name.as_str()) {
            continue;
        }
        let path = format!("images/{name}.png");
        if !files.exists(&path) {
            let message =
                format!("image '{name}' is not declared with image() and {path} does not exist");
            if facts.dynamic_images {
                report.warn_at(&module, span, message);
            } else {
                report.error_at(&module, span, message);
            }
        }
    }

    for (path, kind, loc) in &facts.files {
        let file = path.split('?').next().unwrap_or(path);
        if file.starts_with("user:") || files.exists(file) {
            continue;
        }
        let what = match kind {
            FileKind::Image => "image",
            FileKind::Audio => "audio file",
            FileKind::Video => "video",
            FileKind::Text => "text file",
        };
        let (module, span) = at(loc);
        let message = format!("{what} '{file}' does not exist");
        if *kind == FileKind::Text {
            report.warn_at(
                &module,
                span,
                format!("{message}; readText() will return null"),
            );
        } else {
            report.error_at(&module, span, message);
        }
    }

    for (message, loc) in &facts.nondeterministic {
        let (module, span) = at(loc);
        report.warn_at(&module, span, *message);
    }
}

fn check_labels(graph: &Graph, facts: &api::Facts, report: &mut Report) {
    let at = |loc: &api::Loc| (graph.module_id(loc.module).to_owned(), loc.span);
    let mut defined: HashMap<&str, &api::Loc> = HashMap::new();
    for (name, loc) in &facts.labels {
        if defined.insert(name, loc).is_some() {
            let (module, span) = at(loc);
            report.warn_at(
                &module,
                span,
                format!("label '{name}' is defined more than once; the last definition wins"),
            );
        }
    }
    if !defined.contains_key("start") {
        report.error("no 'start' label: starting a new game runs label('start', …)");
    }
    let referenced: HashSet<&str> = facts.label_refs.iter().map(|(n, _)| n.as_str()).collect();
    for (name, loc) in &facts.label_refs {
        if !defined.contains_key(name.as_str()) {
            let (module, span) = at(loc);
            report.error_at(&module, span, format!("unknown label '{name}'"));
        }
    }
    if !facts.dynamic_label_refs {
        for (name, loc) in &facts.labels {
            if !referenced.contains(name.as_str())
                && !matches!(name.as_str(), "start" | "splashscreen")
            {
                let (module, span) = at(loc);
                report.warn_at(
                    &module,
                    span,
                    format!("label '{name}' is never jumped to or called"),
                );
            }
        }
    }
}

fn check_assets(files: &GameFiles, report: &mut Report) {
    if deflorta_assets::font_families(files).is_empty() {
        report.warn("no fonts in fonts/; system fonts will be used and text may look different on each computer");
    }
    for path in files.list("tl") {
        if std::path::Path::new(&path)
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let valid = files
            .read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| match value {
                serde_json::Value::Object(table) => Some(table),
                _ => None,
            })
            .is_some_and(|table| table.values().all(|v| v.is_string() || v.is_null()));
        if !valid {
            report.error(format!(
                "{path} must be a JSON object mapping source text to a translation (or null)"
            ));
        }
    }
}

fn check_boot(path: &std::path::Path, report: &mut Report) {
    match crate::distribution::inspect(path) {
        Ok(config) => {
            let families = config.font_families;
            if config.id == DEFAULT_ID {
                report.warn("configure({ id }) is not set; saves would be shared with other games that do not set it");
            }
            if !families.is_empty() && !families.contains(&config.font) {
                report.error(format!(
                    "font '{}' is not in fonts/ (available: {})",
                    config.font,
                    families.join(", ")
                ));
            }
        }
        Err(err) => report.error(format!("the game failed to start: {err:#}")),
    }
}
