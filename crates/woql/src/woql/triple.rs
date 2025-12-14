use crate::*;

/// Specify an edge pattern in the graph.
ast_struct!(
    Triple as triple {
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

#[macro_export]
macro_rules! t {
    ($subject:expr, $pred:expr, $obj:expr, $graph:expr) => {
        triple($subject, $pred, $obj, $graph)
    };
    ($subject:expr, $pred:expr, $obj:expr) => {
        triple($subject, $pred, $obj, Some(GraphType::Instance))
    }
}

impl ToCLIQueryAST for Triple {
    fn to_ast(&self) -> String {
        format!("t({},{},{})",
                self.subject.to_ast(),
                self.predicate.to_ast(),
                self.object.to_ast()
        ).to_string()
    }
}