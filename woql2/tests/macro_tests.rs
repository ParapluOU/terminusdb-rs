//! Tests for WOQL construction macros

use terminusdb_woql2::prelude::*;

#[test]
fn test_value_macros() {
    // Test var! macro
    let x_var = var!(x);
    assert!(matches!(x_var, Value::Variable(ref s) if s == "x"));
    
    let named_var = var!("my_variable");
    assert!(matches!(named_var, Value::Variable(ref s) if s == "my_variable"));
    
    // Test node! macro
    let person_node = node!("Person");
    assert!(matches!(person_node, Value::Node(ref s) if s == "Person"));
    
    // Test data! macro
    let int_data = data!(42);
    assert!(matches!(int_data, Value::Data(_)));
    
    let string_data = data!("hello");
    assert!(matches!(string_data, Value::Data(_)));
    
    let float_data = data!(3.14);
    assert!(matches!(float_data, Value::Data(_)));
    
    let bool_data = data!(true);
    assert!(matches!(bool_data, Value::Data(_)));
    
    // Test list! macro
    let items = list![data!(1), data!(2), data!(3)];
    assert!(matches!(items, Value::List(ref v) if v.len() == 3));
    
    let mixed_list = list![var!(x), node!("item"), data!("text")];
    assert!(matches!(mixed_list, Value::List(ref v) if v.len() == 3));
}

#[test]
fn test_query_macros() {
    // Test triple! macro
    let t = triple!(var!(x), "rdf:type", "Person");
    assert!(matches!(t, Query::Triple(_)));
    
    // Test and! macro
    let and_query = and!(
        triple!(var!(x), "rdf:type", "Person"),
        triple!(var!(x), "name", var!(name))
    );
    assert!(matches!(and_query, Query::And(ref a) if a.and.len() == 2));
    
    // Test or! macro
    let or_query = or!(
        triple!(var!(x), "rdf:type", "Person"),
        triple!(var!(x), "rdf:type", "Organization")
    );
    assert!(matches!(or_query, Query::Or(ref o) if o.or.len() == 2));
    
    // Test not! macro
    let not_query = not!(triple!(var!(x), "archived", data!(true)));
    assert!(matches!(not_query, Query::Not(_)));
    
    // Test select! macro
    let select_query = select!([x, name], and!(
        triple!(var!(x), "rdf:type", "Person"),
        triple!(var!(x), "name", var!(name))
    ));
    assert!(matches!(select_query, Query::Select(ref s) if s.variables.len() == 2));
}

#[test]
fn test_comparison_macros() {
    // Test eq! macro
    let eq_query = eq!(var!(x), data!(42));
    assert!(matches!(eq_query, Query::Equals(_)));
    
    // Test greater! macro
    let gt_query = greater!(var!(age), data!(18));
    assert!(matches!(gt_query, Query::Greater(_)));
    
    // Test less! macro
    let lt_query = less!(var!(age), data!(65));
    assert!(matches!(lt_query, Query::Less(_)));
}

#[test]
fn test_document_macros() {
    // Test read_doc! macro
    let read_query = read_doc!(node!("doc:123"), var!(doc));
    assert!(matches!(read_query, Query::ReadDocument(_)));
    
    // Test insert_doc! macro
    let insert_query = insert_doc!(var!(doc));
    assert!(matches!(insert_query, Query::InsertDocument(ref i) if i.identifier.is_none()));
    
    let insert_with_id = insert_doc!(var!(doc), node!("doc:123"));
    assert!(matches!(insert_with_id, Query::InsertDocument(ref i) if i.identifier.is_some()));
    
    // Test update_doc! macro
    let update_query = update_doc!(var!(doc));
    assert!(matches!(update_query, Query::UpdateDocument(ref u) if u.identifier.is_none()));
    
    let update_with_id = update_doc!(var!(doc), node!("doc:123"));
    assert!(matches!(update_with_id, Query::UpdateDocument(ref u) if u.identifier.is_some()));
    
    // Test delete_doc! macro
    let delete_query = delete_doc!(node!("doc:123"));
    assert!(matches!(delete_query, Query::DeleteDocument(_)));
}

#[test]
fn test_control_flow_macros() {
    // Test limit! macro
    let limit_query = limit!(10, triple!(var!(x), "rdf:type", "Person"));
    assert!(matches!(limit_query, Query::Limit(ref l) if l.limit == 10));
    
    // Test if_then_else! macro
    let if_query = if_then_else!(
        greater!(var!(age), data!(18)),
        triple!(var!(x), "status", "adult"),
        triple!(var!(x), "status", "minor")
    );
    assert!(matches!(if_query, Query::If(_)));
}

#[test]
fn test_complex_query_example() {
    // Complex query example using macros
    let query = select!([person, name, age], and!(
        triple!(var!(person), "rdf:type", "Person"),
        triple!(var!(person), "name", var!(name)),
        triple!(var!(person), "age", var!(age)),
        greater!(var!(age), data!(21)),
        not!(triple!(var!(person), "archived", data!(true)))
    ));
    
    // Verify the query structure
    match query {
        Query::Select(select) => {
            assert_eq!(select.variables.len(), 3);
            assert_eq!(select.variables[0], "person");
            assert_eq!(select.variables[1], "name");
            assert_eq!(select.variables[2], "age");
            
            match &*select.query {
                Query::And(and) => {
                    assert_eq!(and.and.len(), 5);
                }
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_dsl_rendering() {
    // Test that our macros produce queries that can be rendered to DSL
    let query = and!(
        triple!(var!(x), "rdf:type", "Person"),
        triple!(var!(x), "name", var!(name)),
        greater!(var!(age), data!(18))
    );
    
    // The query should have a valid DSL representation
    let dsl = query.to_dsl();
    assert!(dsl.contains("and("));
    assert!(dsl.contains("triple("));
    assert!(dsl.contains("$x"));
    assert!(dsl.contains("greater("));
}

#[test]
fn test_shortcut_macros() {
    // Test type! macro
    let type_query = type_!(var!(x), "Person");
    match type_query {
        Query::Triple(t) => {
            assert!(matches!(t.predicate, NodeValue::Node(ref s) if s == "rdf:type"));
            assert!(matches!(t.object, Value::Node(ref s) if s == "Person"));
        }
        _ => panic!("Expected Triple query"),
    }
    
    // Test isa! macro
    let isa_query = isa!(var!(x), "Person");
    assert!(matches!(isa_query, Query::IsA(_)));
    
    // Test optional! macro
    let opt_query = optional!(triple!(var!(x), "email", var!(email)));
    assert!(matches!(opt_query, Query::WoqlOptional(_)));
    
    // Test distinct_vars! macro
    let distinct_query = distinct_vars!([x, y], triple!(var!(x), "knows", var!(y)));
    match distinct_query {
        Query::Distinct(d) => {
            assert_eq!(d.variables.len(), 2);
            assert_eq!(d.variables[0], "x");
            assert_eq!(d.variables[1], "y");
        }
        _ => panic!("Expected Distinct query"),
    }
    
    // Test count_into! macro
    let count_query = count_into!(triple!(var!(x), "rdf:type", "Person"), var!(count));
    assert!(matches!(count_query, Query::Count(_)));
    
    // Test cast! macro
    let cast_query = cast!(var!(x), "xsd:integer", var!(int_x));
    assert!(matches!(cast_query, Query::Typecast(_)));
    
    // Test immediately! macro
    let imm_query = immediately!(insert_doc!(var!(doc)));
    assert!(matches!(imm_query, Query::Immediately(_)));
    
    // Test link! macro
    let link_query = link!(var!(person), "friend", var!(friend));
    assert!(matches!(link_query, Query::Link(_)));
    
    // Test data_triple! macro
    let data_query = data_triple!(var!(person), "age", data!(25));
    assert!(matches!(data_query, Query::Data(_)));
    
    // Test regex! macro
    let regex_query = regex!("^[A-Z]", var!(name));
    assert!(matches!(regex_query, Query::Regexp(_)));
    
    // Test trim! macro
    let trim_query = trim!(var!(input), var!(output));
    assert!(matches!(trim_query, Query::Trim(_)));
    
    // Test true_! macro
    let true_query = true_!();
    assert!(matches!(true_query, Query::True(_)));
}

#[test]
fn test_complex_query_with_shortcuts() {
    // Complex query using shortcut macros
    let query = select!([person, name, friends], and!(
        type_!(var!(person), "Person"),
        triple!(var!(person), "name", var!(name)),
        optional!(triple!(var!(person), "email", var!(email))),
        count_into!(
            link!(var!(person), "friend", var!(friend)),
            var!(friends)
        )
    ));
    
    // Verify structure
    match query {
        Query::Select(s) => {
            assert_eq!(s.variables.len(), 3);
            match &*s.query {
                Query::And(a) => assert_eq!(a.and.len(), 4),
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_compare_macro() {
    // Test basic comparisons
    let gt_query = compare!((var!(age)) > (data!(18)));
    assert!(matches!(gt_query, Query::Greater(_)));
    
    let lt_query = compare!((var!(age)) < (data!(65)));
    assert!(matches!(lt_query, Query::Less(_)));
    
    let eq_query = compare!((var!(x)) == (var!(y)));
    assert!(matches!(eq_query, Query::Equals(_)));
    
    // Test compound comparisons
    let gte_query = compare!((var!(age)) >= (data!(18)));
    match gte_query {
        Query::Or(or) => {
            assert_eq!(or.or.len(), 2);
            assert!(matches!(or.or[0], Query::Greater(_)));
            assert!(matches!(or.or[1], Query::Equals(_)));
        }
        _ => panic!("Expected Or query for >="),
    }
    
    let lte_query = compare!((var!(age)) <= (data!(65)));
    match lte_query {
        Query::Or(or) => {
            assert_eq!(or.or.len(), 2);
            assert!(matches!(or.or[0], Query::Less(_)));
            assert!(matches!(or.or[1], Query::Equals(_)));
        }
        _ => panic!("Expected Or query for <="),
    }
    
    let ne_query = compare!((var!(x)) != (var!(y)));
    match ne_query {
        Query::Not(not) => {
            assert!(matches!(&*not.query, Query::Equals(_)));
        }
        _ => panic!("Expected Not query for !="),
    }
}

#[test]
fn test_compare_macro_in_complex_query() {
    // Use compare! macro in a realistic query
    let query = select!([person, name, age], and!(
        type_!(var!(person), "Person"),
        triple!(var!(person), "name", var!(name)),
        triple!(var!(person), "age", var!(age)),
        compare!((var!(age)) >= (data!(21))),
        compare!((var!(age)) < (data!(65)))
    ));
    
    // Verify the query structure
    match &query {
        Query::Select(s) => {
            assert_eq!(s.variables.len(), 3);
            match &*s.query {
                Query::And(a) => {
                    assert_eq!(a.and.len(), 5);
                    // Check that the >= comparison became an Or
                    match &a.and[3] {
                        Query::Or(or) => assert_eq!(or.or.len(), 2),
                        _ => panic!("Expected Or for >= comparison"),
                    }
                    // Check that the < comparison is still Less
                    assert!(matches!(&a.and[4], Query::Less(_)));
                }
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }
    
    // Test DSL rendering
    let dsl = query.to_dsl();
    assert!(dsl.contains("or(greater("));
    assert!(dsl.contains("less("));
}