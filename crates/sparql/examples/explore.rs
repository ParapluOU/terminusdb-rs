//! Offline inspection harness: compile a handful of SPARQL queries and print the
//! WOQL they lower to (DSL + JSON-LD). No database, no `client` feature — the
//! fast loop for developing the compiler.
//!
//!     cargo run -p terminusdb-sparql --example explore

fn main() {
    let queries = [
        r#"PREFIX s: <http://terminusdb.com/schema#>
           SELECT ?name WHERE { ?p a s:Person . ?p s:name ?name }"#,
        r#"PREFIX s: <http://terminusdb.com/schema#>
           SELECT DISTINCT ?name ?age WHERE {
             ?p a s:Person . ?p s:name ?name . ?p s:age ?age .
             FILTER(?age > 26 && ?age < 40)
           } ORDER BY DESC(?age) LIMIT 5 OFFSET 1"#,
        r#"PREFIX s: <http://terminusdb.com/schema#>
           SELECT ?name ?nick WHERE {
             ?p a s:Person . ?p s:name ?name .
             OPTIONAL { ?p s:nickname ?nick }
           }"#,
        r#"PREFIX s: <http://terminusdb.com/schema#>
           SELECT ?name WHERE {
             { ?p s:name ?name . FILTER(?name = "Jane") }
             UNION
             { ?p s:name ?name . FILTER(?name = "John") }
           }"#,
        r#"PREFIX s: <http://terminusdb.com/schema#>
           SELECT * WHERE { ?p a s:Person . ?p s:name ?name }"#,
    ];
    for q in queries {
        println!("\n================================================================");
        match terminusdb_sparql::explain(q) {
            Ok(ex) => println!("{}", ex.report()),
            Err(e) => println!("ERROR: {e}"),
        }
    }
}
