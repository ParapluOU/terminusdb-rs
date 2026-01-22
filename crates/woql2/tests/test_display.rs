use terminusdb_woql2::prelude::*;
use terminusdb_woql2::query::{Query, And};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{Value, NodeValue};
use terminusdb_schema::GraphType;

#[test]
fn test_display_simple_triple() {
    let triple = Triple {
        subject: NodeValue::Variable("Person".to_string()),
        predicate: NodeValue::Node("@schema:name".to_string()),
        object: Value::Variable("Name".to_string()),
        graph: Some(GraphType::Instance),
    };
    
    // Test Display directly on Triple
    assert_eq!(triple.to_string(), r#"triple($Person, "@schema:name", $Name)"#);
    
    // Test Display on Query enum
    let query = Query::Triple(triple);
    assert_eq!(query.to_string(), r#"triple($Person, "@schema:name", $Name)"#);
}

#[test]
fn test_display_values() {
    let var = Value::Variable("Test".to_string());
    assert_eq!(var.to_string(), "$Test");
    
    let node = NodeValue::Node("@schema:Person".to_string());
    assert_eq!(node.to_string(), r#""@schema:Person""#);
    
    let data = DataValue::Variable("Count".to_string());
    assert_eq!(data.to_string(), "$Count");
}

#[test]
fn test_display_complex_query() {
    let select = Select {
        variables: vec!["Name".to_string(), "Age".to_string()],
        query: Box::new(Query::And(And {
            and: vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".to_string()),
                    predicate: NodeValue::Node("@schema:name".to_string()),
                    object: Value::Variable("Name".to_string()),
                    graph: Some(GraphType::Instance),
                }),
                Query::Greater(Greater {
                    left: DataValue::Variable("Age".to_string()),
                    right: DataValue::Data(terminusdb_schema::XSDAnySimpleType::Float(18.0)),
                }),
            ],
        })),
    };
    
    let expected = r#"select([$Name, $Age], and(triple($Person, "@schema:name", $Name), greater($Age, 18)))"#;
    assert_eq!(select.to_string(), expected);
}

#[test]
fn test_display_format_integration() {
    let query = Query::And(And {
        and: vec![
            Query::Triple(Triple {
                subject: NodeValue::Variable("X".to_string()),
                predicate: NodeValue::Node("rdf:type".to_string()),
                object: Value::Node("@schema:Person".to_string()),
                graph: Some(GraphType::Instance),
            }),
            Query::Triple(Triple {
                subject: NodeValue::Variable("X".to_string()),
                predicate: NodeValue::Node("@schema:age".to_string()),
                object: Value::Variable("Age".to_string()),
                graph: Some(GraphType::Instance),
            }),
        ],
    });
    
    // Test with format! macro
    let formatted = format!("Query: {}", query);
    assert!(formatted.starts_with("Query: and("));
    
    // Test with println! (won't actually print during tests)
    let output = format!("{}", query);
    assert!(output.contains("triple($X"));
}

#[test]
fn test_display_arithmetic() {
    let expr = ArithmeticExpression::Plus(Plus {
        left: Box::new(ArithmeticExpression::Value(ArithmeticValue::Variable("A".to_string()))),
        right: Box::new(ArithmeticExpression::Value(ArithmeticValue::Variable("B".to_string()))),
    });
    
    assert_eq!(expr.to_string(), "plus($A, $B)");
}

#[test]
fn test_display_path_pattern() {
    let pattern = PathPattern::Predicate(PathPredicate {
        predicate: Some("@schema:knows".to_string()),
    });
    
    assert_eq!(pattern.to_string(), r#"pred("@schema:knows")"#);
    
    let star_pattern = PathPattern::Star(PathStar {
        star: Box::new(pattern),
    });
    
    assert_eq!(star_pattern.to_string(), r#"star(pred("@schema:knows"))"#);
}