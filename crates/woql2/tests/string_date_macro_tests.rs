//! Tests for string operation and date/time macros

use chrono::Datelike;
use terminusdb_woql2::prelude::*;

#[test]
fn test_string_operation_macros() {
    // Test starts_with!
    let starts_query = starts_with!(var!(name), "Dr.");
    match starts_query {
        Query::Regexp(r) => {
            // Check that the pattern starts with ^
            match &r.pattern {
                DataValue::Data(terminusdb_schema::XSDAnySimpleType::String(s)) => {
                    assert!(s.starts_with("^Dr."));
                    assert!(s.ends_with(".*"));
                }
                _ => panic!("Expected string pattern"),
            }
        }
        _ => panic!("Expected Regexp query"),
    }

    // Test ends_with!
    let ends_query = ends_with!(var!(email), "@example.com");
    match ends_query {
        Query::Regexp(r) => match &r.pattern {
            DataValue::Data(terminusdb_schema::XSDAnySimpleType::String(s)) => {
                assert!(s.starts_with(".*"));
                assert!(s.ends_with("@example.com$"));
            }
            _ => panic!("Expected string pattern"),
        },
        _ => panic!("Expected Regexp query"),
    }

    // Test contains!
    let contains_query = contains!(var!(description), "important");
    match contains_query {
        Query::Regexp(r) => match &r.pattern {
            DataValue::Data(terminusdb_schema::XSDAnySimpleType::String(s)) => {
                assert_eq!(s, ".*important.*");
            }
            _ => panic!("Expected string pattern"),
        },
        _ => panic!("Expected Regexp query"),
    }
}

#[test]
fn test_today_macro() {
    // Test today! macro
    let today_value = today!();
    match today_value {
        Value::Data(terminusdb_schema::XSDAnySimpleType::String(date_str)) => {
            // Check that it's a valid ISO 8601 date
            assert!(date_str.len() > 20); // Should be like "2024-01-15T12:34:56.789Z"
            assert!(date_str.contains("T"));
            assert!(date_str.ends_with("Z"));

            // Verify it can be parsed as a date
            let parsed = chrono::DateTime::parse_from_rfc3339(&date_str);
            assert!(parsed.is_ok(), "today!() should produce valid RFC3339 date");
        }
        _ => panic!("Expected Data string value from today!()"),
    }
}

#[test]
fn test_date_comparison_macros() {
    // Test after!
    let after_query = after!(var!(end_date), data!("2024-01-01T00:00:00Z"));
    assert!(matches!(after_query, Query::Greater(_)));

    // Test before!
    let before_query = before!(var!(start_date), data!("2024-12-31T23:59:59Z"));
    assert!(matches!(before_query, Query::Less(_)));

    // Test in_between!
    let between_query = in_between!(
        var!(event_date),
        data!("2024-01-01T00:00:00Z"),
        data!("2024-12-31T23:59:59Z")
    );
    match between_query {
        Query::And(and) => {
            assert_eq!(and.and.len(), 2);
            // First condition should be >= (which is an Or)
            match &and.and[0] {
                Query::Or(or) => assert_eq!(or.or.len(), 2),
                _ => panic!("Expected Or for >= comparison"),
            }
            // Second condition should be <= (which is an Or)
            match &and.and[1] {
                Query::Or(or) => assert_eq!(or.or.len(), 2),
                _ => panic!("Expected Or for <= comparison"),
            }
        }
        _ => panic!("Expected And query for in_between!"),
    }
}

#[test]
fn test_combined_string_date_query() {
    // Test a realistic query combining string and date operations
    let query = select!(
        [session, title, date],
        and!(
            type_!(var!(session), "Session"),
            triple!(var!(session), "title", var!(title)),
            triple!(var!(session), "date", var!(date)),
            starts_with!(var!(title), "Annual"),
            ends_with!(var!(title), "Report"),
            in_between!(var!(date), data!("2024-01-01T00:00:00Z"), today!())
        )
    );

    match query {
        Query::Select(s) => {
            assert_eq!(s.variables.len(), 3);
            match &*s.query {
                Query::And(a) => {
                    assert_eq!(a.and.len(), 6);
                    // Check that we have two Regexp queries
                    let regexp_count = a
                        .and
                        .iter()
                        .filter(|q| matches!(q, Query::Regexp(_)))
                        .count();
                    assert_eq!(regexp_count, 2);
                }
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_date_macros_with_today() {
    // Test using today! in comparisons
    let after_today = after!(var!(future_date), today!());
    assert!(matches!(after_today, Query::Greater(_)));

    let before_today = before!(var!(past_date), today!());
    assert!(matches!(before_today, Query::Less(_)));

    // Test date range from today to future
    let future_range = in_between!(var!(event_date), today!(), data!("2025-12-31T23:59:59Z"));
    assert!(matches!(future_range, Query::And(_)));

    // Test today_in_between! macro
    let today_check =
        today_in_between!(data!("2020-01-01T00:00:00Z"), data!("2030-12-31T23:59:59Z"));
    match today_check {
        Query::And(and) => {
            assert_eq!(and.and.len(), 2);
            // First should be >= comparison with today
            match &and.and[0] {
                Query::Or(or) => {
                    assert_eq!(or.or.len(), 2);
                    // Should contain a Greater and an Equals
                }
                _ => panic!("Expected Or for >= comparison"),
            }
            // Second should be <= comparison with today
            match &and.and[1] {
                Query::Or(or) => {
                    assert_eq!(or.or.len(), 2);
                    // Should contain a Less and an Equals
                }
                _ => panic!("Expected Or for <= comparison"),
            }
        }
        _ => panic!("Expected And query"),
    }
}

#[test]
fn test_string_patterns_with_special_chars() {
    // Test with regex special characters that should be handled
    let email_domain = ends_with!(var!(email), ".com");
    let file_ext = ends_with!(var!(filename), ".pdf");
    let url_protocol = starts_with!(var!(url), "https://");

    // All should produce Regexp queries
    assert!(matches!(email_domain, Query::Regexp(_)));
    assert!(matches!(file_ext, Query::Regexp(_)));
    assert!(matches!(url_protocol, Query::Regexp(_)));
}

#[test]
fn test_practical_filter_example() {
    // Example: Find all documents created this year with specific naming pattern
    let current_year = chrono::Utc::now().year();
    let year_start = format!("{}-01-01T00:00:00Z", current_year);
    let year_end = format!("{}-12-31T23:59:59Z", current_year);

    let query = select!(
        [doc, title, created],
        and!(
            type_!(var!(doc), "Document"),
            triple!(var!(doc), "title", var!(title)),
            triple!(var!(doc), "created_date", var!(created)),
            starts_with!(var!(title), "DOC-"),
            contains!(var!(title), current_year.to_string().as_str()),
            in_between!(var!(created), data!(year_start), data!(year_end))
        )
    );

    // Verify query structure
    match &query {
        Query::Select(s) => {
            assert_eq!(s.variables.len(), 3);
            match &*s.query {
                Query::And(a) => {
                    assert_eq!(a.and.len(), 6);
                }
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }

    // Test DSL output
    let dsl = query.to_dsl();
    assert!(dsl.contains("regexp("));
    assert!(dsl.contains("DOC-"));
}

#[test]
fn test_today_in_between_practical() {
    // Example: Find active subscriptions (where today is between start and end dates)
    let query = select!(
        [subscription, name, start_date, end_date],
        and!(
            type_!(var!(subscription), "Subscription"),
            triple!(var!(subscription), "name", var!(name)),
            triple!(var!(subscription), "start_date", var!(start_date)),
            triple!(var!(subscription), "end_date", var!(end_date)),
            today_in_between!(var!(start_date), var!(end_date))
        )
    );

    // Verify query structure
    match &query {
        Query::Select(s) => {
            assert_eq!(s.variables.len(), 4);
            match &*s.query {
                Query::And(a) => {
                    assert_eq!(a.and.len(), 5);
                    // The last condition should be an And (from today_in_between!)
                    match &a.and[4] {
                        Query::And(inner_and) => {
                            assert_eq!(inner_and.and.len(), 2);
                        }
                        _ => panic!("Expected And query from today_in_between!"),
                    }
                }
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }

    // Test with constant dates
    let holiday_check =
        today_in_between!(data!("2024-12-20T00:00:00Z"), data!("2025-01-10T00:00:00Z"));

    // Should produce an And query with two Or conditions
    assert!(matches!(holiday_check, Query::And(_)));
}
