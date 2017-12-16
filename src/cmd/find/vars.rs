use std::io::Write;

use var;
use error::CliResult;
use io::Record;

use super::*;

pub struct FindVarHelp;

impl VarHelp for FindVarHelp {
    fn name(&self) -> &'static str {
        "Pattern finding variables"
    }
    fn usage(&self) -> &'static str {
        "find:<variable>[.pattern_rank][:match_num][:group]"
    }
    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            ("match", "The matched sequence/pattern"),
            ("start", "Start of the match."),
            ("end",   "End of the match."),
            ("dist", "Distance of the matched sequence compared to the pattern. Normally, this is \
              the edit distance, unless --gapw is used"),
            ("neg_start", "Start of the match relative to sequence end (negative number)"),
            ("neg_end",   "End of the match relative to sequence end (negative number)"),
            ("range",  "Range of the match in the form start-end"),
            ("drange",
            "Range of the match with two dots as delimiter (start..end). Useful with 'trim'\
            and 'mask'"),
            ("neg_drange",
            "Range of the match (dot delimiter) relative to the sequence end (-<start>..-<end>)"),
            ("name",
            "Name of the best matching pattern if there are multiple (read from pattern file)"),
        ])
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[])
    }
}

// Variables

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Var {
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

use self::Var::*;

#[derive(Debug)]
pub struct FindVars {
    // (var, var_id, position, pattern_rank)
    // position: Some((pos, group)), or None for all hits
    vars: Vec<(Var, usize, Option<usize>, usize, usize)>,
    pos: SearchPositions,
    bounds_needed: (bool, bool),
}

impl FindVars {
    pub fn new() -> FindVars {
        FindVars {
            vars: vec![],
            pos: SearchPositions::new(),
            bounds_needed: (false, false),
        }
    }

    // returns Option<(match_index, group_index)>
    // returns None if not one, but all indices were requested
    fn parse_pos(&self, code: &str) -> CliResult<(Option<usize>, usize)> {
        let parts: Vec<&str> = code.splitn(2, ':').collect();

        let match_idx = match parts[0] {
            "" => Some(0),
            "all" => None,
            _ => {
                let i: usize = parts[0]
                    .parse()
                    .map_err(|_| format!("Invalid match index: {}", parts[0]))?;
                Some(i.checked_sub(1)
                    .ok_or("The match index must be greater than zero")?)
            }
        };

        let group = match parts.len() {
            1 => 0,
            2 => if parts[1] == "" {
                0
            } else {
                parts[1]
                    .parse()
                    .map_err(|_| format!("Invalid group number: {}", parts[1]))?
            },
            // } else {
            //     MatchGroup::Named(parts[1].to_string())
            _ => unreachable!(),
        };

        Ok((match_idx, group))
    }

    /// returns: (name, positions, pattern_rank)
    /// where positions = Some(hit_index, group_index) or None if all hits were requested
    pub fn parse_code<'a>(
        &self,
        code: &'a str,
    ) -> CliResult<(&'a str, Option<usize>, usize, usize)> {
        let mut parts: Vec<&str> = code.splitn(2, ':').collect();

        let mut name = parts.remove(0);

        // pattern rank:
        let mut name_parts: Vec<&str> = name.splitn(2, '.').collect();
        let mut pattern_rank = if name_parts.len() == 2 {
            name_parts[1]
                .parse()
                .map_err(|_| format!("Invalid search pattern number: {}", name_parts[1]))?
        } else {
            1
        };

        if pattern_rank == 0 {
            return fail!("Pattern rank must be > 0");
        }
        pattern_rank -= 1;

        name = name_parts.remove(0);

        let match_code = if parts.len() == 1 {
            parts.remove(0)
        } else {
            "1"
        };

        let (pos, group) = self.parse_pos(match_code)?;

        Ok((name, pos, group, pattern_rank))
    }

    /// returns match ID or
    /// Ok(None) if not one, but all indices were requested
    fn register_match(
        &mut self,
        var: Var,
        var_id: usize,
        pos: Option<usize>,
        group: usize,
        rank: usize,
    ) -> CliResult<()> {
        self.vars.push((var, var_id, pos, group, rank));

        if let Some(p) = pos {
            self.pos.register_pos(p, group);
        } else {
            self.pos.register_all(group);
        }

        Ok(())
    }

    pub fn register_all(&mut self, group: usize) {
        self.pos.register_all(group);
    }

    pub fn positions(&self) -> &SearchPositions {
        &self.pos
    }

    pub fn bounds_needed(&self) -> (bool, bool) {
        self.bounds_needed
    }

    pub fn set_with(
        &mut self,
        rec: &Record,
        matches: &Matches,
        symbols: &mut var::symbols::Table,
        text: &[u8],
    ) -> CliResult<()> {

        for &(ref var, var_id, ref position, group, pattern_rank) in &self.vars {
            if *var == Name {
                let name = matches.pattern_name(pattern_rank).unwrap_or("");
                symbols.set_text(var_id, name.as_bytes());
                continue;
            }

            if let Some(pos) = position.as_ref() {
                // specific hits requested
                if let Some(m) = matches.get_match(*pos, group, pattern_rank) {
                    match *var {
                        Start => symbols.set_int(var_id, (m.start + 1) as i64),
                        End => symbols.set_int(var_id, (m.end) as i64),
                        NegStart => symbols.set_int(var_id, m.neg_start1(rec.seq_len())),
                        NegEnd => symbols.set_int(var_id, m.neg_end1(rec.seq_len())),
                        Dist => symbols.set_int(var_id, m.dist as i64),
                        Range(ref delim) => write!(
                            symbols.mut_text(var_id),
                            "{}{}{}", m.start + 1, delim, m.end
                        )?,
                        NegRange(ref delim) => write!(
                            symbols.mut_text(var_id), "{}{}{}",
                            m.neg_start1(rec.seq_len()), delim, m.neg_end1(rec.seq_len())
                        )?,
                        Match => symbols.set_text(var_id, &text[m.start..m.end]),
                        _ => unreachable!(),
                    }
                    continue;
                } else {
                    // important: reset previous value
                    symbols.set_none(var_id);
                }
            } else {
                // list of all matches requested
                let out = symbols.mut_text(var_id);

                for maybe_m in matches.matches_iter(pattern_rank, group) {
                    if let Some(m) = maybe_m {
                        match *var {
                            Start => write!(out, "{}", m.start + 1)?,
                            End => write!(out, "{}", m.end)?,
                            NegStart => write!(out, "{}", m.neg_start1(rec.seq_len()))?,
                            NegEnd => write!(out, "{}", m.neg_end1(rec.seq_len()))?,
                            Dist => write!(out, "{}", m.dist)?,
                            Range(ref delim) => write!(out, "{}{}{}", m.start + 1, delim, m.end)?,
                            NegRange(ref delim) => write!(
                                out, "{}{}{}",
                                m.neg_start1(rec.seq_len()), delim, m.neg_end1(rec.seq_len())
                            )?,
                            Match => out.extend_from_slice(&text[m.start..m.end]),
                            _ => unreachable!(),
                        }
                        out.push(b',');
                    }
                }
                // remove last comma
                out.pop();
            }
        }
        Ok(())
    }
}

impl VarProvider for FindVars {
    fn prefix(&self) -> Option<&str> {
        Some("f")
    }
    fn name(&self) -> &'static str {
        "matches"
    }

    fn register_var(
        &mut self,
        code: &str,
        var_id: usize,
        _: &mut var::VarStore,
    ) -> CliResult<bool> {
        let (name, pos, group, rank) = self.parse_code(code)?;

        let var = match name {
            "start" => Start,
            "end" => End,
            "range" => Range("-".into()),
            "drange" => Range("..".into()),
            "neg_drange" => NegRange("..".into()),
            "neg_start" => NegStart,
            "neg_end" => NegEnd,
            "dist" => Dist,
            "match" => Match,
            "name" => Name,
            _ => return Ok(false),
        };

        if var != End && var != Dist && var != Name {
            self.bounds_needed.0 = true;
        }
        if var != Start && var != Dist && var != Name {
            self.bounds_needed.1 = true;
        }

        self.register_match(var, var_id, pos, group, rank)?;
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }
}
