use terminusdb_woql_js::parse_js_woql;

fn main() {
    let query = r#"count("v:Count", triple("v:Commit", "rdf:type", "@schema:ValidCommit"))"#;
    println!("Parsing: {}", query);

    match parse_js_woql(query) {
        Ok(json_ld) => {
            println!("✓ Successfully parsed to JSON-LD:");
            println!("{}", serde_json::to_string_pretty(&json_ld).unwrap());

            // Now try to deserialize using from_json
            use terminusdb_woql2::prelude::FromTDBInstance;
            match terminusdb_woql2::query::Query::from_json(json_ld) {
                Ok(q) => println!("\n✓ Successfully deserialized: {:?}", q),
                Err(e) => eprintln!("\n✗ Deserialization error: {:?}", e),
            }
        },
        Err(e) => {
            eprintln!("✗ Parse error: {}", e);
            std::process::exit(1);
        }
    }
}
