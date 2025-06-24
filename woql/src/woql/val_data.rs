use crate::*;

// todo: fix casing. Rust wants capitalised variants
// but the TerminusDBSchema derive uses literal casing for sub-object names
ast_struct!(
    DataValue {
        /// A list of datavalues
        list(Vec<DataValue>),
        /// An xsd data type value.
        data(XSDAnySimpleType),
        /// A variable.
        variable(Variable)
    }
);

impl std::convert::From<&Variable> for DataValue {
    fn from(v: &Variable) -> Self {
        Self::variable(v.clone())
    }
}

// todo: derive
impl ToCLIQueryAST for DataValue {
    fn to_ast(&self) -> String {
        match self {
            DataValue::list(v) => v.to_ast(),
            DataValue::data(v) => v.to_ast(),
            DataValue::variable(v) => v.to_ast()
        }
    }
}