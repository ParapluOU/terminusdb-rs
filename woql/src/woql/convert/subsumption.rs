use crate::*;

/// Provides class subsumption (the inheritance model) according to the schema design.
ast_struct!(
    Subsumption as subsumption {
        /// The child class as a URI or variable.
        child: NodeValue,
        /// The parent class as a URI or variable
        parent: NodeValue
    }
);

impl ToCLIQueryAST for Subsumption {
    fn to_ast(&self) -> String {
        todo!()
    }
}