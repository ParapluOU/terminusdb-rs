use crate::*;

#[macro_export]
macro_rules! pred {
    ($name:ident) => {
        pred(stringify!($name))
    }
}

newtype!({
    name: PredicateURI,
    type: String,
    schemaclass: STRING
});

// todo: type check against struct fields?
// generate per-struct-field predicate objects?
pub fn pred(prop: impl AsRef<str>) -> PredicateURI {
    PredicateURI(prop.as_ref().to_string())
}
