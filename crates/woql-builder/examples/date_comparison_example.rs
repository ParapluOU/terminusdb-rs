use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::value::{datetime_literal, date_literal, time_literal};

fn main() {
    println!("=== Date Comparison WOQL Examples ===\n");
    
    // Example 1: Find all events after a certain date
    println!("1. Query for events after 2024-01-01:");
    let (event_id, event_date) = vars!("EventID", "EventDate");
    
    let query1 = WoqlBuilder::new()
        .triple(event_id.clone(), "event_date", event_date.clone())
        .greater(event_date, date_literal("2024-01-01"))
        .select(vec![event_id])
        .finalize();
    
    println!("Query structure: {:#?}\n", query1);
    
    // Example 2: Find all meetings before a certain datetime
    println!("2. Query for meetings before 2025-06-01T14:00:00Z:");
    let (meeting_id, meeting_time) = vars!("MeetingID", "MeetingTime");
    
    let query2 = WoqlBuilder::new()
        .triple(meeting_id.clone(), "scheduled_time", meeting_time.clone())
        .less(meeting_time, datetime_literal("2025-06-01T14:00:00Z"))
        .select(vec![meeting_id])
        .finalize();
    
    println!("Query structure: {:#?}\n", query2);
    
    // Example 3: Find events on a specific date
    println!("3. Query for events on exactly 2025-01-01:");
    let (event_id, event_date) = vars!("EventID", "EventDate");
    
    let query3 = WoqlBuilder::new()
        .triple(event_id.clone(), "event_date", event_date.clone())
        .eq(event_date, date_literal("2025-01-01"))
        .select(vec![event_id])
        .finalize();
    
    println!("Query structure: {:#?}\n", query3);
    
    // Example 4: Complex query with multiple date conditions
    println!("4. Complex query: Events in Q1 2025");
    let (event_id, event_name, event_date) = vars!("EventID", "EventName", "EventDate");
    
    let query4 = WoqlBuilder::new()
        .triple(event_id.clone(), "name", event_name.clone())
        .triple(event_id.clone(), "event_date", event_date.clone())
        .greater(event_date.clone(), date_literal("2024-12-31"))
        .less(event_date, date_literal("2025-04-01"))
        .select(vec![event_id, event_name])
        .finalize();
    
    println!("Query structure: {:#?}", query4);
}