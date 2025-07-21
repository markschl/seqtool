use std::mem::replace;
use std::str;
use std::{fmt::Write, iter::repeat};

use palette::named;
use ratatui::{
    style::{Style, Stylize},
    text::{Line, Span},
};
use vec_map::VecMap;

use crate::error::CliResult;
use crate::helpers::seqtype::{guess_seqtype, SeqType};
use crate::io::{QualConverter, Record};

use super::{choose_fg, Color, ColorSource, Palette, Palettes};

#[derive(Debug)]
pub(super) struct Formatter {
    id_len: u32,
    show_desc: bool,
    utf8: bool,
    truecolor: bool,
    // fg, bg
    sources: [Option<ColorSource>; 2],
    palettes: [Option<Palette>; 2],
    textcols: [Color; 2],
    bold: bool,
    initialized: bool,
}

impl Formatter {
    pub fn new(id_len: Option<u32>, show_desc: bool) -> Formatter {
        Formatter {
            id_len: id_len.unwrap_or(0),
            show_desc,
            utf8: false,
            truecolor: false,
            sources: [None, None],
            palettes: [None, None],
            textcols: [Color::from_rgb(named::BLACK), Color::from_rgb(named::WHITE)],
            bold: false,
            initialized: false,
        }
    }

    pub fn capabilities(mut self, truecolor: bool, utf8: bool) -> Self {
        self.truecolor = truecolor;
        self.utf8 = utf8;
        self
    }

    pub fn textcols(mut self, dark: Color, bright: Color) -> Result<Self, String> {
        self.textcols = [dark, bright];
        Ok(self)
    }

    pub fn color_config(mut self, bg: Option<ColorSource>, fg: Option<ColorSource>) -> Self {
        self.sources = [bg, fg];
        self
    }

    pub fn bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    #[inline(never)]
    pub fn init(&mut self, id: &[u8], seq: &[u8], palettes: &Palettes) -> CliResult<()> {
        // determine width of first ID
        self.id_len = (str::from_utf8(id)?.chars().count() + 3).clamp(10, 100) as u32;

        for (source, pal) in self.sources.iter().zip(&mut self.palettes) {
            *pal = source.and_then(|s| match s {
                ColorSource::Qual => Some(palettes.qual.clone()),
                ColorSource::Seq => {
                    guess_seqtype(seq, None)
                        .ok()
                        .and_then(|info| match info.seqtype {
                            SeqType::DNA | SeqType::RNA => Some(palettes.dna.clone()),
                            SeqType::Protein => Some(palettes.protein.clone()),
                            _ => None,
                        })
                }
            });
        }

        // set optimal bright or dark text color for different bg colors
        if let Some(pal) = self.palettes[0].as_ref() {
            if self.palettes[1].is_none() {
                let mut fg_map = VecMap::new();
                for (ref symbol, col) in pal {
                    let chosen = choose_fg(&self.textcols[0], &self.textcols[1], col);
                    fg_map.insert(*symbol, chosen);
                }
                self.palettes[1] = Some(fg_map);
                self.sources[1] = self.sources[0];
            }
        }
        self.initialized = true;
        Ok(())
    }

    pub fn format_scale<S>(&self, i: u8, symbols: S) -> Option<Vec<Span<'static>>>
    where
        S: IntoIterator<Item = u8>,
    {
        self.palettes[i as usize].as_ref().map(|map| {
            let mut spans = Vec::new();
            for symbol in symbols {
                let col = map.get(symbol as usize).unwrap().to_ratatui(self.truecolor);
                let style = Style::default().bg(col);
                spans.push(Span::styled(format!("{} ", symbol), style));
            }
            spans
        })
    }

    pub fn format(
        &mut self,
        record: &dyn Record,
        qual_converter: &mut QualConverter,
        palettes: &Palettes,
    ) -> CliResult<(Line<'static>, usize)> {
        let mut line = Line::default();
        let mut id_out = String::new();

        // Write ID and description

        let (id, desc) = record.id_desc();
        if !self.initialized {
            // initializing with first sequence line,
            // (guessing sequence type), but may not always be representative
            let mut buf = Vec::new();
            let seq = record.full_seq(&mut buf);
            self.init(id, &seq, palettes)?
        }

        let id = String::from_utf8_lossy(record.id());
        let ellipsis = if self.utf8 { '…' } else { ' ' };
        let mut id_len = id.chars().count();
        let replace_invalid = |c: char| {
            if c == '\t' {
                ' '
            } else if !self.utf8 && !c.is_ascii() {
                '_'
            } else {
                c
            }
        };
        let mut overflow = id_len >= self.id_len as usize;
        id_out.extend(
            id.chars()
                .map(replace_invalid)
                .take(self.id_len as usize - overflow as usize),
        );
        let rest = self.id_len as isize - id_len as isize;
        if self.show_desc && rest >= 3 {
            if let Some(d) = desc {
                let d = String::from_utf8_lossy(d);
                id_len += 1 + d.chars().count();
                overflow = id_len >= self.id_len as usize;
                id_out.push(' ');
                id_out.extend(
                    d.chars()
                        .map(replace_invalid)
                        .take(rest as usize - 1 - overflow as usize),
                );
            }
        }
        if overflow {
            id_out.push(if self.utf8 { ellipsis } else { ' ' });
        } else {
            id_out.extend(repeat(' ').take(self.id_len as usize - id_len));
        }
        id_out.push(' ');

        debug_assert_eq!(id_out.chars().count(), self.id_len as usize + 1);
        line.push_span(Span::raw(id_out));

        // Write styled sequence

        let mut seqlen = 0;
        // If quality scores are present, use them to determine the background color
        let mut phred = record
            .qual()
            .map(|q| qual_converter.phred_scores(q))
            .transpose()?;
        let mut qual_iter = phred.as_mut().map(|p| p.scores().iter());
        let mut seq = String::new();
        let mut style = Style::default();
        if self.bold {
            style = style.bold();
        }
        let mut prev_q = None;
        for segment in record.seq_segments() {
            for &symbol in segment {
                let q = qual_iter.as_mut().map(|q| q.next().copied().unwrap());
                if seq.as_bytes().last().copied() != Some(symbol) || q != prev_q {
                    line.push_span(Span::styled(
                        replace(&mut seq, String::new()),
                        style.clone(),
                    ));
                    let mut colors = self.palettes.iter().zip(self.sources).map(|(pal, source)| {
                        source.and_then(|s| {
                            let sym = match s {
                                ColorSource::Qual => q.unwrap(),
                                ColorSource::Seq => symbol,
                            };
                            pal.as_ref()
                                .unwrap()
                                .get(sym as usize)
                                .copied()
                                .map(|c| c.to_ratatui(self.truecolor))
                        })
                    });
                    let colors = [colors.next().unwrap(), colors.next().unwrap()];
                    style = style
                        .bg(colors[0].unwrap_or(ratatui::style::Color::Reset))
                        .fg(colors[1].unwrap_or(ratatui::style::Color::Reset));
                }
                seq.push(symbol as char);
                prev_q = q;
            }
            seqlen += segment.len();
        }
        if !seq.is_empty() {
            line.push_span(Span::styled(seq, style))
        }
        if let Some(q) = qual_iter.as_mut() {
            debug_assert!(q.next().is_none());
        }

        if let Some(p) = phred.as_ref() {
            // at the end, write some quality stats
            let prob = p.total_error();
            let rate = prob / seqlen as f64;
            let mut text = String::new();
            if prob < 0.001 {
                write!(text, " err: {prob:>3.2e} ({rate:.4e} / pos.)")?;
            } else {
                write!(text, " err: {prob:>2.3} ({rate:.4} / pos.)")?;
            }
            line.push_span(Span::raw(text));
        }
        Ok((line, seqlen))
    }
}
