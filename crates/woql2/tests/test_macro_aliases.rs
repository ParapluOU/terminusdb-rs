use terminusdb_woql2::*;

#[test]
fn test_optional_aliases() {
    // Test that opt! macro works the same as optional!
    let q1 = optional!(triple!(var!(x), "email", var!(email)));
    let q2 = opt!(triple!(var!(x), "email", var!(email)));

    // Test that option! macro works the same as optional!
    let q3 = option!(triple!(var!(x), "email", var!(email)));

    // All three should be WoqlOptional queries
    assert!(matches!(q1, query::Query::WoqlOptional(_)));
    assert!(matches!(q2, query::Query::WoqlOptional(_)));
    assert!(matches!(q3, query::Query::WoqlOptional(_)));
}

#[test]
fn test_triple_alias() {
    // Test that t! macro works the same as triple!
    let q1 = triple!(var!(x), "rdf:type", "Person");
    let q2 = t!(var!(x), "rdf:type", "Person");

    // Test with graph parameter
    let q3 = triple!(
        var!(x),
        "name",
        var!(name),
        Some(terminusdb_schema::GraphType::Instance)
    );
    let q4 = t!(
        var!(x),
        "name",
        var!(name),
        Some(terminusdb_schema::GraphType::Instance)
    );

    // All should be Triple queries
    assert!(matches!(q1, query::Query::Triple(_)));
    assert!(matches!(q2, query::Query::Triple(_)));
    assert!(matches!(q3, query::Query::Triple(_)));
    assert!(matches!(q4, query::Query::Triple(_)));
}

#[test]
fn test_equals_alias() {
    // Test that equals! macro works the same as eq!
    let q1 = eq!(var!(x), data!(42));
    let q2 = equals!(var!(x), data!(42));

    let q3 = eq!(var!(name), var!(other_name));
    let q4 = equals!(var!(name), var!(other_name));

    // All should be Equals queries
    assert!(matches!(q1, query::Query::Equals(_)));
    assert!(matches!(q2, query::Query::Equals(_)));
    assert!(matches!(q3, query::Query::Equals(_)));
    assert!(matches!(q4, query::Query::Equals(_)));
}

#[test]
fn test_combined_usage() {
    // Test using the aliases together
    let query = and!(
        t!(var!(x), "rdf:type", "Person"),
        t!(var!(x), "name", var!(name)),
        equals!(var!(name), data!("John")),
        opt!(t!(var!(x), "email", var!(email))),
        option!(t!(var!(x), "phone", var!(phone)))
    );

    assert!(matches!(query, query::Query::And(_)));
}
