use std::cmp::max;
use std::io::Write;

use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use super::matches::Matches;
use super::opts::{Opts, RequiredInfo};
use crate::helpers::write_list::write_list_with;
use crate::io::Record;
use crate::var::{modules::VarProvider, parser::Arg, symbols::SymbolTable, VarBuilder, VarStore};

variable_enum! {
    /// # Variables/functions recognized by the 'find' command
    ///
    /// The find command provides many variables/functions to obtain information about
    /// the pattern matches. These are either written to header attributes
    /// (`-a/--attr`) or CSV/TSV fields (e.g. `--to-tsv ...`). See also examples section below.
    ///
    /// # Examples
    ///
    /// Find a primer sequence with up to 2 mismatches (`-d/--dist``) and write
    /// the match range and the mismatches ('dist') to the header as attributes.
    /// The result will be N/A (or undefined in JavaScript) if there are > 2 mismatches
    ///
    /// `st find -d 2 CTTGGTCATTTAGAGGAAGTAA -a rng={match_range} -a dist={match_diffs} reads.fasta`
    ///
    /// >id1 rng=2-21 dist=1
    /// SEQUENCE
    /// >id2 rng=1-20 dist=0
    /// SEQUENCE
    /// >id3 rng= dist=
    /// SEQUENCE
    /// (...)
    ///
    ///
    /// Find a primer sequence and if found, remove it using the 'trim' command,
    /// while non-matching sequences are written to 'no_primer.fasta'
    ///
    /// `st find -f -d 2 CTTGGTCATTTAGAGGAAGTAA --dropped no_primer.fasta -a end={match_end} reads.fasta |
    ///    st trim -e '{attr(match_end)}..' > primer_trimmed.fasta`
    ///
    ///
    /// Search for several primers with up to 2 mismatches and write the name and mismatches
    /// of the best-matching primer to the header
    ///
    /// `st find -d 2 file:primers.fasta -a primer={pattern_name} -a dist={match_diffs} reads.fasta`
    ///
    /// >id1 primer=primer_1 dist=1
    /// SEQUENCE
    /// >id1 primer=primer_2 dist=0
    /// SEQUENCE
    /// >id1 primer= dist=
    /// SEQUENCE
    /// (...)
    FindVar {
        /// The text matched by the pattern. With approximate matching
        /// (`-d/--dist` argument), this is the best hit
        /// (with the smallest edit distance) or the
        /// leftmost occurrence if `--in-order` was specified.
        /// With exact/regex matching, the leftmost hit is always returned.
        /// With multiple patterns in a pattern file, the best hit of the
        /// best-matching pattern is returned (fuzzy matching), or the first
        /// hit of the first pattern with an exact match;
        /// see below for selecting other hits or other patterns).
        ///
        /// `match(hit) returns the matched text of the given hit number,
        /// whereas `match(all)` or `match('all') returns a command delimited
        /// list of all hits. These are either sorted by the edit distance
        /// (default) or by occurrence (with `--in-order` or exact matching).
        ///
        /// `match(1, 2)`, `match(1, 3)`, etc. references the 2nd, 3rd, etc.
        /// best matching pattern in case multiple patterns were suplied in a
        /// file (default: hit=1, pattern=1)."
        Match(Text) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Text matched by regex match group of given number (0 = entire match).
        /// An empty string is returned if the group does not exist
        /// The hit number (sorted by edit distance or occurrence) and the pattern
        /// number can be specified as well (details above).
        MatchGroup(Text) { group: usize, hit: String = String::from("1"), pattern: usize = 1 },
        /// Start coordinate of the first/best match. Other hits/patterns are selected
        /// with `match_start(hit, [pattern])`, for details see `match`
        MatchStart(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Start of the first/best match relative to sequence end (negative coordinate).
        /// Other hits/patterns are selected with `match_neg_start(hit, [pattern])`,
        /// for details see `match`.
        MatchNegStart(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// End coordinate of the first/best match. Other hits/patterns are selected
        /// with `match_end(hit, [pattern])`, for details see `match`
        MatchEnd(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// End of the first/best match relative to sequence end (negative coordinate).
        /// Other hits/patterns are selected with `match_neg_end(hit, [pattern])`,
        /// for details see `match`.
        MatchNegEnd(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Range (start-end) of the first/best match. Other hits/patterns are selected
        /// with `match_range(hit, [pattern])`, for details see `match`
        MatchRange(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Range of the first/best match relative to sequence end (negative coordinate).
        /// Other hits/patterns are selected with `match_neg_range(hit, [pattern])`,
        /// for details see `match`.
        MatchNegRange(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Range of the match with two dots as delimiter (start..end). Useful if the
        /// matched range(s) should be passed to the 'trim' or 'mask' commands.
        MatchDrange (Text) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Range of the match (dot delimiter) relative to the sequence end
        /// (-<start>..-<end>).
        MatchNegDrange (Text) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Start of regex match group no. 'group' of the first/best match
        /// (group=0 is the entire match). Other hits/patterns are selected
        /// with `matchgrp_start(hit, [pattern])`, for details see `match`.
        MatchGrpStart(Number) { group: usize, hit: String = String::from("1"), pattern: usize = 1 },
        /// End of regex match group no. 'group' of the first/best match
        /// (group=0 is the entire match). Other hits/patterns are selected
        /// with `matchgrp_end(hit, [pattern])`, for details see `match`.
        MatchGrpEnd(Number) { group: usize, hit: String = String::from("1"), pattern: usize = 1 },
        /// End coordinate of regex match group no. 'group' relative to the sequence end
        /// (negative coordinate).
        MatchGrpNegEnd(Number) { group: usize, hit: String = String::from("1"), pattern: usize = 1 },
        /// Range (start-end) of regex match group no. 'group' of the first/best match
        /// (group=0 is the entire match). Other hits/patterns are selected
        /// with `matchgrp_range(hit, [pattern])`, for details see `match`.
        MatchGrpRange(Number) { group: usize, hit: String = String::from("1"), pattern: usize = 1 },
        /// Range of regex match group no. 'group' relative to the sequence end
        /// (-<start>..-<end>).
        MatchGrpNegRange(Number) { group: usize, hit: String = String::from("1"), pattern: usize = 1 },
        /// Range of regex match group no. 'group' with '..' as delimiter and relative
        /// to the sequence end (-<start>..-<end>).
        MatchGrpDrange (Text) { group: usize, hit: String = String::from("1"), pattern: usize = 1 },
        /// Number of mismatches/insertions/deletions of the search pattern compared to the sequence
        /// (corresponds to edit distance). Either just `match_diffs` for the best match,
        /// or `match_diffs(h, [p])` to get the edit distance of the h-th best hit of
        /// the p-th pattern. `match_diffs('all', [p]) will return a comma delimited list of
        /// distances for all hits of a pattern.
        MatchDiffs(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Number of insertions in the sequence compared to the search pattern.
        /// Proportion of differences between the search pattern and the matched
        /// sequence, relative to the pattern length. See `match_diffs` for details on
        /// hit/pattern arguments.
        MatchDiffRate(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Name of the matching pattern if multiple patterns were supplied using
        /// `file:patterns.fasta`; or just `<pattern>` if a single pattern
        /// was specified in commandline. `pattern_name(rank)` allows selecting the n-th
        /// matching pattern (sorted by edit distance and/or pattern number)
        PatternName(Text) { rank: usize = 1 },
    }
}

// Variables

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum FindVarType {
    Start,
    End,
    Range(&'static str),
    NegRange(&'static str),
    NegStart,
    NegEnd,
    Diffs,
    DiffRate,
    Dist,
    Match,
    Name,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequestedHit {
    var_type: FindVarType,
    // hit position: None for a list of *all* hits
    hit_pos: Option<usize>,
    // match group: 0 for complete hit, 1.. for regex groups
    match_group: usize,
    // nth best matching pattern
    pattern_rank: usize,
}

#[derive(Debug)]
pub struct FindVars {
    // (symbol_id, settings)
    vars: VarStore<RequestedHit>,
    num_patterns: usize,
    max_hits: usize,    // usize::MAX for all hits
    groups: Vec<usize>, // match group numbers (0 = full hit)
    required_info: RequiredInfo,
}

impl FindVars {
    pub fn new(num_patterns: usize) -> FindVars {
        FindVars {
            vars: VarStore::default(),
            num_patterns,
            max_hits: 0,
            groups: Vec::new(),
            required_info: RequiredInfo::Exists,
        }
    }

    fn add_group(&mut self, group: usize) {
        if !self.groups.iter().any(|g| *g == group) {
            self.groups.push(group);
        }
    }

    pub fn _register_pos(&mut self, pos: usize, group: usize) {
        if pos >= self.max_hits {
            self.max_hits = pos + 1;
        }
        self.add_group(group);
    }

    pub fn _register_all(&mut self, group: usize) {
        self.max_hits = usize::MAX;
        self.add_group(group);
    }

    pub fn update_opts(&self, out: &mut Opts) {
        for g in &self.groups {
            if !out.groups.contains(g) {
                out.groups.push(*g);
            }
        }
        out.required_info = max(out.required_info, self.required_info);
        out.max_hits = max(out.max_hits, self.max_hits);
    }

    fn register_match(&mut self, h: &RequestedHit) -> Result<(), String> {
        if let Some(p) = h.hit_pos {
            self._register_pos(p, h.match_group);
        } else {
            self._register_all(h.match_group);
        }
        Ok(())
    }

    pub fn register_all(&mut self, group: usize) {
        self._register_all(group);
    }

    pub fn set_with(
        &mut self,
        rec: &dyn Record,
        matches: &Matches,
        symbols: &mut SymbolTable,
        text: &[u8],
    ) -> Result<(), String> {
        use FindVarType::*;
        for (symbol_id, req_hit) in self.vars.iter() {
            let out = symbols.get_mut(*symbol_id);
            if req_hit.var_type == Name {
                if let Some(opt_name) = matches.pattern_name(req_hit.pattern_rank) {
                    out.inner_mut()
                        .set_text(opt_name.unwrap_or("<pattern>").as_bytes());
                }
                continue;
            }

            // In the following, we use macros to avoid having to repeat the
            // code for setting a single value or a comma-separated list of
            // multiple values
            macro_rules! set_single {
                ($m:ident, ($fmt:expr, $($args:expr),*)) => {
                    write!(out.inner_mut().mut_text(), $fmt, $($args),*).unwrap()
                };
                ($m:ident, ($set_method:ident($a:expr))) => {
                    out.inner_mut().$set_method($a)
                };

            }

            macro_rules! set_multi {
                ($m:ident, $out:expr, (set_text($a:expr))) => {
                    $out.write_all($a)
                };
                ($m:ident, $out:expr, ($_:ident($a:expr))) => {
                    set_multi!($m, $out, ("{}", $a))
                };
                ($m:ident, $out:expr, ($fmt:expr, $($a:expr),*)) => {
                    write!($out, $fmt, $($a),*)
                };
            }

            macro_rules! impl_set_value {
                (($m:ident), $($variant:pat => $arg:tt),*) => {
                    if let Some(p) = req_hit.hit_pos.as_ref() {
                        // specific hits requested
                        if let Some($m) = matches.get_match(*p, req_hit.pattern_rank, req_hit.match_group) {
                            match req_hit.var_type {
                                $($variant => set_single!($m, $arg)),*,
                                _ => unreachable!(),
                            }
                            continue;
                        }
                    } else {
                        // List of all matches requested:
                        // This is different from above by requiring a string type
                        // in all cases instead of integers.
                        let not_empty = write_list_with(
                            matches.matches_iter(req_hit.pattern_rank, req_hit.match_group),
                            b",",
                            out.inner_mut().mut_text(),
                            |$m, o| match req_hit.var_type {
                                $($variant => set_multi!($m, o, $arg)),*,
                                _ => unreachable!(),
                            },
                        )
                        .unwrap();
                        if not_empty {
                            continue;
                        }
                    }
                    // important: reset previous value if nothing was found
                    out.set_none();
                };
            }

            // here we define how to obtain and set the individual values
            impl_set_value!((m),
                Start => (set_int((m.start + 1) as i64)),
                End => (set_int((m.end) as i64)),
                NegStart => (set_int(m.neg_start1(rec.seq_len()))),
                NegEnd => (set_int(m.neg_end1(rec.seq_len()))),
                Diffs => (set_int(m.dist as i64)),
                Range(delim) => ("{}{}{}", m.start + 1, delim, m.end),
                NegRange(delim) => (
                    "{}{}{}",
                    m.neg_start1(rec.seq_len()),
                    delim,
                    m.neg_end1(rec.seq_len())
                ),
                Match => (set_text(&text[m.start..m.end]))
            );
        }
        Ok(())
    }
}

impl VarProvider for FindVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(FindVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        if let Some((var, out_type)) = FindVar::from_func(name, args)? {
            use FindVar::*;
            use FindVarType::*;
            let (var_type, hit, pattern_rank, match_group) = match var {
                FindVar::Match { hit, pattern } => (FindVarType::Match, hit, pattern, 0),
                MatchGroup {
                    hit,
                    pattern,
                    group,
                } => (Match, hit, pattern, group),
                MatchDiffs { hit, pattern } => (Diffs, hit, pattern, 0),
                MatchDiffRate { hit, pattern } => (DiffRate, hit, pattern, 0),
                MatchStart { hit, pattern } => (Start, hit, pattern, 0),
                MatchNegStart { hit, pattern } => (NegStart, hit, pattern, 0),
                MatchEnd { hit, pattern } => (End, hit, pattern, 0),
                MatchNegEnd { hit, pattern } => (NegEnd, hit, pattern, 0),
                MatchRange { hit, pattern } => (Range("-"), hit, pattern, 0),
                MatchNegRange { hit, pattern } => (NegRange("-"), hit, pattern, 0),
                MatchDrange { hit, pattern } => (Range(".."), hit, pattern, 0),
                MatchNegDrange { hit, pattern } => (NegRange(".."), hit, pattern, 0),
                MatchGrpStart {
                    hit,
                    pattern,
                    group,
                } => (Start, hit, pattern, group),
                MatchGrpEnd {
                    hit,
                    pattern,
                    group,
                } => (End, hit, pattern, group),
                MatchGrpNegEnd {
                    hit,
                    pattern,
                    group,
                } => (NegEnd, hit, pattern, group),
                MatchGrpNegRange {
                    hit,
                    pattern,
                    group,
                } => (NegRange("-"), hit, pattern, group),
                MatchGrpRange {
                    hit,
                    pattern,
                    group,
                } => (Range("-"), hit, pattern, group),
                MatchGrpDrange {
                    hit,
                    pattern,
                    group,
                } => (Range(".."), hit, pattern, group),
                PatternName { rank } => (Name, "1".into(), rank, 0),
            };

            // parse hit number
            let hit_pos = if hit == "all" {
                None
            } else {
                let num: usize = hit
                    .parse()
                    .map_err(|_| format!("Invalid hit number: {}", hit))?;
                Some(num.checked_sub(1).ok_or("The hit number must be > 0")?)
            };

            // pattern rank:
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

            // update required_info if this variable needs more information than
            // already configured (passed on to `Matcher` objects)
            let required_info = match var_type {
                Diffs | DiffRate | Name => RequiredInfo::Distance,
                _ => RequiredInfo::Range,
            };
            if required_info > self.required_info {
                self.required_info = required_info;
            }

            let req_hit = RequestedHit {
                var_type,
                hit_pos,
                match_group,
                pattern_rank,
            };
            self.register_match(&req_hit)?;
            let symbol_id = builder.store_register(req_hit, &mut self.vars);
            return Ok(Some((symbol_id, out_type)));
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }
}
