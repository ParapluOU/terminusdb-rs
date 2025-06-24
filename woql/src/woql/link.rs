use crate::*;

/// Specify an edge pattern which is not terminal, but a link between objects.
ast_struct!(
    Link as link {
        /// A URI or variable which is the source or subject of the graph edge.
        subject: NodeValue,
        /// A URI or variable which is the edge-label or predicate of the graph edge.
        predicate: NodeValue,
        /// A URI or variable which is the target or object of the graph edge.
        object: Value,
        /// An optional graph (either 'instance' or 'schema')
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for Link {
    fn to_ast(&self) -> String {
        todo!()
    }
}