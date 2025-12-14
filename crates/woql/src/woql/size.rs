use crate::*;

/// Size of a database in magic units (bytes?).
ast_struct!(
    Size as size {
        /// The resource to obtain the size of.
        resource: String,
        /// The size.
        size: DataValue
    }
);

impl ToCLIQueryAST for Size {
    fn to_ast(&self) -> String {
        todo!()
    }
}