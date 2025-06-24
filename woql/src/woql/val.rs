use crate::*;

// todo: allow configure the use of casing in TerminusDBSchema derive
/// A variable, node or data point.
ast_struct!(
    Value {
        /// a dictionary mapping of values
        dictionary(DictionaryTemplate),
        /// A list of datavalues
        list(Vec<Value>),
        /// A URI representing a resource.
        node(NodeURI),
        /// A variable.
        variable(Variable),
        /// An xsd data type value.
        data(XSDAnySimpleType)
    }
);

impl std::convert::From<&Variable> for Value {
    fn from(v: &Variable) -> Self {
        Self::variable(v.clone())
    }
}

// todo: derive
impl ToCLIQueryAST for Value {
    fn to_ast(&self) -> String {
        match self {
            Value::dictionary(inner) => inner.to_ast(),
            Value::list(inner) => inner.to_ast(),
            Value::node(inner) => inner.to_ast(),
            Value::variable(inner) => inner.to_ast(),
            Value::data(inner) => inner.to_ast(),
        }
    }
}

impl std::convert::From<PredicateURI> for Value {
    fn from(pred: PredicateURI) -> Self {
        Self::node(
            std::convert::Into::into(pred)
        )
    }
}