use std::cmp::max;
use std::cmp::min;
use std::collections::HashMap;
use std::env::var;
use std::io::{self, Write};
use std::str;
use std::str::FromStr;

use ansi_colours::ansi256_from_rgb;
use enterpolation::{linear::Linear, Curve, Merge};
use palette::named;
use palette::rgb::Rgb;
use palette::white_point::D65;
use palette::Hsv;
use palette::{FromColor, IntoColor, LinSrgb, Mix};
use termcolor::{self, WriteColor};
use vec_map::VecMap;

#[cfg(target_family = "unix")]
use pager::Pager;

use crate::config;
use crate::error::CliResult;
use crate::io::{qual_to_prob, QualFormat};
use crate::helpers::seqtype::{guess_seqtype, SeqType};
use crate::opt;

pub static USAGE: &str = concat!(
    "
View biological sequences, coloured by base / amino acid, or by sequence quality.
The output is automatically forwarded to the 'less' pager on UNIX.

Usage:
    st view [options] [<input>...]
    st view (-h | --help)

General command options:
    -n, --num-seqs <N>  Number of sequences to select
    -i, --id-len <N>    Length of IDs in characters. Longer IDs are truncated
                        (default: 10 - 100 depending on ID length)
    -d, --show-desc     Show descriptions along IDs if there is enough space.
    -f, --foreground    Color base / amino acid letters instead of background.
                        If base qualities are present, background coloration
                        is shown, and the foreground scheme will be 'dna-bright'
                        (change with --dna-pal).

Pager (UNIX only):
    -n, --no-pager      Disable automatic forwarding to pager
    --pager <pager>     Pager command to use (default: less -RS).
                        This overrides the value of the $ST_PAGER env.
                        variable, if set.
    -b, --break         Break lines in pager, disabling 'horizontal scrolling'.
                        Equivalent to --pager 'less -R'

Coloring:
    --list-pal          List all palettes and exit.
    --dna-pal <pal>     Color mapping for DNA. Palette name or list of
                        <bases>:<color> (hex code or CSS/SVG color name)
                        [default: dna] (available: dna, dna-bright, dna-dark,
                        pur-pyrimid, gc-at).
    --aa-pal <palette>  Color mapping for amino acids. Palette name or list of
                        <letters>:<color> [default: rasmol] (available:
                        rasmol, polarity).
    --qscale <colors>   Color scale to use for coloring according to base
                        quality. Palette name or sequence of hex codes from
                        low to high [default: blue-red] (available: blue-red).
    --qmax <value>      Upper limit of Phred score visualization (-q)
                        [default: 41]
    --textcols <c>      Text colors used with background coloring.
                        Specify as: <dark>,<bright>. Which one is used will be
                        chosen depending on the brightness of the background.
                        [default: 333333,eeeeee]
    -c, --truecolor     Use 16M colors, not only 256. This has to be supported
                        by the terminal. Useful if autorecognition did not work.
",
    common_opts!()
);

lazy_static! {
    static ref PALETTES: HashMap<&'static str, &'static str> = hashmap!{
        "rasmol" =>
            "DE:e60a0a,CM:e6e600,RK:145aff,ST:fa9600,FY:3232aa,NQ:00dcdc,G:ebebeb,LVI:0f820f,A:c8c8c8,W:b45Ab4,H:8282d2,P:dc9682",
        "polarity" => // similar as Geneious
            "GAVLIFWMP:ffd349,STCYNQ:3dff51,DE:ff2220,KRH:1e35ff",
        "dna" =>
            "A:ce0000,C:0000ce,G:ffde00,TU:00bb00,RYSWKMBDHVN:8f8f8f",
        "dna-bright" =>
            "A:ff3333,C:3333ff,G:ffe747,TU:00db00,RYSWKMBDHVN:b8b8b8",
        "dna-dark" =>
            "A:940000,C:00008f,G:9e8900,TU:006b00,RYSWKMBDHVN:8f8f8f",
        "pur-pyrimid" =>
            "AGR:e4cff,CTUY:25bdff",
        "gc-at" =>
            "GCS:ff2b25,ATUW:ffd349",
        "blue-red" =>
            "ee0000,0000ee"
    };
}

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;

    let nmax: Option<usize> = args.opt_value("--num-seqs")?;
    let mut id_len: Option<usize> = args.opt_value("--id-len")?;
    let show_desc = args.get_bool("--show-desc");
    let truecolor = args.get_bool("--truecolor");
    let qmax: u8 = args.value("--qmax")?;
    let dna_pal = args.get_str("--dna-pal");
    let aa_pal = args.get_str("--aa-pal");
    let qscale = args.get_str("--qscale");
    let textcols = args.get_str("--textcols");
    let foreground = args.get_bool("--foreground");

    if id_len == Some(0) {
        id_len = Some(1);
    }

    if args.get_bool("--list-pal") {
        eprintln!(concat!(
            "List of palette names their color mappings, which are in the form\n",
            "<symbol>:<colors>. Colors are specified as HEX codes. The colors can be\n",
            "directly configured using --dna-pal / --aa-pal / --qscale. These options\n",
            "accept both palette names and color mappings.\n"
        ));
        for (pal, mapping) in PALETTES.iter() {
            eprintln!("{:<10} {}", pal, mapping);
        }
        return Ok(());
    }

    #[cfg(target_family = "unix")]
    #[allow(unused_variables)]
    setup_pager(
        args.opt_str("--pager"),
        args.get_bool("--break"),
        args.get_bool("--no-pager"),
    );

    // setup colors

    let textcols: Vec<_> = textcols.split(',').collect();
    if textcols.len() != 2 {
        return fail!("Invalid number of text colors. Specify '--textcols <dark>,<bright>'. ");
    }

    let mut writer = ColorWriter::new()
        .truecolor(truecolor)
        .dna_pal(dna_pal)
        .protein_pal(aa_pal)
        .qual_scale(qscale)
        .textcols(textcols[0], textcols[1])?;

    if cfg.input_opts()[0].format.has_qual() {
        writer.set(ColorSource::Qual { qmax }, ColorMode::Bg);
        if foreground {
            writer.set(ColorSource::Symbol, ColorMode::Fg);
            if dna_pal == "dna" {
                writer = writer.dna_pal("dna-bright");
            }
        }
    } else if foreground {
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
    let mut i = 0;
    let mut id_len = id_len.unwrap_or(0);
    // TODO: not actually required, currently
    cfg.with_vars(|vars| {
        cfg.read(vars, |record, vars| {
            if let Some(n) = nmax {
                if i >= n {
                    return Ok(false);
                }
            }

            // write seq. ids / desc

            let (id, desc) = record.id_desc_bytes();

            if id_len == 0 {
                // determine ID width of first ID
                id_len = min(100, max(10, std::str::from_utf8(id)?.chars().count() + 3));
            }

            write_id(id, desc, &mut writer, id_len, show_desc, utf8)?;

            // write seq

            if let Some(qual) = record.qual() {
                let mut qual_iter = qual.iter();

                let mut prob = 0.;
                let mut seqlen = 0;

                for seq in record.seq_segments() {
                    if !writer.initialized() {
                        // TODO: initializing with first sequence line -> enough?
                        writer.init(seq)?;
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
                for seq in record.seq_segments() {
                    if !writer.initialized() {
                        writer.init(seq)?;
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

enum ColorSource {
    Symbol,
    Qual { qmax: u8 },
}

enum ColorMode {
    Fg,
    Bg,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Color {
    rgb: (u8, u8, u8),
    ansi: AnsiColor,
}

impl Color {
    fn from_rgb(c: LinSrgb) -> Self {
        Self {
            rgb: (
                c.red.round() as u8,
                c.green.round() as u8,
                c.blue.round() as u8,
            ),
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

impl From<LinSrgb> for AnsiColor {
    fn from(c: LinSrgb) -> Self {
        let c: Rgb<_, u8> = c.into();
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
    dna_pal: String,
    protein_pal: String,
    qual_scale: String,
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
            textcols: (
                Color::from_rgb(named::BLACK.into_linear()),
                Color::from_rgb(named::WHITE.into_linear()),
            ),
            actions: vec![],
            initialized: false,
            truecolor: has_truecolor(),
            dna_pal: PALETTES["dna"].to_string(),
            protein_pal: PALETTES["rasmol"].to_string(),
            qual_scale: PALETTES["blue-red"].to_string(),
        }
    }

    fn dna_pal(mut self, pal: &str) -> Self {
        self.dna_pal = pal.to_string();
        self
    }

    fn protein_pal(mut self, pal: &str) -> Self {
        self.protein_pal = pal.to_string();
        self
    }

    fn qual_scale(mut self, scale: &str) -> Self {
        self.qual_scale = scale.to_string();
        self
    }

    fn truecolor(mut self, truecolor: bool) -> Self {
        self.truecolor = truecolor;
        self
    }

    fn textcols(mut self, dark: &str, bright: &str) -> Result<Self, String> {
        self.textcols = (Color::from_str(dark)?, Color::from_str(bright)?);
        Ok(self)
    }

    fn set(&mut self, source: ColorSource, mode: ColorMode) {
        self.actions.push((source, mode));
    }

    fn initialized(&self) -> bool {
        self.initialized
    }

    fn init(&mut self, seq: &[u8]) -> Result<(), String> {
        for (source, mode) in &self.actions {
            let store_to = match *mode {
                ColorMode::Fg => &mut self.fg_map,
                ColorMode::Bg => &mut self.bg_map,
            };

            *store_to = match *source {
                ColorSource::Qual { qmax } => {
                    let qscale = &self.qual_scale;
                    let scale = PALETTES.get(qscale.trim()).copied().unwrap_or(qscale);
                    Some((load_phred_colors(scale, qmax)?, true))
                }
                ColorSource::Symbol => {
                    let mut palette = None;
                    if let Some((seqtype, _, _)) = guess_seqtype(seq, None) {
                        palette = match seqtype {
                            SeqType::Dna | SeqType::Rna => Some(&self.dna_pal),
                            SeqType::Protein => Some(&self.protein_pal),
                            _ => None,
                        }
                    }
                    palette.map(|pal| {
                        let pal = PALETTES.get(pal.trim()).copied().unwrap_or(pal.as_str());
                        Ok::<_, String>((parse_colormap(pal)?, false))
                    })
                    .transpose()?
                }
            };
        }

        // set optimal text color

        if let Some((bg_map, _)) = self.bg_map.as_ref() {
            if self.fg_map.is_none() {
                let mut fg_map = VecMap::new();
                let dark_l = palette::Lab::<D65, _>::from(self.textcols.0.rgb).l;
                let bright_l = palette::Lab::<D65, _>::from(self.textcols.1.rgb).l;
                for (ref symbol, col) in bg_map {
                    let l = palette::Lab::<D65, _>::from(col.rgb).l;
                    let col = if ((bright_l - l) as f32) / ((bright_l - dark_l) as f32) < 0.3 {
                        &self.textcols.0
                    } else {
                        &self.textcols.1
                    };
                    fg_map.insert(*symbol, col.clone());
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

fn parse_colormap(colors: &str) -> Result<VecMap<Color>, String> {
    let mut out = VecMap::new();

    for c in colors.split(',') {
        let mut s = c.split(':');
        let symbols = s.next().unwrap().as_bytes().to_vec();
        if let Some(col) = s.next() {
            let col = Color::from_str(col)?;
            for s in symbols {
                out.insert(s as usize, col.clone());
            }
        } else {
            return fail!(format!(
                "Invalid color mapping: '{}'. Use 'XY:rrggbb' for mapping X and Y to a given color",
                c
            ));
        }
    }
    Ok(out)
}

fn load_phred_colors(scale: &str, qmax: u8) -> Result<VecMap<Color>, String> {
    // HSV color gradient
    // TODO: needed?
    let scale: Vec<_> = scale
        .split(',')
        .map(|code| Ok(Adapter(Hsv::from_color(parse_color(code)?))))
        .collect::<Result<_, String>>()?;

    let mut out = VecMap::new();
    let gradient = Linear::builder()
        .elements(scale)
        .equidistant::<f32>()
        .normalized()
        .build()
        .unwrap();
    for (i, c) in gradient.take(qmax as usize).enumerate() {
        out.insert(i, Color::from_rgb(c.0.into_color()));
    }
    Ok(out)
}

fn parse_color(s: &str) -> Result<LinSrgb, String> {
    if let Some(c) = named::from_str(s).or_else(|| Rgb::from_str(s).ok()) {
        return Ok(c.into_linear());
    }
    fail!(format!("Invalid color code: '{}'. The colors must be in Hex format (rrggbb) or a name (e.g. 'cyan')", s))
}
