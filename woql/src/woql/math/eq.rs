use crate::*;

/// True whenever 'left' is the same as 'right'. Performs unification.
ast_struct!(
    Equals as eq {
        /// A URI, data value or variable.
        left: DataValue,
        /// A URI, data value or variable.
        right: DataValue
    }
);

impl ToCLIQueryAST for Equals {
    fn to_ast(&self) -> String {
        todo!()
    }
}