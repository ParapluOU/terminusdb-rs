#[cfg(test)]
mod test_datetime_integration {
    use terminusdb_schema::XSDAnySimpleType;
    use terminusdb_woql2::*;

    #[test]
    fn test_datetime_comparison_queries() {
        // Test creating queries with datetime comparisons
        let q1 = and!(
            triple!(var!(event), "created_at", var!(created)),
            greater!(var!(created), datetime!("2024-01-01T00:00:00Z"))
        );

        // Verify the query structure
        match q1 {
            query::Query::And(and) => {
                assert_eq!(and.and.len(), 2);

                // Check the second query is a Greater comparison
                match &and.and[1] {
                    query::Query::Greater(g) => match &g.right {
                        value::DataValue::Data(XSDAnySimpleType::DateTime(dt)) => {
                            assert_eq!(dt.to_rfc3339(), "2024-01-01T00:00:00+00:00");
                        }
                        _ => panic!("Expected datetime in Greater comparison"),
                    },
                    _ => panic!("Expected Greater query"),
                }
            }
            _ => panic!("Expected And query"),
        }
    }

    #[test]
    fn test_datetime_in_triple() {
        // Test using datetime directly in a triple
        let q = triple!(
            var!(doc),
            "last_modified",
            datetime!("2024-12-31T23:59:59Z")
        );

        match q {
            query::Query::Triple(t) => match t.object {
                value::Value::Data(XSDAnySimpleType::DateTime(dt)) => {
                    assert_eq!(dt.to_rfc3339(), "2024-12-31T23:59:59+00:00");
                }
                _ => panic!("Expected datetime in triple object"),
            },
            _ => panic!("Expected Triple query"),
        }
    }

    #[test]
    fn test_date_range_query() {
        // Test a date range query
        let start_date = datetime!("2024-01-01T00:00:00Z");
        let end_date = datetime!("2024-12-31T23:59:59Z");

        let q = and!(
            triple!(var!(event), "date", var!(d)),
            greater!(var!(d), start_date),
            less!(var!(d), end_date)
        );

        // Verify it creates a valid query structure
        match q {
            query::Query::And(and) => {
                assert_eq!(and.and.len(), 3);
            }
            _ => panic!("Expected And query"),
        }
    }

    #[test]
    fn test_today_in_query() {
        // Test using today!() in queries
        let q = select!(
            [created_at],
            and!(
                triple!(var!(doc), "created_at", var!(created_at)),
                less!(var!(created_at), today!())
            )
        );

        // Verify structure
        match q {
            query::Query::Select(s) => {
                assert_eq!(s.variables, vec!["created_at"]);
                match &*s.query {
                    query::Query::And(and) => {
                        assert_eq!(and.and.len(), 2);
                    }
                    _ => panic!("Expected And query inside Select"),
                }
            }
            _ => panic!("Expected Select query"),
        }
    }
}
