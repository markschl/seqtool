use std::io::Write as _;

use enterpolation::{linear::Linear, Generator, Merge};
use palette::convert::FromColorUnclamped;
use palette::rgb::{self, Rgb};
use palette::{FromColor, Hsv, Mix, Srgb};
use termcolor::{self, WriteColor};
use vec_map::VecMap;

use crate::error::CliResult;

use super::{choose_fg, parse_color, Color};

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
    fn parse_palette(color_str: &str) -> Result<VecMap<Color>, String>;

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
                    "Invalid color mapping: '{}'. Use 'WHAT:rrggbb' \
                    for mapping WHAT to a given color (in hex code)",
                    c
                ));
            }
        }
        Ok(out)
    }

    fn display_palette(
        colors_str: &str,
        writer: &mut termcolor::StandardStream,
        textcols: &(Color, Color),
        rgb: bool,
    ) -> CliResult<()>;
}

#[derive(Clone, Debug)]
pub struct SeqPaletteType;

impl PaletteType for SeqPaletteType {
    fn parse_palette(color_str: &str) -> Result<VecMap<Color>, String> {
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
        colors_str: &str,
        writer: &mut termcolor::StandardStream,
        textcols: &(Color, Color),
        rgb: bool,
    ) -> CliResult<()> {
        let default_spec = termcolor::ColorSpec::new();
        let mut colspec = termcolor::ColorSpec::new();

        writer.set_color(&default_spec)?;
        for (symbols, color) in Self::parse_pal_mapping(colors_str)? {
            write!(writer, "{}:", symbols)?;
            let bg = Color::from_rgb(color);
            let chosen = choose_fg(&textcols.0, &textcols.1, &bg).to_termcolor(rgb);
            colspec.set_fg(Some(chosen));
            colspec.set_bg(Some(bg.to_termcolor(rgb)));
            writer.set_color(&colspec)?;
            let c: u32 = color.into_u32::<rgb::channels::Rgba>();
            write!(writer, "{:06x}", c >> 8)?;
            writer.set_color(&default_spec)?;
            write!(writer, ",")?;
        }
        writer.set_color(&default_spec)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct QualPaletteType;

impl PaletteType for QualPaletteType {
    fn parse_palette(color_str: &str) -> Result<VecMap<Color>, String> {
        let mut elements = vec![];
        let mut knots = vec![];
        for (qual, color) in Self::parse_pal_mapping(color_str)? {
            // parse quality score
            let qual: u8 = qual.parse().map_err(|_| {
                format!(
                    "Invalid quality code: '{}'. Expecting Phred scores, \
                    usually between 0 and ~42",
                    qual
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
            let col = gradient.gen(qual as f32);
            let col = Rgb::from_color_unclamped(col.0);
            out.insert(qual, Color::from_rgb(col.into()));
        }
        Ok(out)
    }

    fn display_palette(
        colors_str: &str,
        writer: &mut termcolor::StandardStream,
        textcols: &(Color, Color),
        rgb: bool,
    ) -> CliResult<()> {
        SeqPaletteType::display_palette(colors_str, writer, textcols, rgb)?;
        write!(writer, "   [")?;

        let colmap = Self::parse_palette(colors_str)?;
        let default_spec = termcolor::ColorSpec::new();
        let mut colspec = termcolor::ColorSpec::new();
        writer.set_color(&default_spec)?;
        for qual in (2..43).step_by(2) {
            let bg = &colmap[qual];
            let chosen = choose_fg(&textcols.0, &textcols.1, bg).to_termcolor(rgb);
            colspec.set_fg(Some(chosen));
            colspec.set_bg(Some(bg.to_termcolor(rgb)));
            writer.set_color(&colspec)?;
            write!(writer, "{} ", qual)?;
        }
        writer.set_color(&default_spec)?;
        write!(writer, "]")?;
        Ok(())
    }
}

pub fn display_pal<T>(
    pal: &[(&str, &str)],
    writer: &mut termcolor::StandardStream,
    textcols: &(Color, Color),
    rgb: bool,
) -> CliResult<()>
where
    T: PaletteType,
{
    for (name, colors_str) in pal {
        write!(writer, "{:<12}", name)?;
        T::display_palette(colors_str, writer, textcols, rgb)?;
        writeln!(writer)?;
    }
    Ok(())
}
