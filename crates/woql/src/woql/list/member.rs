use crate::*;

/// Generate or test every element of a list.
ast_struct!(
    Member as member {
        /// The element to test for membership or to supply as generated.
        member: DataValue,
        /// The list of elements against which to generate or test.
        list: DataValue
    }
);

impl ToCLIQueryAST for Member {
    fn to_ast(&self) -> String {
        todo!()
    }
}