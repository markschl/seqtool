use std::cmp::{max, min};
use std::env::var;
use std::io::{self, Write as _};
use std::str::{self, FromStr};

use ansi_colours::ansi256_from_rgb;
use clap::{value_parser, Args, Parser};
use enterpolation::{linear::Linear, Generator, Merge};
use palette::{
    convert::FromColorUnclamped,
    named,
    rgb::{self, Rgb},
    white_point::D65,
    FromColor, Hsv, Mix, Srgb,
};
use termcolor::{self, WriteColor};
use vec_map::VecMap;

#[cfg(target_family = "unix")]
use pager::Pager;

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::seqtype::{guess_seqtype, SeqType};
use crate::io::{qual_to_prob, QualFormat};
use crate::opt::CommonArgs;

/// Colored sequence view
/// View biological sequences, colored by base / amino acid, or by sequence quality.
/// The output is automatically forwarded to the 'less' pager on UNIX.
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
    id_len: Option<u32>,

    /// Show descriptions along IDs if there is enough space.
    #[arg(short, long)]
    show_desc: bool,

    /// Color base / amino acid letters instead of background.
    /// If base qualities are present, background coloration is shown,
    /// and the foreground scheme will be 'dna-bright' (change with --dna-pal).
    #[arg(short, long)]
    foreground: bool,

    /// View only the top <N> sequences without pager. Automatic handoff to a
    /// pager is only available in UNIX (turn off with --no-pager).
    #[arg(short, long, default_value_t = 100, value_name = "N")]
    n_max: u64,
}

#[cfg(target_family = "unix")]
#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "View pager (UNIX only)")]
pub struct PagerArgs {
    /// Disable paged display
    #[arg(long)]
    no_pager: bool,

    /// Pager command to use.
    #[arg(long, default_value = "less -RS", env = "ST_PAGER")]
    pager: Option<String>,

    /// Break lines in pager, disabling 'horizontal scrolling'.
    /// Equivalent to --pager 'less -R'
    #[arg(short, long, name = "break")]
    break_: bool,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Colors")]
pub struct ColorArgs {
    /// Show a list of all builtin palettes and exit.
    #[arg(long)]
    list_pal: bool,

    /// Color mapping for DNA.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// (builtin palettes: dna, dna-bright, dna-dark, pur-pyrimid, gc-at).
    #[arg(long, value_name = "PAL", default_value = "dna", value_parser = |p: &str| read_palette::<SeqPaletteType>(p, &DNA_PAL))]
    dna_pal: VecMap<Color>,

    /// Color mapping for amino acids.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// (available: rasmol, polarity).
    #[arg(long, value_name = "PAL", default_value = "rasmol", value_parser = |p: &str| read_palette::<SeqPaletteType>(p, &PROTEIN_PAL))]
    aa_pal: VecMap<Color>,

    /// Color scale to use for coloring according to base quality.
    /// Palette name (hex code, CSS/SVG color name)
    /// or list of 'base1:rrggbb,base2:rrggbb,...'
    /// Palette name or sequence of hex codes from low to high.
    #[arg(long, value_name = "PAL", default_value = "red-blue", value_parser = |p: &str| read_palette::<QualPaletteType>(p, &QUAL_SCALE))]
    qscale: VecMap<Color>,

    /// Text colors used with background coloring. Specify as: <dark>,<bright>.
    /// Which one is used will be chosen depending on the brightness of
    /// the background.
    #[arg(long, value_name = "COLORS", default_value = "333333,eeeeee", value_parser = parse_textcols)]
    textcols: (Color, Color),

    /// Use 16M colors, not only 256. This has to be supported by the terminal.
    /// Useful if autorecognition fails.
    #[arg(short, long)]
    truecolor: Option<bool>,
}

lazy_static! {
    static ref DNA_PAL: SimplePal = SimplePal::default()
        .add("dna", "A:ce0000,C:0000ce,G:ffde00,TU:00bb00,RYSWKMBDHVN:8f8f8f")
        .add("dna-bright", "A:ff3333,C:3333ff,G:ffe747,TU:00db00,RYSWKMBDHVN:b8b8b8")
        .add("dna-dark", "A:940000,C:00008f,G:9e8900,TU:006b00,RYSWKMBDHVN:8f8f8f")
        .add("pur-pyrimid", "AGR:ff83fa,CTUY:25bdff")
        .add("gcat", "GCS:ff2b25,ATUW:ffd349");

    static ref PROTEIN_PAL: SimplePal = SimplePal::default()
        .add("rasmol", "DE:e60a0a,CM:e6e600,RK:145aff,ST:fa9600,FY:3232aa,NQ:00dcdc,G:ebebeb,LVI:0f820f,A:c8c8c8,W:b45Ab4,H:8282d2,P:dc9682")
        .add("polarity", "GAVLIFWMP:ffd349,STCYNQ:3dff51,DE:ff2220,KRH:1e35ff");

    static ref QUAL_SCALE: SimplePal = SimplePal::default()
        .add("red-blue", "5:red,35:blue,40:darkblue");
}

pub fn run(cfg: Config, args: &ViewCommand) -> CliResult<()> {
    let truecolor = args.color.truecolor.unwrap_or_else(|| has_truecolor());
    if args.color.list_pal {
        print_palettes(&args.color.textcols, truecolor)?;
        return Ok(());
    }

    // set up pager and determine sequence limit
    #[cfg(target_family = "unix")]
    #[allow(unused_variables)]
    let n_max = {
        setup_pager(
            args.pager.pager.as_deref(),
            args.pager.break_,
            args.pager.no_pager,
        );
        if args.pager.no_pager {
            Some(args.general.n_max)
        } else {
            None
        }
    };
    #[cfg(not(target_family = "unix"))]
    let n_max = Some(args.general.n_max);

    // setup colors
    let mut writer = ColorWriter::new()
        .truecolor(truecolor)
        .textcols(args.color.textcols.0.clone(), args.color.textcols.1.clone())?;

    if cfg.input_opts()[0].format.has_qual() {
        writer.set(ColorSource::Qual, ColorMode::Bg);
        if args.general.foreground {
            writer.set(ColorSource::Symbol, ColorMode::Fg);
        }
    } else if args.general.foreground {
        writer.set(ColorSource::Symbol, ColorMode::Fg);
    } else {
        writer.set(ColorSource::Symbol, ColorMode::Bg);
    }

    // terminal encoding
    // TODO: reasonable?
    let utf8 = cfg!(target_family = "unix")
        && var("LANG")
            .unwrap_or_else(|_| "".to_string())
            .to_ascii_lowercase()
            .contains("utf-8")
        || cfg!(target_os = "windows");

    // run
    let mut i: u64 = 0;
    let mut id_len = args.general.id_len.unwrap_or(0);
    // TODO: not actually required, currently
    cfg.with_vars(None, |vars| {
        cfg.read(vars, |record, vars| {
            if let Some(n) = n_max {
                if i >= n {
                    return Ok(false);
                }
            }

            // write seq. ids / desc

            let (id, desc) = record.id_desc_bytes();

            if id_len == 0 {
                // determine ID width of first ID
                id_len = min(100, max(10, std::str::from_utf8(id)?.chars().count() + 3)) as u32;
            }

            write_id(
                id,
                desc,
                &mut writer,
                id_len as usize,
                args.general.show_desc,
                utf8,
            )?;

            // write seq

            if let Some(qual) = record.qual() {
                // If quality scores are present, color by them

                let mut qual_iter = qual.iter();

                let mut prob = 0.;
                let mut seqlen = 0;

                for seq in record.seq_segments() {
                    if !writer.initialized() {
                        // TODO: initializing with first sequence line -> enough?
                        writer.init(
                            seq,
                            args.color.dna_pal.clone(),
                            args.color.aa_pal.clone(),
                            args.color.qscale.clone(),
                        )?;
                    }

                    for &symbol in seq {
                        let q = *qual_iter
                            .next()
                            .expect("BUG: Sequence length != Length of qual.");

                        let phred = vars.data().qual_converter.convert(q, QualFormat::Phred)?;

                        writer.write_symbol(symbol, Some(phred))?;

                        prob += qual_to_prob(phred);
                    }

                    seqlen += seq.len();
                }

                writer.reset()?;

                let rate = prob / seqlen as f64;
                if prob < 0.001 {
                    write!(writer, " err: {:>3.2e} ({:.4e} / pos.)", prob, rate)?;
                } else {
                    write!(writer, " err: {:>2.3} ({:.4} / pos.)", prob, rate)?;
                }
            } else {
                // if no quality scores, color by sequence
                for seq in record.seq_segments() {
                    if !writer.initialized() {
                        writer.init(
                            seq,
                            args.color.dna_pal.clone(),
                            args.color.aa_pal.clone(),
                            args.color.qscale.clone(),
                        )?;
                    }

                    for &symbol in seq {
                        writer.write_symbol(symbol, None)?;
                    }
                }

                writer.reset()?;
            }

            writer.write_all(b"\n")?;

            i += 1;
            Ok(true)
        })
    })
}

#[cfg(target_family = "unix")]
fn setup_pager(cmd: Option<&str>, break_lines: bool, no_pager: bool) {
    if !no_pager {
        let env_pager = var("ST_PAGER");
        let pager = env_pager
            .as_ref()
            .ok()
            .map(|s| s.as_str())
            .or(cmd)
            .unwrap_or(if break_lines { "less -R" } else { "less -RS" });
        Pager::with_pager(pager).setup();
    }
}

fn write_id<W: io::Write>(
    id: &[u8],
    desc: Option<&[u8]>,
    mut writer: W,
    total_len: usize,
    show_desc: bool,
    utf8: bool,
) -> CliResult<()> {
    let id = str::from_utf8(id)?;

    let ellipsis = if utf8 { '…' } else { ' ' };
    let id_len = id.chars().count();
    if id_len > total_len {
        write!(writer, "{}{} ", &id[..total_len - 1], ellipsis)?;
    } else {
        let rest = total_len - id_len;

        if show_desc && rest >= 3 {
            if let Some(d) = desc {
                let d = str::from_utf8(d)?;

                if d.chars().count() > rest {
                    write!(writer, "{} {}{} ", id, &d[..rest - 2], ellipsis)?;
                } else {
                    write!(writer, "{} {:<2$} ", id, d, rest)?;
                }
                return Ok(());
            }
        }

        write!(writer, "{:<1$} ", id, total_len)?;
    }
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

fn read_palette<T: PaletteType>(s: &str, default_pal: &SimplePal) -> Result<VecMap<Color>, String> {
    if let Some(colors) = default_pal.get(s) {
        if let Ok(cols) = T::parse_palette(colors) {
            return Ok(cols);
        }
    }
    T::parse_palette(s)
}

fn print_palettes(fg: &(Color, Color), rgb: bool) -> CliResult<()> {
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
        // TODO: needed?
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

#[derive(Clone, Debug, Default)]
struct SimplePal(Vec<(String, String)>);

impl SimplePal {
    fn add(mut self, name: &str, colors_str: &str) -> Self {
        self.0.push((name.to_string(), colors_str.to_string()));
        self
    }

    fn members(&self) -> &[(String, String)] {
        &self.0
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c.as_str())
    }

    fn display_pal<T>(
        &self,
        writer: &mut termcolor::StandardStream,
        textcols: &(Color, Color),
        rgb: bool,
    ) -> CliResult<()>
    where
        T: PaletteType,
    {
        for (name, colors_str) in self.members() {
            write!(writer, "{:<12}", name)?;
            T::display_palette(colors_str, writer, textcols, rgb)?;
            writeln!(writer, "")?;
        }
        Ok(())
    }
}

enum ColorSource {
    Symbol,
    Qual,
}

enum ColorMode {
    Fg,
    Bg,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Color {
    rgb: (u8, u8, u8),
    ansi: AnsiColor,
}

impl Color {
    fn from_rgb(c: Srgb<u8>) -> Self {
        Self {
            rgb: (c.red, c.green, c.blue),
            ansi: c.into(),
        }
    }

    fn from_str(s: &str) -> Result<Self, String> {
        parse_color(s).map(Self::from_rgb)
    }

    fn to_termcolor(&self, rgb: bool) -> termcolor::Color {
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

// Necessary since LinSrgb doesn't implement `Merge` automatically, but also to
// get the correct circular blending.
// TODO: Lch?
#[derive(Clone, Copy, Debug)]
struct Adapter<T>(T);

impl<T: Mix> Merge<T::Scalar> for Adapter<T> {
    fn merge(self, to: Self, factor: T::Scalar) -> Self {
        Adapter(self.0.mix(to.0, factor))
    }
}

struct ColorWriter {
    writer: termcolor::StandardStream,
    fg_map: Option<(VecMap<Color>, bool)>,
    bg_map: Option<(VecMap<Color>, bool)>,
    colspec: termcolor::ColorSpec,
    current_fg: Option<termcolor::Color>,
    current_bg: Option<termcolor::Color>,
    textcols: (Color, Color),
    actions: Vec<(ColorSource, ColorMode)>,
    initialized: bool,
    truecolor: bool,
}

impl ColorWriter {
    fn new() -> ColorWriter {
        ColorWriter {
            writer: termcolor::StandardStream::stdout(termcolor::ColorChoice::Auto),
            fg_map: None,
            bg_map: None,
            colspec: termcolor::ColorSpec::new(),
            current_fg: None,
            current_bg: None,
            textcols: (Color::from_rgb(named::BLACK), Color::from_rgb(named::WHITE)),
            actions: vec![],
            initialized: false,
            truecolor: has_truecolor(),
        }
    }

    fn truecolor(mut self, truecolor: bool) -> Self {
        self.truecolor = truecolor;
        self
    }

    fn textcols(mut self, dark: Color, bright: Color) -> Result<Self, String> {
        self.textcols = (dark, bright);
        Ok(self)
    }

    fn set(&mut self, source: ColorSource, mode: ColorMode) {
        self.actions.push((source, mode));
    }

    fn initialized(&self) -> bool {
        self.initialized
    }

    #[inline(never)]
    fn init(
        &mut self,
        seq: &[u8],
        dna_pal: VecMap<Color>,
        protein_pal: VecMap<Color>,
        qual_scale: VecMap<Color>,
    ) -> Result<(), String> {
        for (source, mode) in &self.actions {
            let store_to = match *mode {
                ColorMode::Fg => &mut self.fg_map,
                ColorMode::Bg => &mut self.bg_map,
            };

            *store_to = match *source {
                ColorSource::Qual => Some((qual_scale.clone(), true)),
                ColorSource::Symbol => {
                    guess_seqtype(seq, None).and_then(|(ref seqtype, _, _)| match seqtype {
                        SeqType::Dna | SeqType::Rna => Some((dna_pal.clone(), false)),
                        SeqType::Protein => Some((protein_pal.clone(), false)),
                        _ => None,
                    })
                }
            };
        }

        // set optimal text color

        if let Some((bg_map, _)) = self.bg_map.as_ref() {
            if self.fg_map.is_none() {
                let mut fg_map = VecMap::new();
                for (ref symbol, col) in bg_map {
                    let chosen = choose_fg(&self.textcols.0, &self.textcols.1, &col);
                    fg_map.insert(*symbol, chosen);
                }
                self.fg_map = Some((fg_map, false));
            }
        }

        self.initialized = true;

        Ok(())
    }

    fn write_symbol(&mut self, symbol: u8, qual: Option<u8>) -> io::Result<()> {
        if !self.initialized {
            panic!("BUG: ColorWriter must be initialized");
        }
        let mut changed = false;
        if let Some(&(ref map, load_qual)) = self.fg_map.as_ref() {
            let c = self._get_color(symbol, qual, map, load_qual);
            if self.current_fg != c {
                self.current_fg = c;
                self.colspec.set_fg(c);
                changed = true;
            }
        }
        if let Some(&(ref map, load_qual)) = self.bg_map.as_ref() {
            let c = self._get_color(symbol, qual, map, load_qual);
            if self.current_bg != c {
                self.current_bg = c;
                self.colspec.set_bg(c);
                changed = true;
            }
        }
        if changed {
            self.writer.set_color(&self.colspec)?;
        }
        write!(self.writer, "{}", symbol as char)
    }

    fn _get_color(
        &self,
        symbol: u8,
        qual: Option<u8>,
        map: &VecMap<Color>,
        load_qual: bool,
    ) -> Option<termcolor::Color> {
        let symbol = if load_qual {
            qual.expect("BUG: no qual")
        } else {
            symbol
        };
        map.get(symbol as usize)
            .map(|c| c.to_termcolor(self.truecolor))
    }

    fn reset(&mut self) -> io::Result<()> {
        self.current_fg = None;
        self.current_bg = None;
        self.colspec.clear();
        self.writer.set_color(&self.colspec)
    }
}

/// chooses the optimal text color based on the brightness/darkness of the background color
fn choose_fg(fg_dark: &Color, fg_bright: &Color, bg_col: &Color) -> Color {
    let dark_l = palette::Lab::<D65, _>::from(fg_dark.rgb).l as f32;
    let bright_l = palette::Lab::<D65, _>::from(fg_bright.rgb).l as f32;
    let bg = palette::Lab::<D65, _>::from(bg_col.rgb).l as f32;
    if (bright_l - bg) / (bright_l - dark_l) < 0.3 {
        fg_dark.clone()
    } else {
        fg_bright.clone()
    }
}

impl io::Write for ColorWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn has_truecolor() -> bool {
    var("COLORTERM")
        .map(|s| s == "truecolor" || s == "24bit")
        .unwrap_or(false)
    // 256-color: $TERM contains 256
    // see also https://github.com/chalk/supports-color/blob/master/index.js
}

fn parse_color(s: &str) -> Result<Srgb<u8>, String> {
    named::from_str(s).or_else(|| Srgb::from_str(s).ok())
        .ok_or_else(|| format!("Invalid color code: '{}'. The colors must be in Hex format (rrggbb) or a name (e.g. 'cyan')", s))
}
