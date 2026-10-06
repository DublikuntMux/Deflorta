//! Font loading and shaped rich-text buffers (cosmic-text via glyphon).

use std::collections::HashMap;
use std::path::Path;

use log::{info, warn};

use glyphon::cosmic_text::Align as TextAlignment;
use glyphon::{
    Attrs, Buffer, Color as GlyphColor, Family, FontSystem, Metrics, Shaping, Style as FontStyle,
    Weight, Wrap, fontdb,
};

use super::desc::{Color, SpanDesc, TextAlign};
use crate::util::math::unit_to_u8;

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

/// Underline or strikethrough, in physical pixels relative to the text origin.
#[derive(Clone, Debug)]
pub struct Decoration {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// None uses the node's text color.
    pub color: Option<Color>,
}

/// A ruby annotation buffer placed relative to the text origin.
#[derive(Clone, Debug)]
pub struct RubyPlacement {
    pub id: String,
    pub x: f32,
    pub y: f32,
}

#[derive(PartialEq, Clone)]
struct Shaped {
    spans: Vec<SpanDesc>,
    style: TextStyle,
    scale: f32,
    revealed: usize,
    /// Alpha baked into explicitly colored spans, quantized.
    alpha: u8,
    /// Forces every glyph to this color (text shadows).
    color_override: Option<[u8; 4]>,
}

impl Shaped {
    // A text-cache hit requires the exact shaping scale.
    #[allow(clippy::float_cmp)]
    fn matches(
        &self,
        spans: &[SpanDesc],
        style: &TextStyle,
        scale: f32,
        revealed: usize,
        alpha: u8,
        color_override: Option<[u8; 4]>,
    ) -> bool {
        self.spans == spans
            && self.style == *style
            && self.scale == scale
            && self.revealed == revealed
            && self.alpha == alpha
            && self.color_override == color_override
    }
}

/// A shaped text buffer owned by one text node.
pub struct TextEntry {
    pub buffer: Buffer,
    shaped: Option<Shaped>,
    width: Option<f32>,
    size: (f32, f32),
    pub decorations: Vec<Decoration>,
    pub rubies: Vec<RubyPlacement>,
}

pub struct TextSystem {
    pub font_system: FontSystem,
    entries: HashMap<String, TextEntry>,
}

/// Metadata layout: span index * 2 + 1 if the glyph is hidden by the typewriter.
const fn metadata(span: usize, hidden: bool) -> usize {
    span * 2 + if hidden { 1 } else { 0 }
}

impl TextSystem {
    /// Loads every font in `<game>/fonts`. Falls back to system fonts when the game ships none.
    pub fn new(game_dir: &Path) -> Self {
        let mut db = fontdb::Database::new();
        db.load_fonts_dir(game_dir.join("fonts"));
        let font_system = if db.is_empty() {
            warn!(
                "No fonts in {}/fonts, using system fonts",
                game_dir.display()
            );
            FontSystem::new()
        } else {
            let mut families: Vec<&str> = db
                .faces()
                .filter_map(|f| f.families.first().map(|(name, _)| name.as_str()))
                .collect();
            families.sort_unstable();
            families.dedup();
            info!("Loaded {} font faces: {}", db.len(), families.join(", "));
            FontSystem::new_with_locale_and_db("en-US".into(), db)
        };
        Self {
            font_system,
            entries: HashMap::new(),
        }
    }

    /// Borrows the font system mutably alongside read access to the buffers (for rendering).
    pub const fn split(&mut self) -> (&mut FontSystem, &HashMap<String, TextEntry>) {
        (&mut self.font_system, &self.entries)
    }

    pub fn entry(&self, id: &str) -> Option<&TextEntry> {
        self.entries.get(id)
    }

    /// Drops buffers whose owner node is not kept, including shadow/ruby buffers.
    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.entries.retain(|id, _| {
            let mut owner = id.as_str();
            loop {
                if keep(owner) {
                    return true;
                }
                let Some((parent, suffix)) = owner.rsplit_once('#') else {
                    return false;
                };
                if suffix != "shadow"
                    && !suffix.strip_prefix("ruby").is_some_and(|index| {
                        !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit())
                    })
                {
                    return false;
                }
                owner = parent;
            }
        });
    }

    /// Shapes `spans` for `id` (reusing the previous result when nothing
    /// changed) and returns the text size in physical pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        id: &str,
        spans: &[SpanDesc],
        style: &TextStyle,
        scale: f32,
        width: Option<f32>,
        revealed: usize,
        alpha: f32,
        color_override: Option<Color>,
    ) -> (f32, f32) {
        let font_system = &mut self.font_system;
        let entry = self.entries.entry(id.to_owned()).or_insert_with(|| {
            // Like CSS, only break between words; min-content width is the longest word.
            let mut buffer = Buffer::new(font_system, Metrics::new(16.0, 20.0));
            buffer.set_wrap(Wrap::Word);
            TextEntry {
                buffer,
                shaped: None,
                width: None,
                size: (0.0, 0.0),
                decorations: Vec::new(),
                rubies: Vec::new(),
            }
        });

        let has_colors = color_override.is_some() || spans.iter().any(|s| s.color.is_some());
        let shaped_alpha = if has_colors { unit_to_u8(alpha) } else { 255 };
        let shaped_color = color_override.map(|c| c.with_alpha_mul(alpha).to_rgba8());
        let shape_changed = entry.shaped.as_ref().is_none_or(|shaped| {
            !shaped.matches(spans, style, scale, revealed, shaped_alpha, shaped_color)
        });

        // The buffer width is in physical pixels, so a surface scale change
        // must update it even when the virtual width remains the same.
        let width = width.map(|w| w * scale);
        let width_changed = if entry.width == width {
            false
        } else {
            entry.buffer.set_size(width, None);
            entry.width = width;
            true
        };
        if shape_changed {
            let wanted = Shaped {
                spans: spans.to_vec(),
                style: style.clone(),
                scale,
                revealed,
                alpha: shaped_alpha,
                color_override: shaped_color,
            };
            let size = (style.font_size * scale).max(1.0);
            entry
                .buffer
                .set_metrics(Metrics::new(size, size * style.line_height));
            set_spans(&mut entry.buffer, &wanted);
            entry.shaped = Some(wanted);
        }
        let reshaped = width_changed || shape_changed;
        if !reshaped {
            return entry.size;
        }

        entry.buffer.shape_until_scroll(font_system, false);

        let mut w: f32 = 0.0;
        let mut h: f32 = 0.0;
        for run in entry.buffer.layout_runs() {
            w = w.max(run.line_w);
            h = run.line_top + run.line_height;
        }
        entry.size = (w, h);
        let (decorations, rubies) = annotate(&entry.buffer, spans);
        entry.decorations = decorations;
        let ruby_jobs: Vec<_> = rubies
            .into_iter()
            .enumerate()
            .map(|(k, (span, x0, x1, top))| (format!("{id}#ruby{k}"), span, x0, x1, top))
            .collect();
        let mut placements = Vec::new();
        for (ruby_id, span, x0, x1, top) in ruby_jobs {
            let source = &spans[span];
            let ruby_size = source.size.unwrap_or(style.font_size) * 0.5;
            let ruby_style = TextStyle {
                font_size: ruby_size,
                line_height: 1.0,
                align: None,
                ..style.clone()
            };
            let ruby_spans = [SpanDesc {
                text: source.ruby.clone().unwrap_or_default(),
                color: source.color,
                ..Default::default()
            }];
            let (rw, rh) = self.prepare(
                &ruby_id,
                &ruby_spans,
                &ruby_style,
                scale,
                None,
                usize::MAX,
                alpha,
                color_override,
            );
            placements.push(RubyPlacement {
                id: ruby_id,
                x: (x0 + x1 - rw) / 2.0,
                y: f32::mul_add(rh, -0.9, top),
            });
        }
        if let Some(entry) = self.entries.get_mut(id) {
            entry.rubies = placements;
        }
        (w, h)
    }
}

fn set_spans(buffer: &mut Buffer, shaped: &Shaped) {
    let style = &shaped.style;
    let scale = shaped.scale;
    let base_family = if style.family.is_empty() {
        Family::SansSerif
    } else {
        Family::Name(&style.family)
    };
    let base = Attrs::new()
        .family(base_family)
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
    let alpha = f32::from(shaped.alpha) / 255.0;

    let mut runs: Vec<(&str, Attrs)> = Vec::new();
    let mut pos = 0;
    for (index, span) in shaped.spans.iter().enumerate() {
        if span.text.is_empty() {
            continue;
        }
        let mut attrs = base.clone();
        if let Some(font) = &span.font {
            attrs = attrs.family(Family::Name(font));
        }
        if span.b {
            attrs = attrs.weight(Weight::BOLD);
        }
        if span.i {
            attrs = attrs.style(FontStyle::Italic);
        }
        if let Some(size) = span.size {
            let px = (size * scale).max(1.0);
            attrs = attrs.metrics(Metrics::new(px, px * style.line_height));
        }
        if let Some(rgba) = shaped.color_override {
            attrs = attrs.color(GlyphColor::rgba(rgba[0], rgba[1], rgba[2], rgba[3]));
        } else if let Some(color) = span.color {
            let [r, g, b, a] = color.with_alpha_mul(alpha).to_rgba8();
            attrs = attrs.color(GlyphColor::rgba(r, g, b, a));
        }
        let chars = span.text.chars().count();
        let visible = shaped.revealed.saturating_sub(pos).min(chars);
        let split = span
            .text
            .char_indices()
            .nth(visible)
            .map_or(span.text.len(), |(i, _)| i);
        if split > 0 {
            runs.push((
                &span.text[..split],
                attrs.clone().metadata(metadata(index, false)),
            ));
        }
        if split < span.text.len() {
            let hidden = attrs
                .color(GlyphColor::rgba(0, 0, 0, 0))
                .metadata(metadata(index, true));
            runs.push((&span.text[split..], hidden));
        }
        pos += chars;
    }
    buffer.set_rich_text(runs, &base, Shaping::Advanced, alignment);
}

type RubyJob = (usize, f32, f32, f32);

/// Computes decoration rectangles and ruby anchor ranges from the layout.
fn annotate(buffer: &Buffer, spans: &[SpanDesc]) -> (Vec<Decoration>, Vec<RubyJob>) {
    let mut decorations = Vec::new();
    let mut rubies: Vec<RubyJob> = Vec::new();
    let mut ruby_seen = vec![false; spans.len()];
    for run in buffer.layout_runs() {
        // Group consecutive visible glyphs by span.
        let mut groups: Vec<(usize, f32, f32, f32)> = Vec::new();
        for glyph in run.glyphs {
            if glyph.metadata % 2 == 1 {
                continue;
            }
            let span = glyph.metadata / 2;
            match groups.last_mut() {
                Some((s, _, x1, _)) if *s == span => *x1 = glyph.x + glyph.w,
                _ => groups.push((span, glyph.x, glyph.x + glyph.w, glyph.font_size)),
            }
        }
        for (span_index, x0, x1, font_size) in groups {
            let Some(span) = spans.get(span_index) else {
                continue;
            };
            let thickness = (font_size * 0.06).max(1.0);
            if span.u {
                decorations.push(Decoration {
                    x: x0,
                    y: f32::mul_add(font_size, 0.12, run.line_y),
                    w: x1 - x0,
                    h: thickness,
                    color: span.color,
                });
            }
            if span.s {
                decorations.push(Decoration {
                    x: x0,
                    y: f32::mul_add(font_size, -0.3, run.line_y),
                    w: x1 - x0,
                    h: thickness,
                    color: span.color,
                });
            }
            if span.ruby.is_some() && !ruby_seen[span_index] {
                ruby_seen[span_index] = true;
                rubies.push((span_index, x0, x1, run.line_top));
            }
        }
    }
    (decorations, rubies)
}

#[cfg(test)]
mod tests;
