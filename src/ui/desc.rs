//! Serialized UI tree and command types produced by the JS runtime.

use std::collections::HashMap;

use num_traits::{AsPrimitive, ToPrimitive};
use serde::Deserialize;

use crate::util::math::unit_to_u8;

/// Output of one `__deflorta_pump()` call.
#[derive(Deserialize, Default)]
pub struct PumpOutput {
    pub tree: Option<NodeDesc>,
    #[serde(default)]
    pub instant: bool,
    /// Exit animations for keyed elements removed in this commit.
    #[serde(default)]
    pub exits: HashMap<String, Option<AnimDesc>>,
    #[serde(default)]
    pub cmds: Vec<Command>,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Command {
    Config {
        config: GameConfig,
    },
    Timer {
        id: u64,
        ms: f64,
    },
    CancelTimer {
        id: u64,
    },
    #[serde(rename_all = "camelCase")]
    Music {
        file: Option<String>,
        #[serde(default = "default_true")]
        r#loop: bool,
        #[serde(default = "default_one")]
        volume: f32,
        #[serde(default)]
        fade_in: f32,
        #[serde(default)]
        fade_out: f32,
    },
    Sound {
        file: String,
        #[serde(default = "default_one")]
        volume: f32,
    },
    /// Plays a voice line, stopping the previous one; `None` stops voice.
    Voice {
        file: Option<String>,
    },
    Volume {
        channel: String,
        value: f32,
    },
    /// Shows all text up to the next click-wait (or the end).
    RevealSkip,
    /// Starts decoding images in the background.
    Preload {
        images: Vec<String>,
    },
    /// Captures a frame for a save thumbnail: the screen as it is now (before
    /// the next tree is shown, e.g. under a menu that is opening), or with
    /// `after`, once the next tree is shown (e.g. a scene that is starting).
    CaptureThumbnail {
        #[serde(default)]
        after: bool,
    },
    /// Writes the last captured thumbnail to `<data dir>/<name>.png`.
    SaveThumbnail {
        name: String,
    },
    /// Removes `<data dir>/<name>.png` (when its save is deleted).
    DeleteThumbnail {
        name: String,
    },
    Fullscreen {
        on: bool,
    },
    Quit,
}

const fn default_true() -> bool {
    true
}

const fn default_one() -> f32 {
    1.0
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameConfig {
    pub id: String,
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub font: String,
    /// Game version as set by the script (any JSON value, e.g. "1.2" or 3).
    #[serde(default)]
    pub version: Option<serde_json::Value>,
    #[serde(default)]
    pub clear_color: Option<Color>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Box,
    Text,
    Image,
    Slider,
    Input,
    Video,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Fill,
    Cover,
    Contain,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodeDesc {
    pub t: Option<NodeKind>,
    pub key: Option<String>,
    #[serde(default)]
    pub style: Style,
    /// Paint-only overrides applied while hovered or focused.
    pub hover: Option<Style>,
    pub on_click: Option<u32>,
    /// Slider value changes.
    pub on_change: Option<u32>,
    /// Text input edits.
    pub on_input: Option<u32>,
    /// Enter pressed in a text input.
    pub on_submit: Option<u32>,
    /// A non-looping video finished.
    pub on_end: Option<u32>,
    /// Whether keyboard/gamepad focus can land here (defaults to having a handler).
    pub focusable: Option<bool>,
    /// Receive focus when the element appears.
    #[serde(default)]
    pub autofocus: bool,
    pub tooltip: Option<String>,
    /// Scroll containers: start scrolled to the end (chat logs, history).
    #[serde(default)]
    pub start_at_end: bool,
    #[serde(default)]
    pub children: Vec<Self>,
    pub text: Option<String>,
    /// Rich text; takes precedence over `text`.
    pub spans: Option<Vec<SpanDesc>>,
    /// Typewriter speed in characters per second.
    pub cps: Option<f32>,
    pub src: Option<String>,
    /// Image shown while hovered or focused (image buttons).
    pub hover_src: Option<String>,
    #[serde(default)]
    pub fit: Fit,
    /// Fraction of the element's size placed at its layout position, e.g. [0.5, 1] for bottom-center.
    pub anchor: Option<[f32; 2]>,
    pub enter: Option<AnimDesc>,
    pub exit: Option<AnimDesc>,
    /// Animate layout position changes of this keyed element.
    pub r#move: Option<AnimDesc>,
    /// ATL-style animation program.
    pub transform: Option<TransformDesc>,
    // Slider
    pub value: Option<serde_json::Value>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub step: Option<f32>,
    // Input
    pub placeholder: Option<String>,
    pub max_length: Option<usize>,
    // Video
    #[serde(default)]
    pub r#loop: bool,
}

impl NodeDesc {
    pub fn kind(&self) -> NodeKind {
        self.t.unwrap_or(NodeKind::Box)
    }

    pub fn number_value(&self) -> f32 {
        self.value
            .as_ref()
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
            .as_()
    }

    pub fn string_value(&self) -> &str {
        self.value.as_ref().and_then(|v| v.as_str()).unwrap_or("")
    }

    pub fn is_focusable(&self) -> bool {
        self.focusable.unwrap_or_else(|| {
            self.on_click.is_some() || self.on_change.is_some() || self.kind() == NodeKind::Input
        })
    }
}

/// One run of rich text. Empty-text spans carry typewriter pauses.
// These independent flags mirror the JavaScript rich-text schema.
#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize, Clone, Default, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpanDesc {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub b: bool,
    #[serde(default)]
    pub i: bool,
    #[serde(default)]
    pub u: bool,
    #[serde(default)]
    pub s: bool,
    pub color: Option<Color>,
    /// Absolute font size in virtual pixels.
    pub size: Option<f32>,
    pub font: Option<String>,
    /// Annotation drawn above the span (furigana).
    pub ruby: Option<String>,
    /// Typewriter pauses for this many seconds before the span.
    pub wait: Option<f32>,
    /// Typewriter stops before the span until the player clicks.
    #[serde(default)]
    pub click: bool,
    /// Everything before this span appears instantly.
    #[serde(default)]
    pub fast: bool,
}

/// Easing curves, named like Ren'Py warpers (`easein`, `easeout`, …).
#[allow(clippy::enum_variant_names)]
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    #[default]
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
}

impl Ease {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t * t,
            Self::EaseOut => 1.0 - (1.0 - t).powi(3),
            Self::Ease | Self::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0f32).mul_add(t, 2.0).powi(3) / 2.0
                }
            }
            Self::Bounce => {
                let (n, d) = (7.5625, 2.75);
                if t < 1.0 / d {
                    n * t * t
                } else if t < 2.0 / d {
                    let t = t - 1.5 / d;
                    (n * t).mul_add(t, 0.75)
                } else if t < 2.5 / d {
                    let t = t - 2.25 / d;
                    (n * t).mul_add(t, 0.9375)
                } else {
                    let t = t - 2.625 / d;
                    (n * t).mul_add(t, 0.984_375)
                }
            }
        }
    }
}

/// Animation endpoint: `enter` gives starting values, `exit` final values.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct AnimDesc {
    pub dur: f32,
    pub ease: Option<Ease>,
    pub opacity: Option<f32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub scale: Option<f32>,
    pub rotate: Option<f32>,
    pub mask: Option<MaskDesc>,
}

impl AnimDesc {
    /// Enter/exit transitions default to ease-out; moves to ease-in-out.
    pub fn ease_or(&self, default: Ease) -> Ease {
        self.ease.unwrap_or(default)
    }
}

/// Screen-space reveal pattern for transitions.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MaskDesc {
    /// Reveal where the mask image is darkest first (image dissolve).
    Image {
        src: String,
        #[serde(default = "default_ramp")]
        ramp: f32,
    },
    Wipe {
        #[serde(default)]
        dir: WipeDir,
        #[serde(default = "default_ramp")]
        ramp: f32,
    },
    Pixellate {
        #[serde(default = "default_block")]
        size: f32,
    },
}

const fn default_ramp() -> f32 {
    0.1
}

const fn default_block() -> f32 {
    32.0
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WipeDir {
    Left,
    #[default]
    Right,
    Up,
    Down,
}

/// Animatable properties of a transform step.
#[derive(Deserialize, Clone, Default, Debug, PartialEq)]
pub struct TransformProps {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub opacity: Option<f32>,
    pub scale: Option<f32>,
    pub rotate: Option<f32>,
    /// Fractional crop rectangle [x, y, w, h] of an image.
    pub crop: Option<[f32; 4]>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum RepeatCount {
    Forever(bool),
    Times(u32),
}

/// One instruction of an ATL-style program.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum TransformStep {
    Set {
        set: TransformProps,
    },
    Tween {
        dur: f32,
        #[serde(default)]
        ease: Ease,
        to: TransformProps,
    },
    Pause {
        pause: f32,
    },
    Parallel {
        parallel: Vec<Vec<Self>>,
    },
    Repeat {
        repeat: RepeatCount,
        steps: Vec<Self>,
    },
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct TransformDesc {
    pub steps: Vec<TransformStep>,
}

/// A length: a number of virtual pixels, `"50%"`, `"12px"` or `"auto"`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dim {
    Px(f32),
    Percent(f32),
    Auto,
}

impl<'de> Deserialize<'de> for Dim {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Number(f32),
            Text(String),
        }
        let text = match Raw::deserialize(d)? {
            Raw::Number(v) => return Ok(Self::Px(v)),
            Raw::Text(text) => text,
        };
        let s = text.trim();
        let number = |v: &str| v.trim().parse::<f32>().map_err(serde::de::Error::custom);
        if s == "auto" {
            Ok(Self::Auto)
        } else if let Some(p) = s.strip_suffix('%') {
            Ok(Self::Percent(number(p)? / 100.0))
        } else if let Some(p) = s.strip_suffix("px") {
            Ok(Self::Px(number(p)?))
        } else {
            Err(serde::de::Error::custom(format!("invalid dimension '{s}'")))
        }
    }
}

/// Box edges: a number, [vertical, horizontal] or [top, right, bottom, left].
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(untagged)]
pub enum Edges {
    All(f32),
    Two([f32; 2]),
    Four([f32; 4]),
}

impl Edges {
    /// Returns (top, right, bottom, left).
    pub const fn trbl(self) -> [f32; 4] {
        match self {
            Self::All(v) => [v; 4],
            Self::Two([v, h]) => [v, h, v, h],
            Self::Four(e) => e,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color(pub [f32; 4]);

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid color '{s}'")))
    }
}

impl Color {
    pub const WHITE: Self = Self([1.0, 1.0, 1.0, 1.0]);
    pub const TRANSPARENT: Self = Self([0.0; 4]);

    /// Parses `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` (sRGB).
    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.trim().strip_prefix('#')?;
        let digits: Vec<u8> = hex
            .chars()
            .map(|c| c.to_digit(16).and_then(|d| d.to_u8()))
            .collect::<Option<_>>()?;
        let channels: Vec<u8> = match digits.len() {
            3 | 4 => digits.iter().map(|d| d * 17).collect(),
            6 | 8 => digits.chunks(2).map(|c| c[0] * 16 + c[1]).collect(),
            _ => return None,
        };
        let a = channels.get(3).copied().unwrap_or(255);
        Some(Self([
            f32::from(channels[0]) / 255.0,
            f32::from(channels[1]) / 255.0,
            f32::from(channels[2]) / 255.0,
            f32::from(a) / 255.0,
        ]))
    }

    pub fn with_alpha_mul(self, a: f32) -> Self {
        let [r, g, b, alpha] = self.0;
        Self([r, g, b, alpha * a])
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        self.0.map(unit_to_u8)
    }
}

#[cfg(test)]
mod color_tests {
    use super::Color;

    #[test]
    fn byte_channels_round_clamp_and_handle_nan() {
        assert_eq!(
            Color([-1.0, 0.5, 2.0, f32::NAN]).to_rgba8(),
            [0, 128, 255, 0]
        );
        assert_eq!(
            Color([f32::NEG_INFINITY, f32::INFINITY, 0.0, 1.0]).to_rgba8(),
            [0, 255, 0, 255]
        );
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub color: Color,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum FontWeight {
    Number(u16),
    Name(FontWeightName),
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FontWeightName {
    Normal,
    Bold,
}

impl FontWeight {
    pub const fn value(self) -> u16 {
        match self {
            Self::Number(n) => n,
            Self::Name(FontWeightName::Normal) => 400,
            Self::Name(FontWeightName::Bold) => 700,
        }
    }
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident = $text:literal),* $(,)? }) => {
        #[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $(#[serde(rename = $text)] $variant),* }
    };
}

string_enum!(Display { Flex = "flex", Grid = "grid", None = "none" });
string_enum!(Position { Relative = "relative", Absolute = "absolute" });
string_enum!(FlexDirection {
    Row = "row",
    Column = "column",
    RowReverse = "row-reverse",
    ColumnReverse = "column-reverse",
});
string_enum!(FlexWrap { NoWrap = "nowrap", Wrap = "wrap" });
string_enum!(Overflow { Visible = "visible", Hidden = "hidden", Scroll = "scroll" });
string_enum!(Align {
    Start = "flex-start",
    End = "flex-end",
    Center = "center",
    Stretch = "stretch",
    Baseline = "baseline",
    SpaceBetween = "space-between",
    SpaceAround = "space-around",
    SpaceEvenly = "space-evenly",
});
string_enum!(TextAlign { Left = "left", Center = "center", Right = "right", Justify = "justify" });

/// Element style. Layout properties follow CSS flexbox/grid; text properties inherit.
#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    // Layout
    pub display: Option<Display>,
    pub position: Option<Position>,
    pub left: Option<Dim>,
    pub top: Option<Dim>,
    pub right: Option<Dim>,
    pub bottom: Option<Dim>,
    pub width: Option<Dim>,
    pub height: Option<Dim>,
    pub min_width: Option<Dim>,
    pub min_height: Option<Dim>,
    pub max_width: Option<Dim>,
    pub max_height: Option<Dim>,
    pub padding: Option<Edges>,
    pub margin: Option<Edges>,
    pub gap: Option<f32>,
    pub flex_direction: Option<FlexDirection>,
    pub flex_wrap: Option<FlexWrap>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub justify_content: Option<Align>,
    pub align_items: Option<Align>,
    pub align_self: Option<Align>,
    /// Number of equal columns/rows; implies `display: "grid"`.
    pub grid_columns: Option<u16>,
    pub grid_rows: Option<u16>,
    pub overflow: Option<Overflow>,
    // Paint
    pub background: Option<Color>,
    pub radius: Option<f32>,
    pub border_width: Option<f32>,
    pub border_color: Option<Color>,
    pub opacity: Option<f32>,
    pub scale: Option<f32>,
    /// Rotation in degrees (not applied to text).
    pub rotate: Option<f32>,
    /// Slider fill and thumb colors; scrollbar color for scroll containers.
    pub fill_color: Option<Color>,
    pub thumb_color: Option<Color>,
    pub thumb_size: Option<f32>,
    pub scrollbar_color: Option<Color>,
    // Text (inherited)
    pub color: Option<Color>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub font_family: Option<String>,
    pub font_weight: Option<FontWeight>,
    pub italic: Option<bool>,
    pub text_align: Option<TextAlign>,
    pub text_shadow: Option<Shadow>,
}
