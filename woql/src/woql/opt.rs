use crate::*;

/// A query which will succeed (without bindings) even in the case of failure.
ast_struct!(
    Optional as opt {
        query: Query
    }
);

impl ToCLIQueryAST for Optional {
    fn to_ast(&self) -> String {
        todo!()
    }
}