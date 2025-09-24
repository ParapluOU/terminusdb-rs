use terminusdb_woql2::*;
use terminusdb_woql2::query::{NamedParametricQuery, Query};
use serde_json::json;

#[test]
fn test_npq_json_ld_format() {
    // Test creating a simple NamedParametricQuery
    let npq = NamedParametricQuery {
        name: "test".to_string(),
        parameters: vec![],
        query: Query::True(query::True {}),
    };
    
    println!("NPQ struct: {:#?}", npq);
    
    // Test JSON-LD representations that we tried with TerminusDB
    let json_attempts = vec![
        // Attempt 1: Direct NPQ
        json!({
            "@type": "NamedParametricQuery",
            "name": "test",
            "parameters": [],
            "query": {
                "@type": "True"
            }
        }),
        
        // Attempt 2: As InsertDocument
        json!({
            "@type": "InsertDocument",
            "document": {
                "@type": "NamedParametricQuery",
                "@id": "NamedParametricQuery/test",
                "name": "test",
                "parameters": [],
                "query": {
                    "@type": "True"
                }
            }
        }),
        
        // Attempt 3: With woql prefix
        json!({
            "@type": "InsertDocument",
            "document": {
                "@type": "woql:NamedParametricQuery",
                "name": "test",
                "parameters": [],
                "query": {
                    "@type": "woql:True"
                }
            }
        }),
    ];
    
    for (i, attempt) in json_attempts.iter().enumerate() {
        println!("\nAttempt {}: {}", i + 1, serde_json::to_string_pretty(attempt).unwrap());
    }
    
    // Test Call
    let call = call!("test");
    match call {
        Query::Call(ref c) => {
            println!("\nCall created successfully!");
            println!("Call name: {}", c.name);
            println!("Call arguments: {:?}", c.arguments);
        }
        _ => panic!("Expected Call variant"),
    }
    
    // Call JSON representation
    let call_json = json!({
        "@type": "Call",
        "name": "test",
        "arguments": []
    });
    
    println!("\nCall JSON: {}", serde_json::to_string_pretty(&call_json).unwrap());
}