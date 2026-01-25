use terminusdb_woql2::query::Query;
use terminusdb_woql2::*;

#[test]
fn test_select_with_identifiers() {
    // This is the recommended way
    let q = select!(
        [x, y],
        and!(
            triple!(var!(x), "rdf:type", "Person"),
            triple!(var!(x), "name", var!(y))
        )
    );

    match q {
        Query::Select(s) => {
            assert_eq!(s.variables, vec!["x", "y"]);
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_select_with_string_literals() {
    // Using string literals directly
    let q = select!(
        ["x", "y"],
        and!(
            triple!(var!(x), "rdf:type", "Person"),
            triple!(var!(x), "name", var!(y))
        )
    );

    match q {
        Query::Select(s) => {
            assert_eq!(s.variables, vec!["x", "y"]);
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_select_with_var_macro() {
    // This now works correctly - var!(x) returns just "x" not "$x"
    let q = select!(
        [var!(x), var!(y)],
        and!(
            triple!(var!(x), "rdf:type", "Person"),
            triple!(var!(x), "name", var!(y))
        )
    );

    match q {
        Query::Select(s) => {
            assert_eq!(s.variables, vec!["x", "y"]);
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
#[should_panic(expected = "Only Value::Variable can be used as a select argument")]
fn test_select_with_non_variable_value() {
    // This should panic because we're trying to use a non-variable Value
    let node_value = node!("Person");
    let _q = select!([node_value.clone()], triple!(var!(x), "rdf:type", "Person"));
}

#[test]
fn test_mixed_select_args() {
    // Mix of different argument types
    let var_x = var!(x);
    let q = select!(
        [var_x, "y"],
        and!(
            triple!(var!(x), "rdf:type", "Person"),
            triple!(var!(x), "name", var!(y)),
            triple!(var!(x), "age", var!(z))
        )
    );

    match q {
        Query::Select(s) => {
            assert_eq!(s.variables, vec!["x", "y"]);
        }
        _ => panic!("Expected Select query"),
    }
}
