use crate::*;

use std::collections::BTreeSet;

/// A representation of a JSON style dictionary, but with free variables.
/// It is similar to an interpolated string in that it is a template with quoted data and substituted values.",
ast_struct!(
    DictionaryTemplate as dict_tpl {
        /// Pairs of Key-Values to be constructed into a dictionary
        data: (BTreeSet<FieldValuePair>)
    }
);

impl ToCLIQueryAST for DictionaryTemplate {
    fn to_ast(&self) -> String {
        todo!()
    }
}