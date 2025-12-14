use crate::*;

/// Subtract two numbers.
ast_struct!(
    Times as times {
        /// First operand of add.
        left: ArithmeticExpression,
        /// Second operand of add.
        right: ArithmeticExpression
    }
);

impl ToCLIQueryAST for Times {
    fn to_ast(&self) -> String {
        todo!()
    }
}