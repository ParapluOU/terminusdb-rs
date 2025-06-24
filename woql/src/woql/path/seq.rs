use crate::*;

/// A sequence of patterns in which each of the patterns in the list must result in objects which are subjects of the next pattern in the list.
ast_struct!(
    PathSequence as path_seq {
        /// A sequence of path patterns.
        sequence: (Vec<PathPattern>)
    }
);

impl ToCLIQueryAST for PathSequence {
    fn to_ast(&self) -> String {
        todo!()
    }
}