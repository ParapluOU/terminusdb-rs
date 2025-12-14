use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::value::{datetime_literal, date_literal, time_literal};
use terminusdb_woql2::prelude::{Query as Woql2Query, DataValue};
use terminusdb_schema::XSDAnySimpleType;

#[test]
fn test_date_comparison_queries() {
    println!("\n=== Testing Date Comparison Query Generation ===\n");
    
    // Test 1: Date comparison with greater
    let (event_id, event_date_var) = vars!("EventID", "EventDate");
    let date_cutoff = date_literal("2024-01-01");
    
    let query1 = WoqlBuilder::new()
        .triple(event_id.clone(), "event_date", event_date_var.clone())
        .greater(event_date_var.clone(), date_cutoff)
        .finalize();
    
    // Verify the structure
    match &query1 {
        Woql2Query::And(and_q) => {
            assert_eq!(and_q.and.len(), 2);
            
            // Check the greater comparison
            match &and_q.and[1] {
                Woql2Query::Greater(greater_q) => {
                    // Check that we have a date literal
                    match &greater_q.right {
                        DataValue::Data(XSDAnySimpleType::Date(date)) => {
                            println!("✓ Date literal parsed correctly: {}", date);
                            assert_eq!(date.to_string(), "2024-01-01");
                        }
                        _ => panic!("Expected Date literal in greater comparison"),
                    }
                }
                _ => panic!("Expected Greater query"),
            }
        }
        _ => panic!("Expected And query"),
    }
    
    // Test 2: DateTime comparison with less
    let (event_id2, event_time_var) = vars!("EventID2", "EventTime");
    let datetime_cutoff = datetime_literal("2025-06-01T00:00:00Z");
    
    let query2 = WoqlBuilder::new()
        .triple(event_id2.clone(), "event_time", event_time_var.clone())
        .less(event_time_var.clone(), datetime_cutoff)
        .finalize();
    
    // Verify the structure
    match &query2 {
        Woql2Query::And(and_q) => {
            match &and_q.and[1] {
                Woql2Query::Less(less_q) => {
                    // Check that we have a datetime literal
                    match &less_q.right {
                        DataValue::Data(XSDAnySimpleType::DateTime(dt)) => {
                            println!("✓ DateTime literal parsed correctly: {}", dt);
                            assert!(dt.to_rfc3339().contains("2025-06-01"));
                        }
                        _ => panic!("Expected DateTime literal in less comparison"),
                    }
                }
                _ => panic!("Expected Less query"),
            }
        }
        _ => panic!("Expected And query"),
    }
    
    // Test 3: Time comparison with equals
    let (meeting_time_var, target_time) = (vars!("MeetingTime"), time_literal("14:30:00"));
    
    let query3 = WoqlBuilder::new()
        .triple("meeting123", "start_time", meeting_time_var.clone())
        .eq(meeting_time_var.clone(), target_time)
        .finalize();
    
    // Verify the structure
    match &query3 {
        Woql2Query::And(and_q) => {
            match &and_q.and[1] {
                Woql2Query::Equals(eq_q) => {
                    // Check that we have a time literal
                    match &eq_q.right {
                        terminusdb_woql2::value::Value::Data(XSDAnySimpleType::Time(time)) => {
                            println!("✓ Time literal parsed correctly: {}", time);
                            assert_eq!(time.to_string(), "14:30:00");
                        }
                        _ => panic!("Expected Time literal in equals comparison"),
                    }
                }
                _ => panic!("Expected Equals query"),
            }
        }
        _ => panic!("Expected And query"),
    }
    
    println!("\n✅ All date/time comparison queries work correctly!");
    println!("   - Date literals are parsed and stored as chrono::NaiveDate");
    println!("   - DateTime literals are parsed and stored as chrono::DateTime<Utc>");
    println!("   - Time literals are parsed and stored as chrono::NaiveTime");
    println!("   - All comparison operators (greater, less, eq) work with temporal types");
}