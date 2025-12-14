use terminusdb_woql_js::parse_js_woql;

fn main() {
    let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
    match parse_js_woql(query) {
        Ok(json) => println!("Success: {}", serde_json::to_string_pretty(&json).unwrap()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
