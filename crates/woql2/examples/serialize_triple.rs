use terminusdb_schema::ToJson;
use terminusdb_woql2::prelude::*;

fn main() {
    let query = Query::triple(
        node("v:Subject"),
        node("v:Predicate"),
        value_node("v:Object"),
    );

    let json = query.to_instance(None).to_json();
    println!("Rust Query to JSON:");
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}
