#[cfg(test)]
mod test_datetime_macro {
    use terminusdb_woql2::*;
    use terminusdb_schema::XSDAnySimpleType;
    use chrono::Utc;
    use terminusdb_woql2::value::Value;

    #[test]
    fn test_datetime_macro() {
        let dt = datetime!("2024-01-01T00:00:00Z");
        
        match dt {
            Value::Data(XSDAnySimpleType::DateTime(datetime)) => {
                assert_eq!(datetime.to_rfc3339(), "2024-01-01T00:00:00+00:00");
            }
            _ => panic!("Expected Value::Data(XSDAnySimpleType::DateTime)"),
        }
    }

    #[test]
    fn test_datetime_macro_with_timezone() {
        let dt = datetime!("2024-12-31T23:59:59+01:00");
        
        match dt {
            Value::Data(XSDAnySimpleType::DateTime(datetime)) => {
                // Should be converted to UTC
                assert_eq!(datetime.to_rfc3339(), "2024-12-31T22:59:59+00:00");
            }
            _ => panic!("Expected Value::Data(XSDAnySimpleType::DateTime)"),
        }
    }

    #[test]
    fn test_today_macro() {
        let today = today!();
        
        match today {
            Value::Data(XSDAnySimpleType::DateTime(datetime)) => {
                // Just check it's a valid datetime close to now
                let now = Utc::now();
                let diff = (now - datetime).num_seconds().abs();
                assert!(diff < 2, "today!() should return current time, diff was {} seconds", diff);
            }
            _ => panic!("Expected Value::Data(XSDAnySimpleType::DateTime)"),
        }
    }

    #[test]
    fn test_datetime_in_queries() {
        use terminusdb_woql2::query::Query;
        
        // Test using datetime in a comparison
        let q = greater!(var!(created), datetime!("2024-01-01T00:00:00Z"));
        
        // Test using datetime in a triple
        let q2 = triple!(var!(x), "created_at", datetime!("2024-01-01T00:00:00Z"));
        
        // Test with today macro
        let q3 = less!(var!(deadline), today!());
        
        // Verify these compile and produce valid queries
        match q {
            Query::Greater(_) => {},
            _ => panic!("Expected Greater query"),
        }
        
        match q2 {
            Query::Triple(_) => {},
            _ => panic!("Expected Triple query"),
        }
        
        match q3 {
            Query::Less(_) => {},
            _ => panic!("Expected Less query"),
        }
    }
}