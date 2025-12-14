use crate::*;

/// Evaluate an arithmetic expression to obtain a result.
ast_struct!(
    Eval as eval {
        /// The expression to be evaluated.
        expression: ArithmeticExpression,
        /// The numeric result.
        result: ArithmeticValue
    }
);

impl ToCLIQueryAST for Eval {
    fn to_ast(&self) -> String {
        todo!()
    }
}