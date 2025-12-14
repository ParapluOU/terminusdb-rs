use crate::*;

/// Casts one type as another if possible.
ast_struct!(
    TypeCast as cast {
        /// The value to cast.
        value: Value,
        /// The type to which to cast.
        r#type: NodeValue,
        /// The resulting value after cast.
        result: Value
    }
);

impl ToCLIQueryAST for TypeCast {
    fn to_ast(&self) -> String {
        todo!()
    }
}