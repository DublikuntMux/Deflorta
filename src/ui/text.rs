//! Font loading and shaped text buffers (cosmic-text via glyphon).

use std::collections::HashMap;
use std::path::Path;

use glyphon::cosmic_text::Align as TextAlignment;
use glyphon::{
    Attrs, Buffer, Color as GlyphColor, Family, FontSystem, Metrics, Shaping, Style as FontStyle,
    Weight, fontdb,
};

use super::desc::TextAlign;

/// Resolved (inherited) text properties of a text node.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    pub align: Option<TextAlign>,
}

/// A shaped text buffer owned by one text node.
pub struct TextEntry {
    pub buffer: Buffer,
    content: String,
    style: TextStyle,
    scale: f32,
    width: Option<f32>,
    revealed: usize,
}

pub struct TextSystem {
    pub font_system: FontSystem,
    entries: HashMap<String, TextEntry>,
}

impl TextSystem {
    /// Loads every font in `<game>/fonts`. Falls back to system fonts when the game ships none.
    pub fn new(game_dir: &Path) -> Self {
        let mut db = fontdb::Database::new();
        db.load_fonts_dir(game_dir.join("fonts"));
        let font_system = if db.is_empty() {
            eprintln!(
                "[deflorta] no fonts in {}/fonts, using system fonts",
                game_dir.display()
            );
            FontSystem::new()
        } else {
            FontSystem::new_with_locale_and_db("en-US".into(), db)
        };
        TextSystem {
            font_system,
            entries: HashMap::new(),
        }
    }

    /// Borrows the font system mutably alongside read access to the buffers (for rendering).
    pub fn split(&mut self) -> (&mut FontSystem, &HashMap<String, TextEntry>) {
        (&mut self.font_system, &self.entries)
    }

    /// Drops buffers for nodes that no longer exist.
    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.entries.retain(|id, _| keep(id));
    }

    /// Ensures the buffer for `id` matches the given content, style and width,
    /// with the first `revealed` characters visible, then returns its size in
    /// physical pixels.
    pub fn prepare(
        &mut self,
        id: &str,
        content: &str,
        style: &TextStyle,
        scale: f32,
        width: Option<f32>,
        revealed: usize,
    ) -> (f32, f32) {
        let font_system = &mut self.font_system;
        let entry = self
            .entries
            .entry(id.to_owned())
            .or_insert_with(|| TextEntry {
                buffer: Buffer::new(font_system, Metrics::new(16.0, 20.0)),
                content: String::new(),
                style: style.clone(),
                scale: 0.0,
                width: None,
                revealed: usize::MAX,
            });

        let metrics_changed = entry.scale != scale || entry.style != *style;
        if metrics_changed {
            let size = (style.font_size * scale).max(1.0);
            entry
                .buffer
                .set_metrics(Metrics::new(size, size * style.line_height));
        }
        if entry.width != width {
            entry.buffer.set_size(width.map(|w| w * scale), None);
            entry.width = width;
        }
        if metrics_changed || entry.content != content || entry.revealed != revealed {
            let family = if style.family.is_empty() {
                Family::SansSerif
            } else {
                Family::Name(&style.family)
            };
            let attrs = Attrs::new()
                .family(family)
                .weight(Weight(style.weight))
                .style(if style.italic {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                });
            let alignment = style.align.map(|a| match a {
                TextAlign::Left => TextAlignment::Left,
                TextAlign::Center => TextAlignment::Center,
                TextAlign::Right => TextAlignment::Right,
                TextAlign::Justify => TextAlignment::Justified,
            });
            let split = content
                .char_indices()
                .nth(revealed)
                .map_or(content.len(), |(i, _)| i);
            let hidden = attrs.clone().color(GlyphColor::rgba(0, 0, 0, 0));
            entry.buffer.set_rich_text(
                [
                    (&content[..split], attrs.clone()),
                    (&content[split..], hidden),
                ],
                &attrs,
                Shaping::Advanced,
                alignment,
            );
            entry.content.clear();
            entry.content.push_str(content);
            entry.style = style.clone();
            entry.scale = scale;
            entry.revealed = revealed;
        }
        entry.buffer.shape_until_scroll(font_system, false);

        let mut w: f32 = 0.0;
        let mut h: f32 = 0.0;
        for run in entry.buffer.layout_runs() {
            w = w.max(run.line_w);
            h = run.line_top + run.line_height;
        }
        (w, h)
    }
}
