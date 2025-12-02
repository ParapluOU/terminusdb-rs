use terminusdb_woql_js::parse_js_woql_to_query;

fn main() {
    let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
    match parse_js_woql_to_query(query) {
        Ok(q) => println!("Success: {:?}", q),
        Err(e) => eprintln!("Error: {}", e),
    }
}
