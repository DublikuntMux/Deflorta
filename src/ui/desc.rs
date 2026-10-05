//! Serialized UI tree and command types produced by the JS runtime.

use std::collections::HashMap;

use serde::Deserialize;

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
    Volume {
        channel: String,
        value: f32,
    },
    RevealAll,
    Fullscreen {
        on: bool,
    },
    Quit,
}

fn default_true() -> bool {
    true
}

fn default_one() -> f32 {
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
    #[serde(default)]
    pub clear_color: Option<Color>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Box,
    Text,
    Image,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Fill,
    Cover,
    Contain,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeDesc {
    pub t: NodeKind,
    pub key: Option<String>,
    #[serde(default)]
    pub style: Style,
    /// Paint-only overrides applied while the pointer is over the element.
    pub hover: Option<Style>,
    pub on_click: Option<u32>,
    #[serde(default)]
    pub children: Vec<NodeDesc>,
    pub text: Option<String>,
    /// Typewriter speed in characters per second.
    pub cps: Option<f32>,
    pub src: Option<String>,
    #[serde(default)]
    pub fit: Fit,
    /// Fraction of the element's size placed at its layout position, e.g. [0.5, 1] for bottom-center.
    pub anchor: Option<[f32; 2]>,
    pub enter: Option<AnimDesc>,
    pub exit: Option<AnimDesc>,
}

/// Animation endpoint: `enter` gives starting values, `exit` final values.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct AnimDesc {
    pub dur: f32,
    pub opacity: Option<f32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub scale: Option<f32>,
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
            Raw::Number(v) => return Ok(Dim::Px(v)),
            Raw::Text(text) => text,
        };
        let s = text.trim();
        let number = |v: &str| v.trim().parse::<f32>().map_err(serde::de::Error::custom);
        if s == "auto" {
            Ok(Dim::Auto)
        } else if let Some(p) = s.strip_suffix('%') {
            Ok(Dim::Percent(number(p)? / 100.0))
        } else if let Some(p) = s.strip_suffix("px") {
            Ok(Dim::Px(number(p)?))
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
    pub fn trbl(self) -> [f32; 4] {
        match self {
            Edges::All(v) => [v; 4],
            Edges::Two([v, h]) => [v, h, v, h],
            Edges::Four(e) => e,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color(pub [f32; 4]);

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid color '{s}'")))
    }
}

impl Color {
    pub const WHITE: Color = Color([1.0, 1.0, 1.0, 1.0]);

    /// Parses `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` (sRGB).
    pub fn parse(s: &str) -> Option<Color> {
        let hex = s.trim().strip_prefix('#')?;
        let digits: Vec<u8> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect::<Option<_>>()?;
        let channels: Vec<u8> = match digits.len() {
            3 | 4 => digits.iter().map(|d| d * 17).collect(),
            6 | 8 => digits.chunks(2).map(|c| c[0] * 16 + c[1]).collect(),
            _ => return None,
        };
        let a = channels.get(3).copied().unwrap_or(255);
        Some(Color([
            channels[0] as f32 / 255.0,
            channels[1] as f32 / 255.0,
            channels[2] as f32 / 255.0,
            a as f32 / 255.0,
        ]))
    }

    pub fn with_alpha_mul(self, a: f32) -> Color {
        let [r, g, b, alpha] = self.0;
        Color([r, g, b, alpha * a])
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        self.0.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Shadow {
    pub color: Color,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(untagged)]
pub enum FontWeight {
    Number(u16),
    Name(FontWeightName),
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FontWeightName {
    Normal,
    Bold,
}

impl FontWeight {
    pub fn value(self) -> u16 {
        match self {
            FontWeight::Number(n) => n,
            FontWeight::Name(FontWeightName::Normal) => 400,
            FontWeight::Name(FontWeightName::Bold) => 700,
        }
    }
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident = $text:literal),* $(,)? }) => {
        #[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
        pub enum $name { $(#[serde(rename = $text)] $variant),* }
    };
}

string_enum!(Display { Flex = "flex", None = "none" });
string_enum!(Position { Relative = "relative", Absolute = "absolute" });
string_enum!(FlexDirection { Row = "row", Column = "column", RowReverse = "row-reverse", ColumnReverse = "column-reverse" });
string_enum!(FlexWrap { NoWrap = "nowrap", Wrap = "wrap" });
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

/// Element style. Layout properties follow CSS flexbox; text properties inherit.
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
    // Paint
    pub background: Option<Color>,
    pub radius: Option<f32>,
    pub border_width: Option<f32>,
    pub border_color: Option<Color>,
    pub opacity: Option<f32>,
    pub scale: Option<f32>,
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
