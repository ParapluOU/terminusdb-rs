use crate::*;

/// A representation of a JSON style dictionary, but with free variables. It is similar to an interpolated string in that it is a template with quoted data and substituted values.",
ast_struct!(
    FieldValuePair as field_val_pair {
        /// The field or key of a dictionary value pair
        field: String,
        /// The value of a dictionary value pair.
        value: Value
    }
);

impl ToCLIQueryAST for FieldValuePair {
    fn to_ast(&self) -> String {
        todo!()
    }
}