//! This module contains a `VarProvider` for a 'key' variable, which is
//! used by the 'sort' and 'unique' commands.

use std::cmp::min;
use std::io;

use memchr::memmem;

use crate::error::CliResult;
use crate::helpers::{
    util::{replace_iter, write_list},
    value::SimpleValue,
};
use crate::var::{
    func::Func,
    symbols::{SymbolTable, VarType},
    VarBuilder, VarInfo, VarProvider, VarProviderInfo,
};
use crate::var_info;

use super::DuplicateInfo;

#[derive(Debug)]
pub struct UniqueVarInfo;

impl VarProviderInfo for UniqueVarInfo {
    fn name(&self) -> &'static str {
        "Unique command variables"
    }

    fn vars(&self) -> &[VarInfo] {
        &[
            var_info!(key => "The value of the unique key"),
            var_info!(n_duplicates([include_self]) =>
                "The `n_duplicates` variable retuns the total number of duplicate records \
                sharing the same unique key. It can also be used as a function `n_duplicates(true)` \
                or `n_duplicates(false)` to either include or exclude the returned unique record \
                from the count."
            ),
            var_info!(duplicates_list([include_self]) =>
                "Returns a comma-delimited list of record IDs that share the same unique key. \
                Make sure that the record IDs don't have commas in them. The ID of the returned \
                unique record is included by default \
                (`duplicate_list` is short for `duplicate_list(true)`), \
                but can be excluded with `duplicate_list(false)`."
            ),
        ]
    }
}

/// In initially formatted records, the 'n_duplicates' and 'duplicate_list' variables
/// are just placeholders, which consist of a prefix + single byte ('D' or 'L').
/// At the time of writing to output, the final number/ID list is known, so the
/// placeholders can be replaced using `fill_placeholder`.
/// The following prefix is extremely unlikely to be present in a record just by chance.
pub static PLACEHOLDER_PREFIX: &[u8] = b"\0d";

pub fn fill_placeholders<W>(
    formatted_record: &[u8],
    data: &DuplicateInfo,
    out: &mut W,
) -> io::Result<()>
where
    W: io::Write + ?Sized,
{
    replace_iter(
        formatted_record,
        memmem::find_iter(formatted_record, PLACEHOLDER_PREFIX).map(|start| {
            (
                start,
                min(start + PLACEHOLDER_PREFIX.len() + 1, formatted_record.len()),
            )
        }),
        out,
        |o, matched, _rest| {
            let code = *matched.last().unwrap();
            match code {
                b'D' | b'd' => {
                    let mut n = match data {
                        DuplicateInfo::Count(n) => *n,
                        DuplicateInfo::Ids(i) => i.len() as u64,
                    };
                    if code == b'd' {
                        n -= 1;
                    }
                    write!(o, "{}", n)
                }
                b'L' | b'l' => match data {
                    DuplicateInfo::Ids(i) => {
                        write_list(if code == b'L' { i } else { &i[1..] }, b",", o)
                    }
                    DuplicateInfo::Count(_) => unreachable!(),
                },
                _ => {
                    debug_assert_eq!(&matched[..PLACEHOLDER_PREFIX.len()], PLACEHOLDER_PREFIX);
                    o.write_all(matched)
                }
            }
        },
    )?;
    Ok(())
}

#[derive(Debug, Copy, Clone)]
pub enum RequiredInformation {
    Count,
    Ids,
}

#[derive(Debug, Default)]
pub struct UniqueVars {
    // unique key (var ID)
    key_id: Option<usize>,
    // number of duplicates: (var ID, include self)
    size_id: Option<(usize, bool)>,
    // list of duplicate IDs: (var id, include self)
    id_list: Option<(usize, bool)>,
}

impl UniqueVars {
    pub fn required_info(&self) -> Option<RequiredInformation> {
        if self.id_list.is_some() {
            Some(RequiredInformation::Ids)
        } else if self.size_id.is_some() {
            Some(RequiredInformation::Count)
        } else {
            None
        }
    }

    pub fn set(&mut self, key: &SimpleValue, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.key_id {
            let v = symbols.get_mut(var_id);
            match key {
                SimpleValue::Text(t) => v.inner_mut().set_text(t),
                SimpleValue::Number(n) => v.inner_mut().set_float(n.0),
                SimpleValue::None => v.set_none(),
            }
        }
        // Number of duplicates / duplicates list: set the placeholder
        // just once (will not change)
        if let Some((var_id, include_self)) = self.size_id.take() {
            let out = symbols.get_mut(var_id).inner_mut().mut_text();
            out.extend_from_slice(PLACEHOLDER_PREFIX);
            out.push(if include_self { b'D' } else { b'd' });
        }
        if let Some((var_id, include_self)) = self.id_list.take() {
            let out = symbols.get_mut(var_id).inner_mut().mut_text();
            out.extend_from_slice(PLACEHOLDER_PREFIX);
            out.push(if include_self { b'L' } else { b'l' });
        }
    }
}

impl VarProvider for UniqueVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &UniqueVarInfo
    }

    fn allow_nested(&self) -> bool {
        false
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        let id = b.symbol_id();
        let ty = match var.name.as_str() {
            "key" => {
                self.key_id = Some(id);
                None
            }
            "n_duplicates" => {
                self.size_id = Some((id, var.opt_arg_as(0).transpose()?.unwrap_or(true)));
                Some(VarType::Int)
            }
            "duplicates_list" => {
                self.id_list = Some((id, var.opt_arg_as(0).transpose()?.unwrap_or(true)));
                Some(VarType::Text)
            }
            _ => unreachable!(),
        };
        Ok(ty)
    }

    fn has_vars(&self) -> bool {
        self.key_id.is_some() || self.size_id.is_some() || self.id_list.is_some()
    }
}
