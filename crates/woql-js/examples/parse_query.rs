use terminusdb_woql_js::parse_js_woql;

fn main() -> anyhow::Result<()> {
    // Example 1: Simple triple query
    let simple_query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;

    println!("Parsing simple query:");
    println!("  Input: {}", simple_query);

    let json_ld = parse_js_woql(simple_query)?;
    println!("  Output JSON-LD:");
    println!("{}\n", serde_json::to_string_pretty(&json_ld)?);

    // Example 2: Complex select query
    let complex_query = r#"
        select(
            "Name", "Age",
            and(
                triple("v:Person", "rdf:type", "@schema:Person"),
                triple("v:Person", "@schema:name", "v:Name"),
                triple("v:Person", "@schema:age", "v:Age"),
                greater("v:Age", 18)
            )
        )
    "#;

    println!("Parsing complex query:");
    println!("  Input: {}", complex_query.trim());

    let json_ld = parse_js_woql(complex_query)?;
    println!("  Output JSON-LD:");
    println!("{}", serde_json::to_string_pretty(&json_ld)?);

    Ok(())
}
