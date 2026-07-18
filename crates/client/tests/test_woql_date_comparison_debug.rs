#![recursion_limit = "256"]

#![cfg(not(target_arch = "wasm32"))]

use terminusdb_woql2::prelude::*;

fn date_lit(s: &str) -> terminusdb_woql2::value::Value {
    terminusdb_woql2::value::Value::Data(terminusdb_schema::XSDAnySimpleType::Date(
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .expect("Invalid date format, expected YYYY-MM-DD"),
    ))
}

fn datetime_lit(s: &str) -> terminusdb_woql2::value::Value {
    terminusdb_woql2::value::Value::Data(terminusdb_schema::XSDAnySimpleType::DateTime(
        chrono::DateTime::parse_from_rfc3339(s)
            .expect("Invalid datetime format, expected RFC3339")
            .with_timezone(&chrono::Utc),
    ))
}

#[test]
fn test_date_comparison_query_generation() {
    println!("\n=== Testing Date Comparison Query Generation ===\n");

    // Test 1: Date comparison with greater
    let (event_id, event_date_var) = (var!(EventID), var!(EventDate));
    let date_cutoff = date_lit("2024-01-01");

    let query1 = select!(
        [event_id.clone()],
        and!(
            triple!(event_id.clone(), "event_date", event_date_var.clone()),
            greater!(event_date_var.clone(), date_cutoff)
        )
    );

    let json1 = query1.to_json();
    println!("Query 1 (Date greater than 2024-01-01):");
    println!("{}", serde_json::to_string_pretty(&json1).unwrap());

    // Verify the date literal is properly formatted in the query
    let json_str = serde_json::to_string(&json1).unwrap();
    assert!(json_str.contains(r#""@type":"xsd:date"#) || json_str.contains("2024-01-01"));

    // Test 2: DateTime comparison with less
    let (event_id2, event_time_var) = (var!(EventID2), var!(EventTime));
    let datetime_cutoff = datetime_lit("2025-06-01T00:00:00Z");

    let query2 = select!(
        [event_id2.clone()],
        and!(
            triple!(event_id2.clone(), "event_time", event_time_var.clone()),
            less!(event_time_var.clone(), datetime_cutoff)
        )
    );

    let json2 = query2.to_json();
    println!("\nQuery 2 (DateTime less than 2025-06-01T00:00:00Z):");
    println!("{}", serde_json::to_string_pretty(&json2).unwrap());

    // Verify the datetime literal is properly formatted
    let json_str2 = serde_json::to_string(&json2).unwrap();
    assert!(
        json_str2.contains(r#""@type":"xsd:dateTime"#) || json_str2.contains("2025-06-01T00:00:00")
    );

    // Test 3: Date equality
    let (event_id3, event_date_var3) = (var!(EventID3), var!(EventDate3));
    let exact_date = date_lit("2025-01-01");

    let query3 = select!(
        [event_id3.clone()],
        and!(
            triple!(event_id3.clone(), "event_date", event_date_var3.clone()),
            eq!(event_date_var3.clone(), exact_date)
        )
    );

    let json3 = query3.to_json();
    println!("\nQuery 3 (Date equals 2025-01-01):");
    println!("{}", serde_json::to_string_pretty(&json3).unwrap());

    println!("\n✅ All date comparison queries generated successfully!");
}
