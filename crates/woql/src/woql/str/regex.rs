use crate::*;

/// Test a string against a PCRE style regex pattern.
ast_struct!(
    Regexp as regex {
        pattern: DataValue,
        string: DataValue,
        result: (Option<DataValue>)
    }
);

impl ToCLIQueryAST for Regexp {
    fn to_ast(&self) -> String {
        todo!()
    }
}