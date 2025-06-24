use crate::*;

ast_struct!(
    PathPattern {
        predicate(Box<PathPredicate>),
        inversePredicate(Box<InversePathPredicate>),
        sequence(Box<PathSequence>),
        or(Box<PathOr>),
        plus(Box<PathPlus>),
        star(Box<PathStar>),
        times(Box<PathTimes>)
    }
);

impl ToCLIQueryAST for PathPattern {
    fn to_ast(&self) -> String {
        todo!()
    }
}