use terminusdb_woql_js::parse_js_woql;

fn main() {
    let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
    match parse_js_woql(query) {
        Ok(json) => {
            println!("JSON-LD produced:");
            println!("{}", serde_json::to_string_pretty(&json).unwrap());

            // Now try to deserialize
            match serde_json::from_value::<terminusdb_woql2::query::Query>(json.clone()) {
                Ok(q) => println!("\nSuccessfully deserialized: {:?}", q),
                Err(e) => eprintln!("\nDeserialization error: {}", e),
            }
        }
        Err(e) => eprintln!("Parse error: {}", e),
    }
}
