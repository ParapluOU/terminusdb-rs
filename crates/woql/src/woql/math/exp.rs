use crate::*;

/// Exponentiate a number
ast_struct!(
    Exp as exp {
        /// First operand of add.
        left: ArithmeticExpression,
        /// Second operand of add.
        right: ArithmeticExpression
    }
);

impl ToCLIQueryAST for Exp {
    fn to_ast(&self) -> String {
        todo!()
    }
}