use terminusdb_woql2::prelude::*;
use terminusdb_woql2::query::{Query, And};
use terminusdb_woql2::triple::Triple;
use terminusdb_woql2::value::{Value, NodeValue};
use terminusdb_schema::GraphType;

fn main() {
    // Create a complex WOQL query
    let query = Query::Select(Select {
        variables: vec!["Name".to_string(), "Age".to_string()],
        query: Box::new(Query::And(And {
            and: vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".to_string()),
                    predicate: NodeValue::Node("rdf:type".to_string()),
                    object: Value::Node("@schema:Person".to_string()),
                    graph: GraphType::Instance,
                }),
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".to_string()),
                    predicate: NodeValue::Node("@schema:name".to_string()),
                    object: Value::Variable("Name".to_string()),
                    graph: GraphType::Instance,
                }),
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".to_string()),
                    predicate: NodeValue::Node("@schema:age".to_string()),
                    object: Value::Variable("Age".to_string()),
                    graph: GraphType::Instance,
                }),
                Query::Greater(Greater {
                    left: DataValue::Variable("Age".to_string()),
                    right: DataValue::Data(terminusdb_schema::XSDAnySimpleType::Float(18.0)),
                }),
            ],
        })),
    });

    // Use Display trait (which internally uses to_dsl())
    println!("Generated DSL using Display trait:");
    println!("{}", query);
    
    // You can also still use to_dsl() directly if needed
    let dsl = query.to_dsl();
    assert_eq!(query.to_string(), dsl);
    
    // Pretty print with line breaks for readability
    println!("\nFormatted DSL:");
    let formatted = query.to_string()
        .replace(", and(", ",\n  and(")
        .replace(", triple(", ",\n    triple(")
        .replace(", greater(", ",\n    greater(");
    println!("{}", formatted);
    
    // Display works on individual components too
    println!("\nIndividual components:");
    let triple = Triple {
        subject: NodeValue::Variable("Person".to_string()),
        predicate: NodeValue::Node("@schema:name".to_string()),
        object: Value::Variable("Name".to_string()),
        graph: GraphType::Instance,
    };
    println!("Triple: {}", triple);
    
    let var = Value::Variable("TestVar".to_string());
    println!("Variable: {}", var);
}