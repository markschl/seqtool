use std::str::FromStr;

use ansi_colours::ansi256_from_rgb;
use palette::white_point::D65;
use palette::{named, Srgb};

pub enum ColorSource {
    Symbol,
    Qual,
}

pub enum ColorMode {
    Fg,
    Bg,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Color {
    rgb: (u8, u8, u8),
    ansi: AnsiColor,
}

impl Color {
    pub fn from_rgb(c: Srgb<u8>) -> Self {
        Self {
            rgb: (c.red, c.green, c.blue),
            ansi: c.into(),
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        parse_color(s).map(Self::from_rgb)
    }

    pub fn to_termcolor(&self, rgb: bool) -> termcolor::Color {
        if rgb {
            termcolor::Color::Rgb(self.rgb.0, self.rgb.1, self.rgb.2)
        } else {
            termcolor::Color::Ansi256(self.ansi.0)
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct AnsiColor(u8);

impl From<Srgb<u8>> for AnsiColor {
    fn from(c: Srgb<u8>) -> Self {
        Self(ansi256_from_rgb((c.red, c.green, c.blue)))
    }
}

pub fn parse_color(s: &str) -> Result<Srgb<u8>, String> {
    named::from_str(s).or_else(|| Srgb::from_str(s).ok())
        .ok_or_else(|| format!("Invalid color code: '{s}'. The colors must be in Hex format (rrggbb) or a name (e.g. 'cyan')"))
}

/// chooses the optimal text color based on the brightness/darkness of the background color
pub fn choose_fg(fg_dark: &Color, fg_bright: &Color, bg_col: &Color) -> Color {
    let dark_l = palette::Lab::<D65, _>::from(fg_dark.rgb).l as f32;
    let bright_l = palette::Lab::<D65, _>::from(fg_bright.rgb).l as f32;
    let bg = palette::Lab::<D65, _>::from(bg_col.rgb).l as f32;
    if (bright_l - bg) / (bright_l - dark_l) < 0.3 {
        fg_dark.clone()
    } else {
        fg_bright.clone()
    }
}
