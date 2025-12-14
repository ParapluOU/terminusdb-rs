use crate::*;

/// Extract the value of a key in a bound document.
ast_struct!(
    Dot as dot {
        /// Document which is being accessed.
        document: DataValue,
        /// The field from which the document which is being accessed.
        field: DataValue,
        /// The value for the document and field.
        value: DataValue
    }
);

impl ToCLIQueryAST for Dot {
    fn to_ast(&self) -> String {
        todo!()
    }
}