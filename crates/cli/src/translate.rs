use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use oxc::allocator::Allocator;
use serde_json::Value;

use crate::api;
use crate::graph::{Builtins, Graph, builtin_exports};
use crate::project::{Project, sources};
use crate::report::Report;

pub struct Strings {
    /// Unique translatable strings: the game's in source order, then the engine's.
    pub translatable: Vec<String>,
    pub report: Report,
    pub sources: HashMap<String, String>,
}

fn push_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, text: &str) {
    if !text.is_empty() && seen.insert(text.to_owned()) {
        out.push(text.to_owned());
    }
}

pub fn extract(project: &Project) -> Result<Strings> {
    let allocator = Allocator::default();
    let graph = project.graph(&allocator);
    if graph.report.has_errors() {
        let sources = sources(&graph);
        bail!(
            "the game has errors; fix them first\n{}",
            graph.report.render(&sources)
        );
    }
    let facts = api::analyze(&graph);
    let mut strings = Vec::new();
    let mut seen = HashSet::new();
    for (text, _) in &facts.strings {
        push_unique(&mut strings, &mut seen, text);
    }
    let mut report = Report::default();
    for loc in &facts.dynamic_strings {
        report.warn_at(
            graph.module_id(loc.module),
            loc.span,
            "text with ${…} substitutions is translated as a whole after substitution and cannot be extracted; use separate lines or _() pieces",
        );
    }

    let runtime_allocator = Allocator::default();
    let ids: Vec<&str> = deflorta_assets::BUILTIN_MODULES
        .iter()
        .map(|(id, _)| *id)
        .collect();
    let mut runtime = Graph::build(&runtime_allocator, &Builtins, &ids);
    runtime.link(&builtin_exports()?);
    for (text, _) in &api::analyze(&runtime).strings {
        push_unique(&mut strings, &mut seen, text);
    }
    Ok(Strings {
        translatable: strings,
        report,
        sources: sources(&graph),
    })
}

pub fn table_path(project: &Project, language: &str) -> PathBuf {
    project.dir.join("tl").join(format!("{language}.json"))
}

pub fn languages(project: &Project) -> Vec<String> {
    let mut languages: Vec<String> = project
        .files
        .list("tl")
        .iter()
        .filter_map(|path| path.strip_prefix("tl/")?.strip_suffix(".json"))
        .filter(|name| !name.contains('/'))
        .map(str::to_owned)
        .collect();
    languages.sort();
    languages
}

pub fn read_table(path: &Path) -> Result<Vec<(String, Value)>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut entries = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    let ordered: OrderedTable = serde::Deserialize::deserialize(&mut deserializer)
        .with_context(|| format!("{} is not a JSON object", path.display()))?;
    deserializer
        .end()
        .with_context(|| format!("{} has trailing data", path.display()))?;
    for (key, value) in ordered.0 {
        if !(value.is_string() || value.is_null()) {
            bail!(
                "{}: the translation of {key:?} must be a string or null",
                path.display()
            );
        }
        entries.push((key, value));
    }
    Ok(entries)
}

struct OrderedTable(Vec<(String, Value)>);

impl<'de> serde::Deserialize<'de> for OrderedTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = OrderedTable;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of translations")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<OrderedTable, A::Error> {
                let mut entries = Vec::new();
                let mut keys = HashSet::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    if !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate translation key {key:?}"
                        )));
                    }
                    entries.push((key, value));
                }
                Ok(OrderedTable(entries))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

fn write_table(path: &Path, entries: &[(String, Value)]) -> Result<()> {
    let mut out = String::from("{\n");
    for (i, (key, value)) in entries.iter().enumerate() {
        let comma = if i + 1 < entries.len() { "," } else { "" };
        let _ = writeln!(
            out,
            "  {}: {}{comma}",
            serde_json::to_string(key)?,
            serde_json::to_string(value)?
        );
    }
    out.push_str("}\n");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, out).with_context(|| format!("cannot write {}", path.display()))
}

pub struct Progress {
    pub language: String,
    pub translated: usize,
    pub total: usize,
    pub added: usize,
    pub obsolete: usize,
}

/// Adds missing strings to a language's table (as `null`). Translations are
/// kept; entries for text no longer in the game are removed with `prune`.
pub fn update(
    project: &Project,
    strings: &[String],
    language: &str,
    prune: bool,
) -> Result<Progress> {
    let path = table_path(project, language);
    let existing = read_table(&path)?;
    let current: HashSet<&str> = strings.iter().map(String::as_str).collect();
    let known: HashMap<&str, &Value> = existing.iter().map(|(k, v)| (k.as_str(), v)).collect();

    let mut entries: Vec<(String, Value)> = strings
        .iter()
        .map(|text| {
            (
                text.clone(),
                known
                    .get(text.as_str())
                    .map_or(Value::Null, |v| (*v).clone()),
            )
        })
        .collect();
    let obsolete: Vec<&(String, Value)> = existing
        .iter()
        .filter(|(key, _)| !current.contains(key.as_str()))
        .collect();
    if !prune {
        entries.extend(obsolete.iter().map(|&entry| entry.clone()));
    }
    write_table(&path, &entries)?;
    Ok(Progress {
        language: language.to_owned(),
        translated: strings
            .iter()
            .filter(|s| known.get(s.as_str()).is_some_and(|v| v.is_string()))
            .count(),
        total: strings.len(),
        added: strings
            .iter()
            .filter(|s| !known.contains_key(s.as_str()))
            .count(),
        obsolete: if prune { 0 } else { obsolete.len() },
    })
}

pub fn status(project: &Project, strings: &[String], language: &str) -> Result<Progress> {
    let existing = read_table(&table_path(project, language))?;
    let known: HashMap<&str, &Value> = existing.iter().map(|(k, v)| (k.as_str(), v)).collect();
    let current: HashSet<&str> = strings.iter().map(String::as_str).collect();
    Ok(Progress {
        language: language.to_owned(),
        translated: strings
            .iter()
            .filter(|s| known.get(s.as_str()).is_some_and(|v| v.is_string()))
            .count(),
        total: strings.len(),
        added: 0,
        obsolete: existing
            .iter()
            .filter(|(k, _)| !current.contains(k.as_str()))
            .count(),
    })
}

pub fn missing(project: &Project, strings: &[String], language: &str) -> Result<Vec<String>> {
    let existing = read_table(&table_path(project, language))?;
    let known: HashMap<&str, &Value> = existing.iter().map(|(k, v)| (k.as_str(), v)).collect();
    Ok(strings
        .iter()
        .filter(|s| !known.get(s.as_str()).is_some_and(|v| v.is_string()))
        .cloned()
        .collect())
}
