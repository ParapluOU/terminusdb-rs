pub enum DefaultPredicateSpecs {}

impl PredicateSpec for DefaultPredicateSpecs {}

// trait to be implemented on enums that list the schema predicates
pub trait PredicateSpec {}

#[macro_export]
macro_rules! predicate_spec {
    ($spec_name:ident => [$($predicate_name:ident),*]) => {
        pub enum $spec_name {
            $(
                $predicate_name
            ),*
        }

        impl terminusdb_schema::PredicateSpec for $spec_name {
            //
        }
    }
}
