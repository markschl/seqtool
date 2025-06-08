use std::cmp::min;
use std::io;

use memchr::memmem;

use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::cmd::shared::item::Key;
use crate::helpers::{replace::replace_iter_custom, write_list::write_list};
use crate::var::{modules::VarProvider, parser::Arg, symbols::SymbolTable, VarBuilder, VarStore};

use super::DuplicateInfo;

variable_enum! {
    /// # Variables/functions provided by the 'unique' command
    ///
    /// # Examples
    ///
    /// De-replicate sequences using the sequence hash (faster than using
    /// the sequence `seq` itself), and also storing the number of duplicates
    /// (including the unique sequence itself) in the sequence header
    ///
    /// `st unique seqhash -a abund={n_duplicates} input.fasta > uniques.fasta`
    ///
    /// >id1 abund=3
    /// TCTTTAATAACCTGATTAG
    /// >id3 abund=1
    /// GGAGGATCCGAGCG
    /// (...)
    ///
    ///
    /// Store the complete list of duplicate IDs in the sequence header
    ///
    /// `st unique seqhash -a duplicates={duplicate_list} input.fasta > uniques.fasta`
    ///
    /// >id1 duplicates=id1,id2,id4
    /// TCTTTAATAACCTGATTAG
    /// >id3 duplicates=id3
    /// GGAGGATCCGAGCG
    /// (...)
    UniqueVar {
        /// The value of the unique key
        Key(?),
        /// The `n_duplicates` variable retuns the total number of duplicate records
        /// sharing the same unique key. It can also be used as a function `n_duplicates(false)`
        /// to exclude the returned unique record from the count.
        /// `n_duplicates` is short for `n_duplicates(true)`.
        NDuplicates(Number) { include_self: bool = true },
        /// Returns a comma-delimited list of record IDs that share the same unique key.
        /// Make sure that the record IDs don't have commas in them. The ID of the returned
        /// unique record is included by default (`duplicate_list` is short for `duplicate_list(true)`)
        /// but can be excluded with `duplicate_list(false)`.
        DuplicatesList(Text) { include_self: bool = true },
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
    replace_iter_custom(
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
                    write!(o, "{n}")?;
                }
                b'L' | b'l' => match data {
                    DuplicateInfo::Ids(i) => {
                        write_list(if code == b'L' { i } else { &i[1..] }, b",", o)?;
                    }
                    DuplicateInfo::Count(_) => unreachable!(),
                },
                _ => {
                    debug_assert_eq!(&matched[..PLACEHOLDER_PREFIX.len()], PLACEHOLDER_PREFIX);
                    o.write_all(matched)?;
                }
            }
            Ok(())
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
    // initial vector of vars (until initialization)
    vars: VarStore<UniqueVar>,
    // this will become Some(symbol ID) after the first record
    key_id: Option<usize>,
    // becomes true after first record
    initialized: bool,
    // needed, since this information will be lost after initialization
    has_vars: bool,
}

impl UniqueVars {
    pub fn required_info(&self) -> Option<RequiredInformation> {
        if self
            .vars
            .iter()
            .any(|(_, v)| matches!(v, UniqueVar::DuplicatesList { .. }))
        {
            Some(RequiredInformation::Ids)
        } else if self
            .vars
            .iter()
            .any(|(_, v)| matches!(v, UniqueVar::NDuplicates { .. }))
        {
            Some(RequiredInformation::Count)
        } else {
            None
        }
    }

    fn init(&mut self, symbols: &mut SymbolTable) {
        for (symbol_id, var) in self.vars.iter() {
            use UniqueVar::*;
            match var {
                Key => self.key_id = Some(*symbol_id),
                // Number of duplicates / duplicates list: set the placeholder
                // just once (will not change)
                NDuplicates { include_self } => {
                    let out = symbols.get_mut(*symbol_id).inner_mut().mut_text();
                    out.extend_from_slice(PLACEHOLDER_PREFIX);
                    out.push(if *include_self { b'D' } else { b'd' });
                }
                DuplicatesList { include_self } => {
                    let out = symbols.get_mut(*symbol_id).inner_mut().mut_text();
                    out.extend_from_slice(PLACEHOLDER_PREFIX);
                    out.push(if *include_self { b'L' } else { b'l' });
                }
            }
        }
    }

    pub fn set(&mut self, key: &Key, symbols: &mut SymbolTable) {
        if !self.initialized {
            self.init(symbols);
            self.initialized = true;
        }
        if let Some(symbol_id) = self.key_id {
            key.write_to_symbol(symbols.get_mut(symbol_id));
        }
    }
}

impl VarProvider for UniqueVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(UniqueVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        Ok(UniqueVar::from_func(name, args)?.map(|(var, out_type)| {
            self.has_vars = true;
            let symbol_id = builder.store_register(var, &mut self.vars);
            (symbol_id, out_type)
        }))
    }

    fn has_vars(&self) -> bool {
        self.has_vars
    }
}
