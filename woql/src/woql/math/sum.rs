use crate::*;

/// Sum a list of strings.
ast_struct!(
    Sum as sum {
        /// The list of numbers to sum.
        list: DataValue,
        /// The result of the sum as a number.
        result: DataValue
    }
);

impl ToCLIQueryAST for Sum {
    fn to_ast(&self) -> String {
        todo!()
    }
}