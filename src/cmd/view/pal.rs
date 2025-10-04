use std::io::{stdout, Write as _};

use crossterm::{
    execute,
    style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use enterpolation::{linear::Linear, Merge, Signal};
use palette::{
    convert::FromColorUnclamped,
    rgb::{self, Rgb},
    FromColor, Hsv, Mix, Srgb,
};
use vec_map::VecMap;

use crate::error::CliResult;

use super::{choose_fg, parse_color, Color};

pub type Palette = VecMap<Color>;

// Necessary since LinSrgb doesn't implement `Merge` automatically, but also to
// get the correct circular blending.
// TODO: Lch?
#[derive(Clone, Copy, Debug)]
pub(super) struct Adapter<T>(T);

impl<T: Mix> Merge<T::Scalar> for Adapter<T> {
    fn merge(self, to: Self, factor: T::Scalar) -> Self {
        Adapter(self.0.mix(to.0, factor))
    }
}

pub trait PaletteType {
    fn parse_palette(color_str: &str) -> Result<Palette, String>;

    fn parse_pal_mapping(color_str: &str) -> Result<Vec<(String, Srgb<u8>)>, String> {
        let mut out = Vec::new();
        for c in color_str.split(',') {
            let c = c.trim();
            if c.is_empty() {
                continue;
            }
            let mut s = c.split(':');
            let symbols = s.next().unwrap().trim().to_string();
            if let Some(col) = s.next() {
                let col = parse_color(col)?;
                out.push((symbols, col))
            } else {
                return Err(format!(
                    "Invalid color mapping: '{c}'. Use 'WHAT:rrggbb' \
                    for mapping WHAT to a given color (in hex code)"
                ));
            }
        }
        Ok(out)
    }

    fn display_palette(
        name: &str,
        colors_str: &str,
        textcols: &(Color, Color),
        rgb: bool,
    ) -> CliResult<()>;
}

#[derive(Clone, Debug)]
pub struct SeqPaletteType;

impl PaletteType for SeqPaletteType {
    fn parse_palette(color_str: &str) -> Result<Palette, String> {
        let mut out = VecMap::new();
        for (symbols, color) in Self::parse_pal_mapping(color_str)? {
            for s in symbols.as_bytes() {
                let s = if s.is_ascii_lowercase() {
                    s.to_ascii_uppercase()
                } else {
                    *s
                };
                out.insert(s as usize, Color::from_rgb(color));
                out.insert(s.to_ascii_lowercase() as usize, Color::from_rgb(color));
            }
        }
        Ok(out)
    }

    fn display_palette(
        name: &str,
        colors_str: &str,
        textcols: &(Color, Color),
        rgb: bool,
    ) -> CliResult<()> {
        let mut out = stdout().lock();
        execute!(out, ResetColor)?;
        write!(stdout(), "{name:<12}")?;
        for (symbols, color) in Self::parse_pal_mapping(colors_str)? {
            write!(out, "{symbols}:")?;
            let bg = Color::from_rgb(color);
            let chosen = choose_fg(&textcols.0, &textcols.1, &bg);
            execute!(
                out,
                SetForegroundColor(chosen.to_crossterm(rgb)),
                SetBackgroundColor(bg.to_crossterm(rgb))
            )?;
            // hex code
            let c: u32 = color.into_u32::<rgb::channels::Rgba>();
            write!(out, "{:06x}", c >> 8)?;
            execute!(out, ResetColor)?;
            write!(out, ",")?;
        }
        execute!(out, ResetColor)?;
        writeln!(out)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct QualPaletteType;

impl PaletteType for QualPaletteType {
    fn parse_palette(color_str: &str) -> Result<Palette, String> {
        let mut elements = vec![];
        let mut knots = vec![];
        for (qual, color) in Self::parse_pal_mapping(color_str)? {
            // parse quality score
            let qual: u8 = qual.parse().map_err(|_| {
                format!(
                    "Invalid quality code: '{qual}'. Expecting Phred scores, \
                    usually between 0 and ~45"
                )
            })?;
            knots.push(qual as f32);
            let col = Adapter(Hsv::from_color(color.into_linear()));
            elements.push(col);
        }
        let mut out = VecMap::new();
        let gradient = Linear::builder()
            .elements(&elements)
            .knots(&knots)
            .build()
            .unwrap();
        for qual in 0..96 {
            let col = gradient.eval(qual as f32);
            let col = Rgb::from_color_unclamped(col.0);
            out.insert(qual, Color::from_rgb(col.into()));
        }
        Ok(out)
    }

    fn display_palette(
        name: &str,
        colors_str: &str,
        textcols: &(Color, Color),
        rgb: bool,
    ) -> CliResult<()> {
        let mut out = stdout().lock();
        SeqPaletteType::display_palette(name, colors_str, textcols, rgb)?;
        write!(out, "   [")?;
        let colmap = Self::parse_palette(colors_str)?;
        execute!(out, ResetColor)?;
        for qual in (2..47).step_by(2) {
            let bg = &colmap[qual];
            let chosen = choose_fg(&textcols.0, &textcols.1, bg);
            execute!(
                out,
                SetForegroundColor(chosen.to_crossterm(rgb)),
                SetBackgroundColor(bg.to_crossterm(rgb)),
                Print(qual),
                Print(' ')
            )?;
        }
        execute!(out, ResetColor)?;
        writeln!(out, "]")?;
        Ok(())
    }
}

pub fn display_pal<T>(pal: &[(&str, &str)], textcols: &(Color, Color), rgb: bool) -> CliResult<()>
where
    T: PaletteType,
{
    for (name, colors_str) in pal {
        T::display_palette(name, colors_str, textcols, rgb)?;
    }
    Ok(())
}
