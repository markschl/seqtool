use std::env::var;
use std::io::{self, Write as _};
use std::str;

#[cfg(target_family = "unix")]
use pager::Pager;
use palette::named;
use termcolor::{self, WriteColor};
use vec_map::VecMap;

use crate::config::Config;
use crate::error::CliResult;

use crate::helpers::seqtype::{guess_seqtype, SeqType};

pub mod cli;
pub mod color;
pub mod pal;

pub use self::cli::*;
pub use self::color::*;
pub use self::pal::*;

pub fn run(mut cfg: Config, args: ViewCommand) -> CliResult<()> {
    let truecolor = args.color.truecolor.unwrap_or_else(has_truecolor);
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

    if cfg.input_config()[0].1.format.has_qual() {
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
    cfg.read(|record, ctx| {
        if let Some(n) = n_max {
            if i >= n {
                return Ok(false);
            }
        }

        // write seq. ids / desc
        let (id, desc) = record.id_desc();
        if id_len == 0 {
            // determine ID width of first ID
            id_len = (std::str::from_utf8(id)?.chars().count() + 3).clamp(10, 100) as u32;
        }
        write_id(
            id,
            desc,
            &mut writer,
            id_len as usize,
            args.general.show_desc,
            utf8,
        )?;

        // write the sequence
        if let Some(qual) = record.qual() {
            // If quality scores are present, use them to determine the background color
            // first, validate and convert to Phred scores
            let phred = ctx.qual_converter.phred_scores(qual)?;
            let mut seqlen = 0;
            let mut qual_iter = phred.scores().iter();
            for seq in record.seq_segments() {
                if !writer.initialized() {
                    // TODO: initializing with first sequence line,
                    // (guessing sequence type), but may not always be representative
                    writer.init(
                        seq,
                        args.color.dna_pal.clone(),
                        args.color.aa_pal.clone(),
                        args.color.qscale.clone(),
                    )?;
                }
                for &symbol in seq {
                    let q = *qual_iter.next().unwrap();
                    writer.write_symbol(symbol, Some(q))?;
                }
                seqlen += seq.len();
            }

            writer.reset()?;

            // finally, write some quality stats
            let prob = phred.total_error();
            let rate = prob / seqlen as f64;
            if prob < 0.001 {
                write!(writer, " err: {prob:>3.2e} ({rate:.4e} / pos.)")?;
            } else {
                write!(writer, " err: {prob:>2.3} ({rate:.4} / pos.)")?;
            }
        } else {
            // if no quality scores present, color by sequence
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

pub(super) struct ColorWriter {
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
                    guess_seqtype(seq, None)
                        .ok()
                        .and_then(|info| match info.seqtype {
                            SeqType::DNA | SeqType::RNA => Some((dna_pal.clone(), false)),
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
                    let chosen = choose_fg(&self.textcols.0, &self.textcols.1, col);
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

impl io::Write for ColorWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
