use terminusdb_woql2::prelude::*;

// Test model for property access
#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Person {
    name: String,
    age: i32,
    email: Option<String>,
    active: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Company {
    name: String,
    employees: Vec<Person>,
    founded_year: i32,
}

#[test]
fn test_field_macro_basic() {
    // Test that field! returns the correct field name
    assert_eq!(field!(Person:name), "name");
    assert_eq!(field!(Person:age), "age");
    assert_eq!(field!(Person:email), "email");
    assert_eq!(field!(Person:active), "active");

    assert_eq!(field!(Company:name), "name");
    assert_eq!(field!(Company:employees), "employees");
    assert_eq!(field!(Company:founded_year), "founded_year");
}

#[test]
fn test_field_macro_in_triple() {
    // Test using field! in triple queries
    let q1 = triple!(var!(x), field!(Person:name), var!(n));
    let q2 = triple!(var!(x), "name", var!(n));

    // Both should produce the same query structure
    match (&q1, &q2) {
        (Query::Triple(t1), Query::Triple(t2)) => {
            // The predicate should be the same
            assert_eq!(t1.predicate, t2.predicate);
        }
        _ => panic!("Expected Triple queries"),
    }
}

#[test]
fn test_field_macro_in_data_triple() {
    // Test using field! in data_triple
    let q = data_triple!(var!(p), field!(Person:age), data!(25));

    match q {
        Query::Data(data) => match &data.predicate {
            NodeValue::Node(s) => assert_eq!(s, "age"),
            _ => panic!("Expected Node predicate"),
        },
        _ => panic!("Expected Data query"),
    }
}

#[test]
fn test_field_macro_in_link() {
    // Test using field! in link
    let q = link!(var!(c), field!(Company:employees), var!(e));

    match q {
        Query::Link(link) => match &link.predicate {
            NodeValue::Node(s) => assert_eq!(s, "employees"),
            _ => panic!("Expected Node predicate"),
        },
        _ => panic!("Expected Link query"),
    }
}

#[test]
fn test_field_macro_in_complex_query() {
    // Test field! in a more complex query
    let q = and!(
        triple!(var!(p), "rdf:type", "Person"),
        triple!(var!(p), field!(Person:name), var!(name)),
        data_triple!(var!(p), field!(Person:age), var!(age)),
        optional!(triple!(var!(p), field!(Person:email), var!(email))),
        equals!(var!(active), data!(true))
    );

    // Verify the query compiles and has the expected structure
    match q {
        Query::And(_) => {
            // Success - query compiled with field! macros
        }
        _ => panic!("Expected And query"),
    }
}

#[test]
fn test_field_macro_with_select() {
    // Test field! in queries with select
    let q = select!(
        [name, age],
        and!(
            triple!(var!(p), "rdf:type", "Person"),
            triple!(var!(p), field!(Person:name), var!(name)),
            triple!(var!(p), field!(Person:age), var!(age))
        )
    );

    match q {
        Query::Select(select) => {
            assert_eq!(select.variables, vec!["name", "age"]);
        }
        _ => panic!("Expected Select query"),
    }
}

// The following test is commented out because it's meant to demonstrate
// compile-time failure when accessing non-existent fields
/*
#[test]
fn test_field_macro_compile_error() {
    // This should fail to compile
    let _ = field!(Person:nonexistent_field);
    // Error: no field `nonexistent_field` on type `Person`
}
*/

#[test]
fn test_field_macro_different_models_same_field() {
    // Test that field! works correctly when different models have the same field name
    let person_name = field!(Person:name);
    let company_name = field!(Company:name);

    // Both should return "name"
    assert_eq!(person_name, "name");
    assert_eq!(company_name, "name");

    // Use in separate queries
    let q1 = triple!(var!(p), field!(Person:name), var!(n));
    let q2 = triple!(var!(c), field!(Company:name), var!(n));

    // Both queries should work correctly
    match (&q1, &q2) {
        (Query::Triple(_), Query::Triple(_)) => {
            // Success
        }
        _ => panic!("Expected Triple queries"),
    }
}
