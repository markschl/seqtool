use std::str::FromStr;

use palette::{Srgb, named, white_point::D65};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ColorSource {
    Seq,
    Qual,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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

    pub fn to_ratatui(self, rgb: bool) -> ratatui::style::Color {
        if rgb {
            ratatui::style::Color::Rgb(self.rgb.0, self.rgb.1, self.rgb.2)
        } else {
            ratatui::style::Color::Indexed(self.ansi.0)
        }
    }

    pub fn to_crossterm(self, rgb: bool) -> crossterm::style::Color {
        if rgb {
            crossterm::style::Color::Rgb {
                r: self.rgb.0,
                g: self.rgb.1,
                b: self.rgb.2,
            }
        } else {
            crossterm::style::Color::AnsiValue(self.ansi.0)
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct AnsiColor(u8);

impl From<Srgb<u8>> for AnsiColor {
    fn from(c: Srgb<u8>) -> Self {
        // Simple conversion adapted from the colorsys.rs crate, not using the grayscale ramp
        let to_ansi = |c| if c < 75 { 0 } else { (c - 35) / 40 };
        Self(to_ansi(c.red) * 6 * 6 + to_ansi(c.green) * 6 + to_ansi(c.blue) + 16)
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
        *fg_dark
    } else {
        *fg_bright
    }
}
