#[cfg(test)]
mod tests {
    use chrono::{Datelike, Timelike};
    use terminusdb_client::{CommitHistoryEntry, CommitId};

    #[test]
    fn test_timestamp_datetime_parsing() {
        // Unix timestamp 1701423000 = 2023-12-01T09:30:00Z
        let entry = CommitHistoryEntry {
            author: "test_user".to_string(),
            identifier: CommitId::new("abc123"),
            message: "Test commit".to_string(),
            timestamp: 1701423000.0,
        };

        let result = entry.timestamp_datetime();
        assert!(result.is_ok());

        let datetime = result.unwrap();
        assert_eq!(datetime.year(), 2023);
        assert_eq!(datetime.month(), 12);
        assert_eq!(datetime.day(), 1);
        assert_eq!(datetime.hour(), 9); // 09:30:00 UTC
        assert_eq!(datetime.minute(), 30);
        assert_eq!(datetime.second(), 0);
    }

    #[test]
    fn test_timestamp_datetime_with_milliseconds() {
        // Use integer milliseconds to avoid floating-point precision issues
        // 1701423000 seconds + 123 milliseconds
        let entry = CommitHistoryEntry {
            author: "test_user".to_string(),
            identifier: CommitId::new("abc123"),
            message: "Test commit".to_string(),
            timestamp: 1701423000.0 + 0.123,
        };

        let result = entry.timestamp_datetime();
        assert!(result.is_ok());

        let datetime = result.unwrap();
        // Allow for small floating-point rounding (122 or 123 are both acceptable)
        let millis = datetime.timestamp_millis() % 1000;
        assert!(millis >= 122 && millis <= 124, "Expected ~123ms, got {}", millis);
    }

    #[test]
    fn test_timestamp_datetime_negative() {
        // Negative timestamps are valid - they represent dates before Unix epoch (1970)
        // -86400.0 = 1969-12-31T00:00:00Z (one day before epoch)
        let entry = CommitHistoryEntry {
            author: "test_user".to_string(),
            identifier: CommitId::new("abc123"),
            message: "Test commit".to_string(),
            timestamp: -86400.0,
        };

        let result = entry.timestamp_datetime();
        assert!(result.is_ok());

        let datetime = result.unwrap();
        assert_eq!(datetime.year(), 1969);
        assert_eq!(datetime.month(), 12);
        assert_eq!(datetime.day(), 31);
    }
}
