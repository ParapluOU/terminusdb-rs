use crate::*;

/// Specify an edge pattern to remove from the graph.
ast_struct!(
    DeleteTriple as delete_triple {
        /// A URI or variable which is the source or subject of the graph edge.
        subject: NodeValue,
        /// A URI or variable which is the edge-label or predicate of the graph edge.
        predicate: NodeValue,
        /// A URI, datatype or variable which is the target or object of the graph edge.
        object: Value,
        /// An optional graph (either 'instance' or 'schema')
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for DeleteTriple {
    fn to_ast(&self) -> String {
        todo!()
    }
}