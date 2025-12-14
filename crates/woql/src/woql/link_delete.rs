use crate::*;

/// "Delete an edge linking nodes.
ast_struct!(
    DeleteLink as delete_link {
        /// A URI or variable which is the source or subject of the graph edge. The variable must be bound."
        subject: NodeValue,
        /// A URI or variable which is the edge-label or predicate of the graph edge. The variable must be bound.
        predicate: NodeValue,
        /// A URI or variable which is the target or object of the graph edge. The variable must be bound.
        object: NodeValue,
        /// An optional graph (either 'instance' or 'schema')
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for DeleteLink {
    fn to_ast(&self) -> String {
        todo!()
    }
}