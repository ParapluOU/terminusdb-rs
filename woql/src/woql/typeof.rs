use crate::*;

/// TypeOf gives the type of an element.
ast_struct!(
    TypeOf as type_of {
        /// The value of which to obtain the type.
        value: Value,
        /// The URI which that specifies the type.
        r#type: NodeValue
    }
);

// typeof
impl ToCLIQueryAST for TypeOf {
    fn to_ast(&self) -> String {
        todo!()
    }
}