use std::io::Write;

use crate::error::CliResult;
use crate::io::Record;
use crate::var::{self, Func, VarBuilder};

use super::*;

pub struct FindVarHelp;

impl VarHelp for FindVarHelp {
    fn name(&self) -> &'static str {
        "Variables/functions to obtain pattern matches"
    }

    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            ("match", "The text matched by the pattern. With fuzzy string matching, \
             this would be the hit with the smallest edit distance, or the \
             first occurrence if --in-order was specified. With exact/regex \
             matching, the leftmost hit is always returned. \
             If there multiple patterns in a pattern file, the best hit of the \
             best-matching pattern is returned (fuzzy matching), or the first \
             hit of the first pattern (see below for selecting other hits or \
             other patterns)."),
            ("match(h, [p=1])",
             "The matched text of the h-th hit (or comma delimited list of all if \
             h='all'); optionally from the p-th matching pattern if several \
             patterns in file (default: p=1)"),
            ("match_group(g, [h=1], [p=1])",
             "Regex match group 'g' of hit 'h' for the p-th matching pattern. \
              An empty string is returned if the group does not exist."),
            ("match_dist",
            "Edit distance (number of mismatches/insertions/deletions) of the search \
            pattern compared to the sequence."),
            ("match_dist(n, [p])",
            "Edit distance of the nth hit of the p-th pattern."),
            ("match_start", "Start coordinate of the match."),
            ("match_end",   "End coordinate of the match."),
            ("match_range",  "Range of the match in the form 'start-end'"),
            ("match_start(n, [p]); match_end(n, [p]); match_range(n, [p])",
            "Start/end/range of the nth hit of the p-th pattern."),
            ("matchgrp_start(g, [n], [p]); matchgrp_end(g, [n], [p]); matchgrp_range(g, [n], [p])",
            "Start/end/range of regex match group 'g' from the the nth hit \
             of the p-th pattern."),
            ("match_neg_start", "Start of the match relative to sequence end (negative number)"),
            ("match_neg_end",   "End of the match relative to sequence end (negative number)"),
            ("match_neg_start(n, [p]), match_neg_end(n, [p])",
            "Start/end of the match relative to sequence end (negative) of the nth hit of \
             the p-th pattern."),
            ("matchgrp_neg_start(g, [n], [p]), matchgrp_neg_start(g, [n], [p])",
             "Negative start/end of regex match group 'g' from the the nth hit \
              of the p-th pattern."),
            ("match_drange",
            "Range of the match with two dots as delimiter (start..end). Useful with 'trim'\
             and 'mask'"),
            ("match_neg_drange",
            "Range of the match (dot delimiter) relative to the sequence end (-<start>..-<end>)"),
            ("match_drange(n, [p]), match_neg_drange(n, [p])",
            "Match ranges (normal/from end) of the nth hit of the p-th pattern."),
            ("matchgrp_drange(g, [n], [p]), matchgrp_neg_drange(g, [n], [p])",
            "Match ranges (normal/from end) of regex match group 'g' from \
             the nth hit of the p-th pattern."),
            ("pattern_name",
            "Name of the matching pattern if there are multiple (pattern file), \
            or <pattern> if one pattern specified in commandline."),
            ("pattern_name(p)",
            "Name of the p-th matching pattern"),
        ])
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[])
    }
}

// Variables

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum VarType {
    Start,
    End,
    Range(String),
    NegRange(String),
    NegStart,
    NegEnd,
    Dist,
    Match,
    Name,
}

use self::VarType::*;

#[derive(Debug)]
pub struct VarPos {
    var_type: VarType,
    var_id: usize,
    // None for all hits
    hit_pos: Option<usize>,
    match_group: usize,
    pattern_rank: usize,
}

#[derive(Debug)]
pub struct FindVars {
    vars: Vec<VarPos>,
    cfg: SearchConfig,
    bounds_needed: (bool, bool),
    num_patterns: usize,
}

impl FindVars {
    pub fn new(num_patterns: usize) -> FindVars {
        FindVars {
            vars: vec![],
            cfg: SearchConfig::new(),
            bounds_needed: (false, false),
            num_patterns,
        }
    }

    pub fn config(&self) -> &SearchConfig {
        &self.cfg
    }

    /// hit_num = None means all hits should be returned/stored
    /// match_group = 0 means the whole hit
    fn register_match(&mut self, pos: VarPos) -> CliResult<()> {
        if let Some(p) = pos.hit_pos {
            self.cfg.register_pos(p, pos.match_group);
        } else {
            self.cfg.register_all(pos.match_group);
        }
        self.vars.push(pos);
        Ok(())
    }

    pub fn register_all(&mut self, group: usize) {
        self.cfg.register_all(group);
    }

    pub fn bounds_needed(&self) -> (bool, bool) {
        self.bounds_needed
    }

    pub fn set_with(
        &mut self,
        rec: &dyn Record,
        matches: &Matches,
        symbols: &mut var::symbols::SymbolTable,
        text: &[u8],
    ) -> CliResult<()> {
        for pos in &self.vars {
            let sym = symbols.get_mut(pos.var_id);
            if pos.var_type == Name {
                let name = matches.pattern_name(pos.pattern_rank).unwrap_or("");
                sym.set_text(name.as_bytes());
                continue;
            }

            if let Some(p) = pos.hit_pos.as_ref() {
                // specific hits requested
                if let Some(m) = matches.get_match(*p, pos.match_group, pos.pattern_rank) {
                    match pos.var_type {
                        Start => sym.set_int((m.start + 1) as i64),
                        End => sym.set_int((m.end) as i64),
                        NegStart => sym.set_int(m.neg_start1(rec.seq_len())),
                        NegEnd => sym.set_int(m.neg_end1(rec.seq_len())),
                        Dist => sym.set_int(i64::from(m.dist)),
                        Range(ref delim) => {
                            write!(sym.mut_text(), "{}{}{}", m.start + 1, delim, m.end)?
                        }
                        NegRange(ref delim) => write!(
                            sym.mut_text(),
                            "{}{}{}",
                            m.neg_start1(rec.seq_len()),
                            delim,
                            m.neg_end1(rec.seq_len())
                        )?,
                        Match => sym.set_text(&text[m.start..m.end]),
                        _ => unreachable!(),
                    }
                    continue;
                }
            } else {
                // List of all matches requested:
                // This is different from above by requiring a string type
                // in all cases instead of integers.
                let out = sym.mut_text();
                let mut n = 0;
                for m in matches
                    .matches_iter(pos.pattern_rank, pos.match_group)
                    .flatten()
                {
                    n += 1;
                    match pos.var_type {
                        Start => write!(out, "{}", m.start + 1)?,
                        End => write!(out, "{}", m.end)?,
                        NegStart => write!(out, "{}", m.neg_start1(rec.seq_len()))?,
                        NegEnd => write!(out, "{}", m.neg_end1(rec.seq_len()))?,
                        Dist => write!(out, "{}", m.dist)?,
                        Range(ref delim) => write!(out, "{}{}{}", m.start + 1, delim, m.end)?,
                        NegRange(ref delim) => write!(
                            out,
                            "{}{}{}",
                            m.neg_start1(rec.seq_len()),
                            delim,
                            m.neg_end1(rec.seq_len())
                        )?,
                        Match => out.extend_from_slice(&text[m.start..m.end]),
                        _ => unreachable!(),
                    }
                    out.push(b',');
                }
                if n > 0 {
                    // remove last comma
                    out.pop();
                    continue;
                }
            }
            // important: reset previous value if nothing was found
            sym.set_none();
        }
        Ok(())
    }
}

impl VarProvider for FindVars {
    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<bool> {
        let name = func.name.as_str();
        // new-style variables/functions
        let (var_type, hit_i, pat_i, grp_i, min_args, max_args) = match name {
            "match_dist" => (Dist, Some(0), 1, None, 0, 2),
            "match" => (Match, Some(0), 1, None, 0, 2),
            "match_group" => (Match, Some(1), 2, Some(0), 1, 3),
            "pattern_name" => (Name, None, 0, None, 0, 1),
            _ => {
                #[allow(clippy::manual_strip)]
                let (grp, name) = if name.starts_with("matchgrp_") {
                    (true, &name[9..])
                } else if name.starts_with("match_") {
                    (false, &name[6..])
                } else {
                    return Ok(false);
                };
                let var = match name {
                    "start" => Start,
                    "end" => End,
                    "neg_start" => NegStart,
                    "neg_end" => NegEnd,
                    "range" => Range("-".into()),
                    "drange" => Range("..".into()),
                    "neg_drange" => NegRange("..".into()),
                    _ => return Ok(false),
                };
                if grp {
                    (var, Some(1), 2, Some(0), 0, 3)
                } else {
                    (var, Some(0), 1, None, 0, 2)
                }
            }
        };

        func.ensure_arg_range(min_args, max_args)?;

        // set variable defaults
        let hit_num = hit_i.and_then(|i| func.arg(i));
        let hit_pos = match hit_num {
            None => Some(0),
            Some("all") => None,
            Some(num) => {
                let num: usize = num
                    .parse()
                    .map_err(|_| format!("Invalid hit number: {}", num))?;
                let i = num.checked_sub(1).ok_or("The hit number must be > 0")?;
                Some(i)
            }
        };

        let match_group = grp_i
            .and_then(|i| func.arg_as::<usize>(i))
            .transpose()?
            // we use group = 0 to indicate the whole match
            .unwrap_or(0);

        // pattern rank:
        let pattern_rank = func.arg_as::<usize>(pat_i).transpose()?.unwrap_or(1);

        if pattern_rank > self.num_patterns {
            return fail!(format!(
                "Pattern rank {} requested, but there are only {} patterns",
                pattern_rank, self.num_patterns
            ));
        }
        let pattern_rank = pattern_rank
            .checked_sub(1)
            .ok_or("The pattern rank must be > 0")?;

        // determine whether the match bounds need to be calculated
        // (can be slower depending on the algorithm)
        if var_type != End && var_type != Dist && var_type != Name {
            self.bounds_needed.0 = true;
        }
        if var_type != Start && var_type != Dist && var_type != Name {
            self.bounds_needed.1 = true;
        }

        let pos = VarPos {
            var_type,
            var_id: b.symbol_id(),
            hit_pos,
            match_group,
            pattern_rank,
        };
        self.register_match(pos)?;
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }
}
