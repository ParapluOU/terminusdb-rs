use crate::*;

/// Predicate determining if one thing is less than another according to natural ordering.
ast_struct!(
    Less as less {
        /// The lesser element.
        left: DataValue,
        /// The greater element.
        right: DataValue
    }
);

impl ToCLIQueryAST for Less {
    fn to_ast(&self) -> String {
        todo!()
    }
}