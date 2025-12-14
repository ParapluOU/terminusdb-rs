use crate::*;

/// Change the default write graph (between instance/schema).
ast_struct!(
    Into as into {
        /// The subquery with a new default write graph."
        query: Query,
        /// The graph filter: 'schema' or 'instance' or '*'"
        graph: TargetGraphType // None = *
    }
);

impl ToCLIQueryAST for Into {
    fn to_ast(&self) -> String {
        todo!()
    }
}