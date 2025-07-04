#[cfg(test)]
mod tests {
    use terminusdb_client::CommitHistoryEntry;
    use chrono::{DateTime, Utc, Datelike, Timelike};

    #[test]
    fn test_timestamp_datetime_parsing() {
        // Test ISO 8601 format
        let entry = CommitHistoryEntry {
            author: "test_user".to_string(),
            identifier: "abc123".to_string(),
            message: "Test commit".to_string(),
            timestamp: "2023-12-01T10:30:00Z".to_string(),
        };

        let result = entry.timestamp_datetime();
        assert!(result.is_ok());
        
        let datetime = result.unwrap();
        assert_eq!(datetime.year(), 2023);
        assert_eq!(datetime.month(), 12);
        assert_eq!(datetime.day(), 1);
        assert_eq!(datetime.hour(), 10);
        assert_eq!(datetime.minute(), 30);
        assert_eq!(datetime.second(), 0);
    }

    #[test]
    fn test_timestamp_datetime_with_milliseconds() {
        let entry = CommitHistoryEntry {
            author: "test_user".to_string(),
            identifier: "abc123".to_string(),
            message: "Test commit".to_string(),
            timestamp: "2023-12-01T10:30:00.123Z".to_string(),
        };

        let result = entry.timestamp_datetime();
        assert!(result.is_ok());
        
        let datetime = result.unwrap();
        assert_eq!(datetime.timestamp_millis() % 1000, 123);
    }

    #[test]
    fn test_timestamp_datetime_invalid_format() {
        let entry = CommitHistoryEntry {
            author: "test_user".to_string(),
            identifier: "abc123".to_string(),
            message: "Test commit".to_string(),
            timestamp: "not a valid timestamp".to_string(),
        };

        let result = entry.timestamp_datetime();
        assert!(result.is_err());
        
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to parse timestamp"));
    }
}