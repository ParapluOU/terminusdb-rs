use crate::*;

/// Specify an edge to add to the graph.
ast_struct!(
    AddTriple as add_triple {
        /// A URI or variable which is the source or subject of the graph edge.
        subject: NodeValue,
        /// A URI or variable which is the edge-label or predicate of the graph edge.
        predicate: NodeValue,
        /// A URI, datatype or variable which is the target or object of the graph edge.
        object: Value,
        /// An optional graph (either 'instance' or 'schema')
        graph: (TargetGraphType)
    }
);

impl ToCLIQueryAST for AddTriple {
    fn to_ast(&self) -> String {
        todo!()
    }
}