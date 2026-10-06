//! Errors and warnings found in a game, printed with source excerpts.

use std::collections::HashMap;
use std::fmt::Write;

use oxc::diagnostics::{NamedSource, OxcDiagnostic};
use oxc::span::Span;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    /// Module id (game path) and span, for JavaScript findings.
    pub location: Option<(String, Span)>,
    pub help: Option<String>,
}

#[derive(Default, Debug)]
pub struct Report {
    pub items: Vec<Diagnostic>,
}

impl Report {
    pub fn push(
        &mut self,
        severity: Severity,
        message: impl Into<String>,
        location: Option<(&str, Span)>,
        help: Option<String>,
    ) {
        self.items.push(Diagnostic {
            severity,
            message: message.into(),
            location: location.map(|(module, span)| (module.to_owned(), span)),
            help,
        });
    }

    pub fn error_at(&mut self, module: &str, span: Span, message: impl Into<String>) {
        self.push(Severity::Error, message, Some((module, span)), None);
    }

    pub fn warn_at(&mut self, module: &str, span: Span, message: impl Into<String>) {
        self.push(Severity::Warning, message, Some((module, span)), None);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.push(Severity::Error, message, None, None);
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.push(Severity::Warning, message, None, None);
    }

    pub fn count(&self, severity: Severity) -> usize {
        self.items.iter().filter(|d| d.severity == severity).count()
    }

    pub fn has_errors(&self) -> bool {
        self.count(Severity::Error) > 0
    }

    /// Renders every finding; `sources` maps module ids to their text.
    pub fn render(&self, sources: &HashMap<String, String>) -> String {
        let mut out = String::new();
        for item in &self.items {
            let diagnostic = match item.severity {
                Severity::Error => OxcDiagnostic::error(item.message.clone()),
                Severity::Warning => OxcDiagnostic::warn(item.message.clone()),
            };
            let diagnostic = match &item.help {
                Some(help) => diagnostic.with_help(help.clone()),
                None => diagnostic,
            };
            let text = match &item.location {
                Some((module, span)) => match sources.get(module) {
                    Some(source) => diagnostic
                        .with_label(*span)
                        .render_with_source_code(NamedSource::new(module, source.clone())),
                    None => diagnostic.render(),
                },
                None => diagnostic.render(),
            };
            let _ = writeln!(out, "{}", text.trim_end());
        }
        out
    }

    /// One-line totals, e.g. "2 errors, 1 warning".
    pub fn summary(&self) -> String {
        let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
        format!(
            "{}, {}",
            plural(self.count(Severity::Error), "error"),
            plural(self.count(Severity::Warning), "warning")
        )
    }
}
