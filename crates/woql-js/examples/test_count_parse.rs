use terminusdb_woql_js::parse_js_woql_to_query;

fn main() {
    let query = r#"count("v:Count", triple("v:Commit", "rdf:type", "@schema:ValidCommit"))"#;
    println!("Parsing: {}", query);

    match parse_js_woql_to_query(query) {
        Ok(q) => {
            println!("✓ Successfully parsed JavaScript WOQL syntax!");
            println!("Query: {:?}", q);
        }
        Err(e) => {
            eprintln!("✗ Error: {}", e);
            std::process::exit(1);
        }
    }
}
