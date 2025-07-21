use std::env;

use clap::{value_parser, Args, Parser};

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::error::CliResult;

use super::{display_pal, Color, Palette, PaletteType, QualPaletteType, SeqPaletteType};

pub const DESC: &str = "\
Sequences are displayed on a single line and can be navigated with
up/down/left/right arrow keys and by scrolling.
They are progressively read into memory while navigating down.

Color palettes can be viewed with `st view -p/--list-pal` and also
configured (as described in this help page).
\
";

#[derive(Parser, Clone, Debug)]
pub struct ViewCommand {
    #[command(flatten)]
    pub general: GeneralViewArgs,

    #[command(flatten)]
    pub color: ColorArgs,

    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "General 'view' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct GeneralViewArgs {
    /// Length of IDs in characters. Longer IDs are truncated (default: 10 - 100 depending on ID length)
    #[arg(short, long, value_name = "CHARS", value_parser = value_parser!(u32).range(1..))]
    pub id_len: Option<u32>,

    /// Show descriptions along IDs if there is enough space.
    #[arg(long, short = 'd')]
    pub show_desc: bool,

    /// Print the sequence in bold letters.
    /// Bold text is always used with `--fg` if quality scores are present.
    #[arg(long, short = 'b')]
    pub bold: bool,

    /// Color the sequence (foreground) instead of instead of the background
    /// (and additionally printed in bold).
    /// The background is simultaneously colored by quality scores if present,
    /// unless `-Q/--no-qual` is used.
    #[arg(long = "fg")]
    pub foreground: bool,

    /// Ignore quality scores and color only by the sequence.
    #[arg(long, short = 'Q')]
    pub no_qual: bool,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Colors")]
pub struct ColorArgs {
    /// Show a list of all builtin palettes and exit.
    #[arg(short = 'p', long)]
    pub list_pal: bool,

    /// Color mapping for DNA.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// (builtin palettes: dna, dna-bright, dna-dark, pur-pyrimid, gc-at).
    #[arg(long, value_name = "PAL", default_value = "dna", value_parser = |p: &str| read_palette::<SeqPaletteType>(p, &DNA_PAL))]
    pub dna_pal: Palette,

    /// Color mapping for amino acids.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// (available: rasmol, polarity).
    #[arg(long, value_name = "PAL", default_value = "rasmol", value_parser = |p: &str| read_palette::<SeqPaletteType>(p, &PROTEIN_PAL))]
    pub aa_pal: Palette,

    /// Color scale to use for coloring according to base quality.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// Palette name or sequence of hex codes from low to high.
    #[arg(long, value_name = "PAL", default_value = "red-blue", value_parser = |p: &str| read_palette::<QualPaletteType>(p, &QUAL_SCALE))]
    pub qscale: Palette,

    /// Text colors used with background coloring. Specify as: <dark>,<bright>.
    /// Which one is used will be chosen depending on the brightness of
    /// the background.
    #[arg(long, value_name = "COLORS", default_value = "333333,eeeeee", value_parser = parse_textcols)]
    pub textcols: (Color, Color),

    /// Use 16M colors, not only 256. This has to be supported by the terminal.
    /// Useful if autorecognition fails.
    #[arg(short, long, value_name = "?")]
    pub truecolor: Option<bool>,
}

pub struct Palettes {
    pub dna: Palette,
    pub protein: Palette,
    pub qual: Palette,
}

impl ColorArgs {
    pub fn palettes(&self) -> Palettes {
        Palettes {
            dna: self.dna_pal.clone(),
            protein: self.aa_pal.clone(),
            qual: self.qscale.clone(),
        }
    }
}

pub const DNA_PAL: &[(&str, &str)] = &[
    (
        "dna",
        "A:ce0000,C:0000ce,G:ffde00,TU:00bb00,RYSWKMBDHVN:8f8f8f",
    ),
    (
        "dna-bright",
        "A:ff3333,C:3333ff,G:ffe747,TU:00db00,RYSWKMBDHVN:b8b8b8",
    ),
    (
        "dna-dark",
        "A:940000,C:00008f,G:9e8900,TU:006b00,RYSWKMBDHVN:8f8f8f",
    ),
    ("pur-pyrimid", "AGR:ff83fa,CTUY:25bdff"),
    ("gcat", "GCS:ff2b25,ATUW:ffd349"),
];

pub const PROTEIN_PAL: &[(&str, &str)] = &[
    ("rasmol", "DE:e60a0a,CM:e6e600,RK:145aff,ST:fa9600,FY:3232aa,NQ:00dcdc,G:ebebeb,LVI:0f820f,A:c8c8c8,W:b45Ab4,H:8282d2,P:dc9682"),
    ("polarity", "GAVLIFWMP:ffd349,STCYNQ:3dff51,DE:ff2220,KRH:1e35ff"),
];

pub const QUAL_SCALE: &[(&str, &str)] = &[("red-blue", "5:red,35:blue,40:darkblue")];

pub fn read_palette<T: PaletteType>(
    palette_str: &str,
    default_pal: &[(&str, &str)],
) -> Result<Palette, String> {
    if let Some((_, colors)) = default_pal.iter().find(|(n, _)| *n == palette_str) {
        if let Ok(cols) = T::parse_palette(colors) {
            return Ok(cols);
        }
    }
    T::parse_palette(palette_str)
}

pub fn print_palettes(fg: &(Color, Color), rgb: bool) -> CliResult<()> {
    eprintln!(concat!(
        "List of palette names their color mappings, which are in the form\n",
        "<symbol>:<colors>. Colors are specified as HEX codes. The colors can be\n",
        "directly configured using --dna-pal / --aa-pal / --qscale. These options\n",
        "accept both palette names and color mappings.\n"
    ));
    eprintln!("\nDNA\n===");
    display_pal::<SeqPaletteType>(DNA_PAL, fg, rgb)?;
    eprintln!("\nProtein\n=======");
    display_pal::<SeqPaletteType>(PROTEIN_PAL, fg, rgb)?;
    eprintln!("\nQuality scores\n==============");
    display_pal::<QualPaletteType>(QUAL_SCALE, fg, rgb)?;
    Ok(())
}

pub fn parse_textcols(text: &str) -> Result<(Color, Color), String> {
    let mut s = text.split(',').map(|s| s.to_string());
    let dark = s.next().unwrap();
    if let Some(bright) = s.next() {
        if s.next().is_none() {
            return Ok((Color::from_str(&dark)?, Color::from_str(&bright)?));
        }
    }
    Err(format!(
        "Invalid text color specification: '{text}'. Must be '<dark>,<bright>'"
    ))
}

pub fn has_truecolor() -> bool {
    env::var("COLORTERM")
        .map(|s| s == "truecolor" || s == "24bit")
        .unwrap_or(false)
    // 256-color: $TERM contains 256
    // see also https://github.com/chalk/supports-color/blob/master/index.js
}

pub fn has_utf8() -> bool {
    // TODO: reasonable?
    cfg!(target_family = "unix")
        && env::var("LANG")
            .unwrap_or_else(|_| "".to_string())
            .to_ascii_lowercase()
            .contains("utf-8")
        || cfg!(target_os = "windows")
}
