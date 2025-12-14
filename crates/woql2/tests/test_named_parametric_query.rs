use terminusdb_woql2::*;

#[test]
fn test_create_named_parametric_query() {
    // Create a named parametric query that finds all instances of a given type
    let query = query::NamedParametricQuery {
        name: "find_by_type".to_string(),
        parameters: vec!["type".to_string()],
        query: triple!(var!(x), "rdf:type", var!(type)),
    };

    assert_eq!(query.name, "find_by_type");
    assert_eq!(query.parameters, vec!["type"]);
}

#[test] 
fn test_create_named_parametric_query_multiple_params() {
    // Create a query that finds relationships between two entities
    let query = named_parametric_query!(
        "find_relationship", 
        ["subject", "predicate", "object"],
        triple!(var!(subject), var!(predicate), var!(object))
    );

    assert_eq!(query.name, "find_relationship");
    assert_eq!(query.parameters, vec!["subject", "predicate", "object"]);
}

#[test]
fn test_create_named_parametric_query_complex() {
    // Create a more complex query with AND
    let query = named_parametric_query!(
        "find_person_by_name_and_age",
        ["name", "age"],
        and!(
            triple!(var!(person), "rdf:type", "Person"),
            triple!(var!(person), "name", var!(name)),
            triple!(var!(person), "age", var!(age))
        )
    );

    assert_eq!(query.name, "find_person_by_name_and_age");
    assert_eq!(query.parameters, vec!["name", "age"]);
}

// This test won't compile because Call is not in the Query enum
// #[test]
// fn test_call_named_query() {
//     let call = call!("find_by_type", ["Person"]);
//     // This would fail to compile because Call cannot be used as a Query
// }