#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_query_string_parsing() {
        // Test that we can detect JSON-LD vs DSL format
        let json_query = r#"{"@type": "Select", "variables": ["Subject"]}"#;
        assert!(serde_json::from_str::<serde_json::Value>(json_query).is_ok());

        let dsl_query = r#"select([$Subject], triple($Subject, "rdf:type", "owl:Class"))"#;
        assert!(serde_json::from_str::<serde_json::Value>(dsl_query).is_err());
    }

    #[test]
    fn test_json_ld_format_detection() {
        // Test various JSON-LD formats
        let valid_json = vec![
            r#"{"@type": "Select", "variables": ["X"]}"#,
            r#"{"@type": "And", "and": []}"#,
            r#"{"@type": "Triple", "subject": {"@type": "NodeValue", "variable": "S"}}"#,
        ];

        for json in valid_json {
            assert!(
                serde_json::from_str::<serde_json::Value>(json).is_ok(),
                "Should parse as valid JSON: {}",
                json
            );
        }
    }

    #[test]
    fn test_dsl_format_detection() {
        // Test various DSL formats that should NOT parse as JSON
        let dsl_queries = vec![
            r#"select([$X], triple($X, "rdf:type", "owl:Class"))"#,
            r#"and(triple($S, $P, $O), greater($O, 5))"#,
            r#"opt(triple($X, "rdfs:label", $Label))"#,
        ];

        for dsl in dsl_queries {
            assert!(
                serde_json::from_str::<serde_json::Value>(dsl).is_err(),
                "Should NOT parse as JSON: {}",
                dsl
            );
        }
    }
}
