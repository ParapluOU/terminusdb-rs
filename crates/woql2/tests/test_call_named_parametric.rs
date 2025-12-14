use terminusdb_woql2::*;
use terminusdb_woql2::query::{Query, Call};

#[test]
fn test_call_macro_compilation() {
    // Test that the call! macro compiles correctly
    let call1 = call!("find_persons");
    let call2 = call!("find_by_type", [node!("@schema:Person")]);
    let call3 = call!("complex_query", [var!(x), data!(42), node!("test")]);
    
    // Verify they produce Call variants
    match call1 {
        Query::Call(ref c) => {
            assert_eq!(c.name, "find_persons");
            assert_eq!(c.arguments.len(), 0);
        }
        _ => panic!("Expected Call variant"),
    }
    
    match call2 {
        Query::Call(ref c) => {
            assert_eq!(c.name, "find_by_type");
            assert_eq!(c.arguments.len(), 1);
        }
        _ => panic!("Expected Call variant"),
    }
    
    match call3 {
        Query::Call(ref c) => {
            assert_eq!(c.name, "complex_query");
            assert_eq!(c.arguments.len(), 3);
        }
        _ => panic!("Expected Call variant"),
    }
}

#[test]
fn test_named_parametric_query_structure() {
    let npq = named_parametric_query!(
        "find_person_by_name_and_age",
        ["name", "age"],
        and!(
            triple!(var!(person), "rdf:type", "@schema:Person"),
            triple!(var!(person), "@schema:name", var!(name)),
            triple!(var!(person), "@schema:age", var!(age))
        )
    );
    
    assert_eq!(npq.name, "find_person_by_name_and_age");
    assert_eq!(npq.parameters, vec!["name", "age"]);
    
    // Test that the query is properly constructed
    match npq.query {
        Query::And(ref and) => {
            assert_eq!(and.and.len(), 3);
        }
        _ => panic!("Expected And query"),
    }
}

#[test]
fn test_call_with_different_argument_types() {
    // Test with variables
    let call_vars = call!("test", [var!(x), var!(y)]);
    match call_vars {
        Query::Call(ref c) => {
            assert_eq!(c.arguments.len(), 2);
            match &c.arguments[0] {
                value::Value::Variable(v) => assert_eq!(v, "x"),
                _ => panic!("Expected variable"),
            }
        }
        _ => panic!("Expected Call"),
    }
    
    // Test with data values
    let call_data = call!("test", [data!(42), data!("hello"), data!(true)]);
    match call_data {
        Query::Call(ref c) => {
            assert_eq!(c.arguments.len(), 3);
        }
        _ => panic!("Expected Call"),
    }
    
    // Test with nodes
    let call_nodes = call!("test", [node!("@schema:Person"), node!("doc:123")]);
    match call_nodes {
        Query::Call(ref c) => {
            assert_eq!(c.arguments.len(), 2);
            match &c.arguments[0] {
                value::Value::Node(n) => assert_eq!(n, "@schema:Person"),
                _ => panic!("Expected node"),
            }
        }
        _ => panic!("Expected Call"),
    }
}

#[test]
fn test_call_in_complex_query() {
    // Test that Call can be used as part of larger queries
    let complex = and!(
        call!("init_data"),
        select!(
            [x, name],
            and!(
                call!("find_by_type", [node!("@schema:Person")]),
                triple!(var!(x), "@schema:name", var!(name))
            )
        )
    );
    
    match complex {
        Query::And(ref and) => {
            assert_eq!(and.and.len(), 2);
            
            // First should be a Call
            match &and.and[0] {
                Query::Call(ref c) => assert_eq!(c.name, "init_data"),
                _ => panic!("Expected Call as first query"),
            }
            
            // Second should be Select
            match &and.and[1] {
                Query::Select(_) => {},
                _ => panic!("Expected Select as second query"),
            }
        }
        _ => panic!("Expected And query"),
    }
}