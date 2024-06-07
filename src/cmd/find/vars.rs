use std::cmp::max;
use std::io::Write;

use bio::alignment::AlignmentOperation;
use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::helpers::write_list::write_list_with;
use crate::io::Record;
use crate::var::{modules::VarProvider, parser::Arg, symbols::SymbolTable, VarBuilder, VarStore};

use super::matcher::regex::resolve_group;
use super::matches::Matches;
use super::opts::{Opts, RequiredInfo};

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
    /// The result will be 'undefined' (=undefined in JavaScript) if there are > 2 mismatches
    ///
    /// `st find -d 2 CTTGGTCATTTAGAGGAAGTAA -a rng={match_range} -a dist={match_diffs} reads.fasta`
    ///
    /// >id1 rng=2:21 dist=1
    /// SEQUENCE
    /// >id2 rng=1:20 dist=0
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
        /// The text matched by the pattern.
        /// With approximate matching (`-D/--diffs` > 0), this is the match with the
        /// smallest edit distance or the leftmost occurrence if `--in-order` was specified.
        /// With exact/regex matching, the leftmost hit is always returned.
        /// In case of multiple patterns in a pattern file, the best hit of the
        /// best-matching pattern is returned (fuzzy matching), or the first
        /// hit of the first pattern with an exact match.
        ///
        /// `match(hit) returns the matched text of the given hit number,
        /// whereas `match(all)` or `match('all') returns a comma-delimited
        /// list of all hits. These are either sorted by the edit distance
        /// (default) or by occurrence (`--in-order` or exact matching).
        ///
        /// `match(1, 2)`, `match(1, 3)`, etc. references the 2nd, 3rd, etc.
        /// best matching pattern in case multiple patterns were suplied in a
        /// file (default: hit=1, pattern=1)."
        Match(Text) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Text match aligned with the pattern, including gaps if needed.
        AlignedMatch(Text) { hit: String = String::from("1"), rank: usize = 1 },
        /// Start coordinate of the first/best match. Other hits/patterns are selected
        /// with `match_start(hit, [pattern])`, for details see `match`
        MatchStart(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Start of the first/best match relative to sequence end (negative coordinate).
        /// Other hits/patterns are selected with `match_neg_start(hit, [pattern])`,
        /// for details see `match`.
        MatchEnd(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// End of the first/best match relative to sequence end (negative coordinate).
        /// Other hits/patterns are selected with `match_neg_end(hit, [pattern])`,
        /// for details see `match`.
        MatchNegStart(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// End coordinate of the first/best match. Other hits/patterns are selected
        /// with `match_end(hit, [pattern])`, for details see `match`
        MatchNegEnd(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Length of the match
        MatchLen(Number) { hit: String = String::from("1"), rank: usize = 1 },
        /// Range (start:end) of the first/best match. Other hits/patterns are selected
        /// with `match_range(hit, [pattern])`, for details see `match`.
        /// The 3rd argument allows changing the range delimiter, e.g. to '-'.
        MatchRange(Number) {
            hit: String = String::from("1"),
            pattern: usize = 1,
            delim: String = String::from(":")
        },
        /// Text matched by regex match group of given number (0 = entire match)
        /// or name in case of a named group: `(?<name>...)`.
        /// The hit number (sorted by edit distance or occurrence) and the pattern
        /// number can be specified as well (see `match` for details).
        MatchGroup(Text) { group: String, hit: String = String::from("1"), pattern: usize = 1 },
        /// Start coordinate of the regex match group 'group' within the first/best match.
        /// See 'match_group' for options and details.
        MatchGrpStart(Number) { group: String, hit: String = String::from("1"), pattern: usize = 1 },
        /// End coordinate of the regex match group 'group' within the first/best match.
        /// See 'match_group' for options and details.
        MatchGrpEnd(Number) { group: String, hit: String = String::from("1"), pattern: usize = 1 },
        /// Start coordinate of regex match group 'group' relative to the sequence end (negative number).
        /// See 'match_group' for options and details.
        MatchGrpNegStart(Number) { group: String, hit: String = String::from("1"), pattern: usize = 1 },
        /// Start coordinate of regex match group 'group' relative to the sequence end (negative number).
        /// See 'match_group' for options and details.
        MatchGrpNegEnd(Number) { group: String, hit: String = String::from("1"), pattern: usize = 1 },
        /// Range (start-end) of regex match group 'group' relative to the sequence end.
        /// See 'match_group' for options and details.
        /// The 4th argument allows changing the range delimiter, e.g. to '-'.
        MatchGrpRange(Number) {
            group: String,
            hit: String = String::from("1"),
            pattern: usize = 1,
            delim: String = String::from(":")
        },
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
        /// Number of insertions in the matched sequence compared to the search pattern.
        MatchIns(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Number of deletions in the matched text sequence to the search pattern.
        MatchDel(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Number of substitutions (non-matching letters) in the matched sequence compared
        /// to the pattern
        MatchSubst(Number) { hit: String = String::from("1"), pattern: usize = 1 },
        /// Name of the matching pattern (patterns supplied with `file:patterns.fasta`).
        /// In case a single pattern was specified in the commandline, this will just be `<pattern>`.
        /// `pattern_name(rank)` selects the n-th matching pattern, sorted by edit distance
        /// and/or pattern number (depending on `-D/-R` and `--in-order`).
        PatternName(Text) { rank: usize = 1 },
        /// The best-matching pattern sequence, or the n-th matching pattern if `rank` is given,
        /// sorted by edit distance or by occurrence (depending on `-D/-R` and `--in-order`).
        Pattern(Text) { rank: usize = 1 },
        /// The aligned pattern, including gaps if needed.
        /// Regex patterns are returned as-is.
        AlignedPattern(Text) { hit: String = String::from("1"), rank: usize = 1 },
        /// Length of the matching pattern (see also `pattern`). For regex patterns, the length
        /// of the complete regular expression is returned.
        PatternLen(Number) { rank: usize = 1 },
    }
}

// Variables

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum FindVarType {
    Start,
    End,
    Range(String),
    NegStart,
    NegEnd,
    Diffs,
    DiffRate,
    Ins,
    Del,
    Subst,
    Match,
    AlignedMatch,
    MatchLen,
    Name,
    Pattern,
    AlignedPattern,
    PatternLen,
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
    vars: VarStore<RequestedHit>,
    // we need a copy of the patterns here to check for regex groups
    patterns: Vec<String>,
    max_hits: usize, // usize::MAX for all hits
    regex: bool,
    groups: Vec<usize>, // match group numbers (0 = full hit)
    required_info: RequiredInfo,
}

impl FindVars {
    pub fn new(patterns: Vec<String>, regex: bool) -> FindVars {
        FindVars {
            vars: VarStore::default(),
            patterns,
            max_hits: 0,
            groups: Vec::new(),
            regex,
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
        for (symbol_id, req_hit) in self.vars.iter() {
            let out = symbols.get_mut(*symbol_id);

            // In the following, we use macros to avoid having to repeat the
            // code for setting a single value or a comma-separated list of
            // multiple values
            macro_rules! set_single {
                ($m:ident, ($fmt:expr, $($args:expr),*)) => {
                    write!(out.inner_mut().mut_text(), $fmt, $($args),*).unwrap()
                };
                ($m:ident, (with_text($func:expr))) => {
                    $func(out.inner_mut().mut_text())
                };
                ($m:ident, ($set_method:ident($a:expr))) => {
                    out.inner_mut().$set_method($a)
                };
            }

            macro_rules! set_multi {
                ($m:ident, $out:expr, (set_text($a:expr))) => {
                    $out.write_all($a)
                };
                ($m:ident, $out:expr, (with_text($func:expr))) => {
                    Ok($func($out))
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
            use FindVarType::*;
            impl_set_value!((m),
                Start => (set_int((m.start + 1) as i64)),
                End => (set_int((m.end) as i64)),
                NegStart => (set_int(m.neg_start1(rec.seq_len()))),
                NegEnd => (set_int(m.neg_end1(rec.seq_len()))),
                Diffs => (set_int(m.dist as i64)),
                DiffRate => (set_float(
                    m.dist as f64 / matches.pattern(req_hit.pattern_rank).unwrap().len() as f64
                )),
                Range(ref delim) => ("{}{}{}", m.start + 1, delim, m.end),
                Ins => (set_int(count_aln_op(&m.alignment_path, AlignmentOperation::Del) as i64)),
                Del => (set_int(count_aln_op(&m.alignment_path, AlignmentOperation::Ins) as i64)),
                Subst => (set_int(count_aln_op(&m.alignment_path, AlignmentOperation::Subst) as i64)),
                Match => (set_text(&text[m.start..m.end])),
                MatchLen => (set_int((m.end - m.start) as i64)),
                Name => (set_text(
                    matches
                        .pattern_name(req_hit.pattern_rank)
                        .unwrap()
                        .unwrap_or("<pattern>")
                        .as_bytes())
                ),
                Pattern => (set_text(matches.pattern(req_hit.pattern_rank).unwrap().as_bytes())),
                PatternLen => (set_int(matches.pattern(req_hit.pattern_rank).unwrap().len() as i64)),
                AlignedPattern => (with_text(
                    |t| align_pattern(
                        matches.pattern(req_hit.pattern_rank).unwrap().as_bytes(),
                        &m.alignment_path,
                        t
                    )
                )),
                AlignedMatch => (with_text(
                    |t| align_match(&text[m.start..m.end], &m.alignment_path, t)
                ))
            );
        }
        Ok(())
    }
}

fn count_aln_op(path: &[AlignmentOperation], op: AlignmentOperation) -> usize {
    path.iter().filter(|&&x| x == op).count()
}

fn align_pattern(pattern: &[u8], path: &[AlignmentOperation], out: &mut Vec<u8>) {
    if path.is_empty() {
        // empty path: exact/regex matching
        out.extend_from_slice(pattern);
    } else {
        use AlignmentOperation::*;
        let mut pattern = pattern.iter();
        for op in path {
            match op {
                Match | Subst | Ins => out.push(*pattern.next().unwrap()),
                Del => out.push(b'-'),
                _ => {}
            }
        }
    }
}

fn align_match(text: &[u8], path: &[AlignmentOperation], out: &mut Vec<u8>) {
    if path.is_empty() {
        // empty path: exact/regex matching
        out.extend_from_slice(text);
    } else {
        use AlignmentOperation::*;
        let mut text = text.iter();
        for op in path {
            match op {
                Match | Subst | Del => out.push(*text.next().unwrap()),
                Ins => out.push(b'-'),
                _ => {}
            }
        }
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
                FindVar::Match { hit, pattern } => (FindVarType::Match, hit, pattern, None),
                FindVar::AlignedMatch { hit, rank } => (FindVarType::AlignedMatch, hit, rank, None),
                FindVar::MatchLen { hit, rank } => (FindVarType::MatchLen, hit, rank, None),
                MatchGroup {
                    hit,
                    pattern,
                    group,
                } => (Match, hit, pattern, Some(group)),
                MatchDiffs { hit, pattern } => (Diffs, hit, pattern, None),
                MatchIns { hit, pattern } => (Ins, hit, pattern, None),
                MatchDel { hit, pattern } => (Del, hit, pattern, None),
                MatchSubst { hit, pattern } => (Subst, hit, pattern, None),
                MatchDiffRate { hit, pattern } => (DiffRate, hit, pattern, None),
                MatchStart { hit, pattern } => (Start, hit, pattern, None),
                MatchEnd { hit, pattern } => (End, hit, pattern, None),
                MatchNegStart { hit, pattern } => (NegStart, hit, pattern, None),
                MatchNegEnd { hit, pattern } => (NegEnd, hit, pattern, None),
                MatchRange {
                    hit,
                    pattern,
                    ref delim,
                } => (Range(delim.clone()), hit, pattern, None),
                MatchGrpStart {
                    hit,
                    pattern,
                    group,
                } => (Start, hit, pattern, Some(group)),
                MatchGrpEnd {
                    hit,
                    pattern,
                    group,
                } => (End, hit, pattern, Some(group)),
                MatchGrpNegStart {
                    hit,
                    pattern,
                    group,
                } => (NegStart, hit, pattern, Some(group)),
                MatchGrpNegEnd {
                    hit,
                    pattern,
                    group,
                } => (NegEnd, hit, pattern, Some(group)),
                MatchGrpRange {
                    hit,
                    pattern,
                    group,
                    ref delim,
                } => (Range(delim.clone()), hit, pattern, Some(group)),
                PatternName { rank } => (Name, "1".into(), rank, None),
                FindVar::Pattern { rank } => (FindVarType::Pattern, "1".into(), rank, None),
                FindVar::AlignedPattern { hit, rank } => {
                    (FindVarType::AlignedPattern, hit, rank, None)
                }
                FindVar::PatternLen { rank } => (FindVarType::PatternLen, "1".into(), rank, None),
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
            debug_assert!(self.patterns.len() > 0);
            if pattern_rank > self.patterns.len() {
                return fail!(format!(
                    "Pattern rank {} requested, but there are only {} patterns",
                    pattern_rank,
                    self.patterns.len()
                ));
            }
            let pattern_rank = pattern_rank
                .checked_sub(1)
                .ok_or("The pattern rank must be > 0")?;

            // resolve match group
            let match_group = match_group.as_deref().unwrap_or("0");
            let match_group = if match_group == "0" {
                0
            } else if !self.regex {
                return Err(format!(
                    "Regex group '{}' was requested, but groups other than '0' (the whole hit) \
                    are not supported for non-regex patterns. Did you forget to enable regex matching with
                    `-r/--regex`?", match_group));
            } else {
                let mut num = None;
                for pattern in &self.patterns {
                    let _n = resolve_group(pattern, match_group)?;
                    if let Some(n) = num {
                        if n != _n {
                            return Err(format!(
                                "Named group '{}' does not resolve to the same group number in all patterns.\
                                This is a requirement in the case of multiple regex patterns. \
                                Consider using simple group numbers instead.",
                                match_group,
                            ));
                        }
                    } else {
                        num = Some(_n);
                    }
                }
                num.unwrap()
            };

            // update required_info if this variable needs more information than
            // already configured (passed on to `Matcher` objects)
            let required_info = match var_type {
                Diffs | DiffRate | Name => RequiredInfo::Distance,
                Ins | Del | Subst | AlignedPattern | AlignedMatch => RequiredInfo::Alignment,
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
