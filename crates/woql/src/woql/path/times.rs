use crate::*;

/// The path pattern specified by 'times' may hold 'from' to 'to' times in succession.
ast_struct!(
    PathTimes as path_times {
        /// A path pattern.
        times: PathPattern,
        /// The number of times to start the repetition of the pattern
        from: usize,
        /// The number of times after which to end the repeition of the pattern.
        to: usize
    }
);

impl ToCLIQueryAST for PathTimes {
    fn to_ast(&self) -> String {
        todo!()
    }
}