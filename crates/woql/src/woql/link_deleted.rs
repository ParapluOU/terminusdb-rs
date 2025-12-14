use crate::*;

/// An edge pattern specifying a link beween nodes deleted *at this commit*.
ast_struct!(
    DeletedLink as deleted_link {
        /// A URI or variable which is the source or subject of the graph edge."
        subject: NodeValue,
        /// A URI or variable which is the edge-label or predicate of the graph edge.
        predicate: NodeValue,
        /// A URI or variable which is the target or object of the graph edge. The variable must be bound.
        object: NodeValue,
        /// An optional graph (either 'instance' or 'schema')
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for DeletedLink {
    fn to_ast(&self) -> String {
        todo!()
    }
}