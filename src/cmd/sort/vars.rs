use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::cmd::shared::sort_item::Key;
use crate::var::{modules::VarProvider, parser::Arg, symbols::SymbolTable, VarBuilder};

variable_enum! {
    /// # Variables provided by the 'sort' command
    ///
    /// # Examples
    ///
    /// Sort sequences by their length and store the length in the sequence
    /// header
    ///
    /// `st sort seqlen input.fasta > output.fasta`
    ///
    /// >id10 seqlen=3
    /// SEQ
    /// >id3 seqlen=5
    /// SEQUE
    /// >id1 seqlen=8
    /// SEQUENCE
    ///
    ///
    /// Sort sequences by (1) a 'primer' attribute in the header, which may have
    /// been obtained using the 'find' command (see `st find --help-vars`), and
    /// (2) their length. Again, we write the key to the output
    /// header
    ///
    /// `st sort seqlen -a key='{key}' input.fasta > output.fasta`
    ///
    /// >id3 primer=p1 key=p1,5
    /// SEQUE
    /// >id1 primer=p1 key=p1,8
    /// SEQUENCE
    /// >id2 primer=p2 key=p2,3
    /// SEQ
    SortVar {
        /// The value of the key used for sorting
        Key(?),
    }
}

#[derive(Debug, Default)]
pub struct SortVars {
    key_id: Option<usize>,
}

impl SortVars {
    pub fn set(&mut self, key: &Key, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.key_id {
            key.write_to_symbol(symbols.get_mut(var_id));
        }
    }
}

impl VarProvider for SortVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(SortVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        Ok(SortVar::from_func(name, args)?.map(|(var, out_type)| {
            let SortVar::Key = var;
            let symbol_id = self.key_id.get_or_insert_with(|| builder.increment());
            (*symbol_id, out_type)
        }))
    }

    fn has_vars(&self) -> bool {
        self.key_id.is_some()
    }
}
