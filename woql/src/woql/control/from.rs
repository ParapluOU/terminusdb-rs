use crate::*;

/// Change the default read graph (between instance/schema).
ast_struct!(
    From as from {
        /// The subquery with a new default graph.
        query: Query,
        /// The graph filter: 'schema' or 'instance' or '*'
        graph: TargetGraphType // None = *
    }
);

impl ToCLIQueryAST for From {
    fn to_ast(&self) -> String {
        todo!()
    }
}