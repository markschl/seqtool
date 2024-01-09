use std::env;

use clap::{value_parser, Args, Parser};
use vec_map::VecMap;

use crate::{cli::CommonArgs, error::CliResult};

use super::{Color, PaletteType, QualPaletteType, SeqPaletteType, SimplePal};

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct ViewCommand {
    #[command(flatten)]
    pub general: GeneralViewArgs,

    #[cfg(target_family = "unix")]
    #[command(flatten)]
    pub pager: PagerArgs,

    #[command(flatten)]
    pub color: ColorArgs,

    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "General command options")]
pub struct GeneralViewArgs {
    /// Length of IDs in characters. Longer IDs are truncated (default: 10 - 100 depending on ID length)
    #[arg(short, long, value_name = "CHARS", value_parser = value_parser!(u32).range(1..))]
    pub id_len: Option<u32>,

    /// Show descriptions along IDs if there is enough space.
    #[arg(short = 'd', long)]
    pub show_desc: bool,

    /// Color base / amino acid letters instead of background.
    /// If base qualities are present, background coloration is shown,
    /// and the foreground scheme will be 'dna-bright' (change with --dna-pal).
    #[arg(long = "fg")]
    pub foreground: bool,

    /// View only the top <N> sequences without pager. Automatic handoff to a
    /// pager is only available in UNIX (turn off with --no-pager).
    #[arg(short, long, default_value_t = 100, value_name = "N")]
    pub n_max: u64,
}

#[cfg(target_family = "unix")]
#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "View pager (UNIX only)")]
pub struct PagerArgs {
    /// Disable paged display
    #[arg(long)]
    pub no_pager: bool,

    /// Pager command to use.
    #[arg(long, default_value = "less -RS", env = "ST_PAGER")]
    pub pager: Option<String>,

    /// Break lines in pager, disabling 'horizontal scrolling'.
    /// Equivalent to --pager 'less -R'
    #[arg(short, long, name = "break")]
    pub break_: bool,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Colors")]
pub struct ColorArgs {
    /// Show a list of all builtin palettes and exit.
    #[arg(long)]
    pub list_pal: bool,

    /// Color mapping for DNA.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// (builtin palettes: dna, dna-bright, dna-dark, pur-pyrimid, gc-at).
    #[arg(long, value_name = "PAL", default_value = "dna", value_parser = |p: &str| read_palette::<SeqPaletteType>(p, &DNA_PAL))]
    pub dna_pal: VecMap<Color>,

    /// Color mapping for amino acids.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// (available: rasmol, polarity).
    #[arg(long, value_name = "PAL", default_value = "rasmol", value_parser = |p: &str| read_palette::<SeqPaletteType>(p, &PROTEIN_PAL))]
    pub aa_pal: VecMap<Color>,

    /// Color scale to use for coloring according to base quality.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// Palette name or sequence of hex codes from low to high.
    #[arg(long, value_name = "PAL", default_value = "red-blue", value_parser = |p: &str| read_palette::<QualPaletteType>(p, &QUAL_SCALE))]
    pub qscale: VecMap<Color>,

    /// Text colors used with background coloring. Specify as: <dark>,<bright>.
    /// Which one is used will be chosen depending on the brightness of
    /// the background.
    #[arg(long, value_name = "COLORS", default_value = "333333,eeeeee", value_parser = parse_textcols)]
    pub textcols: (Color, Color),

    /// Use 16M colors, not only 256. This has to be supported by the terminal.
    /// Useful if autorecognition fails.
    #[arg(short, long)]
    pub truecolor: Option<bool>,
}

lazy_static! {
    pub static ref DNA_PAL: SimplePal = SimplePal::default()
        .add("dna", "A:ce0000,C:0000ce,G:ffde00,TU:00bb00,RYSWKMBDHVN:8f8f8f")
        .add("dna-bright", "A:ff3333,C:3333ff,G:ffe747,TU:00db00,RYSWKMBDHVN:b8b8b8")
        .add("dna-dark", "A:940000,C:00008f,G:9e8900,TU:006b00,RYSWKMBDHVN:8f8f8f")
        .add("pur-pyrimid", "AGR:ff83fa,CTUY:25bdff")
        .add("gcat", "GCS:ff2b25,ATUW:ffd349");

    pub static ref PROTEIN_PAL: SimplePal = SimplePal::default()
        .add("rasmol", "DE:e60a0a,CM:e6e600,RK:145aff,ST:fa9600,FY:3232aa,NQ:00dcdc,G:ebebeb,LVI:0f820f,A:c8c8c8,W:b45Ab4,H:8282d2,P:dc9682")
        .add("polarity", "GAVLIFWMP:ffd349,STCYNQ:3dff51,DE:ff2220,KRH:1e35ff");

    pub static ref QUAL_SCALE: SimplePal = SimplePal::default()
        .add("red-blue", "5:red,35:blue,40:darkblue");
}

pub fn read_palette<T: PaletteType>(
    s: &str,
    default_pal: &SimplePal,
) -> Result<VecMap<Color>, String> {
    if let Some(colors) = default_pal.get(s) {
        if let Ok(cols) = T::parse_palette(colors) {
            return Ok(cols);
        }
    }
    T::parse_palette(s)
}

pub fn print_palettes(fg: &(Color, Color), rgb: bool) -> CliResult<()> {
    eprintln!(concat!(
        "List of palette names their color mappings, which are in the form\n",
        "<symbol>:<colors>. Colors are specified as HEX codes. The colors can be\n",
        "directly configured using --dna-pal / --aa-pal / --qscale. These options\n",
        "accept both palette names and color mappings.\n"
    ));
    eprintln!("\nDNA\n===");
    let mut w = termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto);
    DNA_PAL.display_pal::<SeqPaletteType>(&mut w, fg, rgb)?;
    eprintln!("\nProtein\n=======");
    PROTEIN_PAL.display_pal::<SeqPaletteType>(&mut w, fg, rgb)?;
    eprintln!("\nQuality scores\n==============");
    QUAL_SCALE.display_pal::<QualPaletteType>(&mut w, fg, rgb)?;
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
        "Invalid text color specification: '{}'. Must be '<dark>,<bright>'",
        text
    ))
}

pub fn has_truecolor() -> bool {
    env::var("COLORTERM")
        .map(|s| s == "truecolor" || s == "24bit")
        .unwrap_or(false)
    // 256-color: $TERM contains 256
    // see also https://github.com/chalk/supports-color/blob/master/index.js
}
