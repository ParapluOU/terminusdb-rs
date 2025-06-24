use crate::*;

/// A conditional which runs the then clause for every success from the test clause, otherwise runs the else clause.
ast_struct!(
    If as r#if {
        ///A query which will provide bindings for the then clause."
        test: Query,
        ///A query which will run for every solution of test with associated bindings."
        then: Query,
        ///A query which runs whenever test fails."
        r#else: Query
    }
);

impl ToCLIQueryAST for If {
    fn to_ast(&self) -> String {
        todo!()
    }
}