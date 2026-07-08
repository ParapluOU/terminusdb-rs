use terminusdb_schema::ToJson;
use terminusdb_woql2::prelude::*;

fn main() {
    let query = Query::Triple(Triple {
        subject: NodeValue::Variable("Subject".to_string()),
        predicate: NodeValue::Variable("Predicate".to_string()),
        object: Value::Variable("Object".to_string()),
        graph: None,
    });

    let json = query.to_instance(None).to_json();
    println!("Rust Query to JSON:");
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}
