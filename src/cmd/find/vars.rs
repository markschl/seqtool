use std::io::Write;

use crate::error::CliResult;
use crate::io::Record;
use crate::var::{
    func::Func,
    symbols::{SymbolTable, Value, VarType},
    VarBuilder, VarInfo, VarProvider, VarProviderInfo,
};
use crate::var_info;

#[derive(Debug)]
pub struct FindVarHelp;

impl VarProviderInfo for FindVarHelp {
    fn name(&self) -> &'static str {
        "Variables/functions to obtain pattern matches"
    }

    fn vars(&self) -> &[VarInfo] {
        &[
            var_info!(
                match =>
                "The text matched by the pattern. With approximate matching \
                (`-d/--dist` argument), this is the best hit \
                (with the smallest edit distance) or the \
                leftmost occurrence if `--in-order` was specified. \
                With exact/regex matching, the leftmost hit is always returned. \
                With multiple patterns in a pattern file, the best hit of the \
                best-matching pattern is returned (fuzzy matching), or the first \
                hit of the first pattern with an exact match; \
                see below for selecting other hits or other patterns)."
            ),
            var_info!(
                match (hit, [pattern]) =>
                "The matched text of the given hit number, or a command delimited list of \
                all hits if hit = 'all'. Hits are either sorted by the edit distance \
                or by occurrence (with `--in-order` or exact matching, same as described above). \
                With multiple patterns in a pattern file, the 2nd, 3rd, etc. \
                best matching pattern can be selected with `match(<hit>, 2)` or `match(<hit>, 3)`, \
                etc. (default: pattern=1)."
            ),
            var_info!(
                match_group (group, [hit], [pattern]) =>
                "Text matched by regex match group of given number (0 = entire match). \
                An empty string is returned if the group does not exist \
                The hit number (sorted by edit distance or occurrence) and the pattern \
                number can be specified as well (details above)."
            ),
            var_info!(
                match_dist [ (), (hit, [pattern]) ] =>
                "Number of mismatches/insertions/deletions (edit distance) of the search \
                pattern compared to the sequence. Either just `match_dist` for the best match, \
                or `match_dist(h, [p])` to get the edit distance of the h-th best hit of \
                the p-th pattern. `match_dist('all', [p]) will return a comma delimited list of \
                distances for all hits of a pattern."
            ),
            var_info!(
                        match_start [ (), (hit, [pattern]) ] =>
                        "Start coordinate of the first/best match. \
                        Other hits/patterns are selected with `match_start(hit, [pattern])`, \
                        for details see `match`)"
            ),
            var_info!(
            match_neg_start [ (), (hit, [pattern]) ] =>
                "Start of the first/best match relative to sequence end (negative coordinate). \
                Other hits/patterns are selected with `match_neg_start(hit, [pattern])`, \
                for details see `match`)"
            ),
            var_info!(
                    match_end [ (), (hit, [pattern]) ] =>
                    "End coordinate of the first/best match. \
                    Other hits/patterns are selected with `match_end(hit, [pattern])`, \
                    for details see `match`)"
            ),
            var_info!(
            match_neg_end [ (), (hit, [pattern]) ] =>
                "End of the first/best match relative to sequence end (negative coordinate). \
                Other hits/patterns are selected with `match_neg_end(hit, [pattern])`, \
                for details see `match`)"
                ),
            var_info!(
                match_range [ (), (hit, [pattern]) ] =>
                "Range (start-end) of the first/best match. \
                Other hits/patterns are selected with `match_range(hit, [pattern])`, \
                for details see `match`)"
            ),
            var_info!(
            match_neg_range [ (), (hit, [pattern]) ] =>
                "Range of the first/best match relative to sequence end (negative coordinate). \
                Other hits/patterns are selected with `match_neg_end(hit, [pattern])`, \
                for details see `match`)"
            ),
            var_info!(
                match_drange [ (), (hit, [pattern]) ] =>
                "Range of the match with two dots as delimiter (start..end). Useful if the \
                matched range(s) should be passed to the 'trim' or 'mask' commands."
            ),
            var_info!(
                match_neg_drange [ (), (hit, [pattern]) ] =>
                "Range of the match (dot delimiter) relative to the sequence end \
                (-<start>..-<end>)."
            ),
            var_info!(
                matchgrp_start (group, [h], [p]) =>
                "Start of regex match group no. 'group' of the first/best match. \
                Other hits/patterns are selected with `h` and `p` (for details see `match`)."
            ),
            var_info!(
                matchgrp_neg_start (group, [h], [p]) =>
                "Start coordinate of regex match group no. 'group' relative to the sequence end \
                (negative coordinate)."
            ),
            var_info!(
                matchgrp_end (group, [h], [p]) =>
                "End coordinate of regex match group no. 'group' of the first/best match. \
                Other hits/patterns are selected with `h` and `p` (for details see `match`)."
            ),
            var_info!(
                matchgrp_neg_end (group, [h], [p]) =>
                "End coordinate of regex match group no. 'group' relative to the sequence end \
                (negative coordinate)."
            ),
            var_info!(
                matchgrp_range (group, [h], [p]) =>
                "Range (start-end) of regex match group no. 'group' of the \
                first/best match. \
                Other hits/patterns are selected with `h` and `p` (for details see `match`)."
            ),
            var_info!(
                matchgrp_drange (group, [h], [p]) =>
                "Range (start..end) of regex match group no. 'group'."
            ),
            var_info!(
                matchgrp_neg_drange (group, [h], [p]) =>
                "Range of regex match group no. 'group', with '..' delimiter and relative to the \
                sequence end (`-<start>..-<end>`)."
            ),
            var_info!(
                pattern_name [ (), (p) ] =>
                "Name of the matching pattern if there are multiple \
                supplied using `file:patterns.fasta`, or just `<pattern>` if a single pattern \
                was specified in commandline."
            ),
            var_info!(pattern_name =>
            "Name of the matching pattern if there are multiple (pattern file), \
            or '<pattern>' if a single pattern was specified in commandline. \
            `pattern_name(p)` allows selecting the p-th matching pattern \
            (sorted by edit distance and/or pattern number)"),
        ]
    }
    // fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
    //     Some(&[])
    // }
}

// Variables

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum FindVarType {
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

use self::FindVarType::*;

use super::{FindCommand, Matches, SearchConfig};

#[derive(Debug)]
pub struct VarPos {
    var_type: FindVarType,
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
        symbols: &mut SymbolTable,
        text: &[u8],
    ) -> CliResult<()> {
        for pos in &self.vars {
            let out = symbols.get_mut(pos.var_id);
            let val: &mut Value = out.inner_mut();
            if pos.var_type == Name {
                let name = matches.pattern_name(pos.pattern_rank).unwrap_or("");
                val.set_text(name.as_bytes());
                continue;
            }

            if let Some(p) = pos.hit_pos.as_ref() {
                // specific hits requested
                if let Some(m) = matches.get_match(*p, pos.match_group, pos.pattern_rank) {
                    match pos.var_type {
                        Start => val.set_int((m.start + 1) as i64),
                        End => val.set_int((m.end) as i64),
                        NegStart => val.set_int(m.neg_start1(rec.seq_len())),
                        NegEnd => val.set_int(m.neg_end1(rec.seq_len())),
                        Dist => val.set_int(i64::from(m.dist)),
                        Range(ref delim) => {
                            write!(val.mut_text(), "{}{}{}", m.start + 1, delim, m.end)?
                        }
                        NegRange(ref delim) => write!(
                            val.mut_text(),
                            "{}{}{}",
                            m.neg_start1(rec.seq_len()),
                            delim,
                            m.neg_end1(rec.seq_len())
                        )?,
                        Match => val.set_text(&text[m.start..m.end]),
                        _ => unreachable!(),
                    }
                    continue;
                }
            } else {
                // List of all matches requested:
                // This is different from above by requiring a string type
                // in all cases instead of integers.
                let out = val.mut_text();
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
            out.set_none();
        }
        Ok(())
    }
}

impl VarProvider for FindVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &FindVarHelp
    }

    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        let name = func.name.as_str();
        // obtain args (hit number, pattern number, regex group number)
        let (var_type, out_t, hit_i, pat_i, grp_i) = match name {
            "match_dist" => (Dist, VarType::Int, Some(0), 1, None),
            "match" => (Match, VarType::Text, Some(0), 1, None),
            "match_group" => (Match, VarType::Text, Some(1), 2, Some(0)),
            "pattern_name" => (Name, VarType::Text, None, 0, None),
            _ => {
                #[allow(clippy::manual_strip)]
                let (grp, name) = if name.starts_with("matchgrp_") {
                    (true, &name[9..])
                } else if name.starts_with("match_") {
                    (false, &name[6..])
                } else {
                    unreachable!();
                };
                let var = match name {
                    "start" => Start,
                    "end" => End,
                    "neg_start" => NegStart,
                    "neg_end" => NegEnd,
                    "range" => Range("-".into()),
                    "drange" => Range("..".into()),
                    "neg_drange" => NegRange("..".into()),
                    _ => unreachable!(),
                };
                if grp {
                    (var, VarType::Int, Some(1), 2, Some(0))
                } else {
                    (var, VarType::Int, Some(0), 1, None)
                }
            }
        };

        // set variable defaults
        let hit_num = hit_i.and_then(|i| func.opt_arg(i));
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
            .and_then(|i| func.opt_arg_as::<usize>(i))
            .transpose()?
            // we use group = 0 to indicate the whole match
            .unwrap_or(0);

        // pattern rank:
        let pattern_rank = func.opt_arg_as::<usize>(pat_i).transpose()?.unwrap_or(1);

        debug_assert!(self.num_patterns > 0);
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
        Ok(Some(out_t))
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }
}

pub fn get_varprovider(args: &FindCommand) -> Option<Box<dyn VarProvider>> {
    Some(Box::new(FindVars::new(args.patterns.len())))
}
