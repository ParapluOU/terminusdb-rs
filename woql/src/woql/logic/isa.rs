use crate::*;

/// Test (or generate) the type of an element.
ast_struct!(
    IsA as is_a {
        /// The element to test.
        element: NodeValue,
        /// The type of the element.
        r#type: NodeValue
    }
);

#[macro_export]
macro_rules! is_a {
    // safe use, create NodeValue from TerminusDBSchema type
    ($var:expr, $t:ty) => {
        is_a($var, node::<$t>())
    };
    // unsafe typing: create NodeValue from unchecked string
    ($var:expr, $t:expr) => {
        is_a($var, NodeValue::from($t))
    }
}

impl ToCLIQueryAST for IsA {
    fn to_ast(&self) -> String {
        format!("isa({}, {})", self.element.to_ast(), self.r#type.to_ast()).to_string()
    }
}