use crate::*;

impl std::convert::From<PredicateURI> for NodeValue {
    fn from(pred: PredicateURI) -> Self {
        Self::node(pred.into())
    }
}

ast_struct!(
    NodeValue {
        /// A URI representing a resource.
        node(NodeURI),
        ///A variable.
        variable(Variable)
    }
);

impl std::convert::From<&Variable> for NodeValue {
    fn from(v: &Variable) -> Self {
        Self::variable(v.clone())
    }
}

impl std::convert::From<&str> for NodeValue {
    fn from(v: &str) -> Self {
        Self::node(v.into())
    }
}

// todo: derive
impl ToCLIQueryAST for NodeValue {
    fn to_ast(&self) -> String {
        match self {
            NodeValue::node(inner) => inner.to_ast(),
            NodeValue::variable(inner) => inner.to_ast()
        }
    }
}