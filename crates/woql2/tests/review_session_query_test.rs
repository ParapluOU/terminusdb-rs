//! Test for complex ReviewSession filtering query using macros

use terminusdb_woql2::prelude::*;

#[test]
fn test_review_session_filter_query() {
    // Build the complex ReviewSession filter query using macros
    let query = select!(
        [
            SessionId,
            PublicationTitle,
            CommitteeId,
            CommitteeName,
            CommitteeCode
        ],
        and!(
            // Session type and basic properties
            type_!(var!(Session), "@schema:ReviewSession"),
            triple!(var!(Session), "@schema:id", var!(SessionId)),
            triple!(var!(Session), "title", var!(SessionTitle)),
            triple!(var!(Session), "publication_id", var!(PublicationId)),
            // Publication properties
            triple!(var!(Publication), "@schema:id", var!(PublicationId)),
            triple!(var!(Publication), "title", var!(PublicationTitle)),
            // Optional committee information
            optional!(and!(
                triple!(var!(Publication), "committee", var!(CommitteeObj)),
                triple!(var!(CommitteeObj), "id", var!(CommitteeId)),
                triple!(var!(CommitteeObj), "name", var!(CommitteeName)),
                triple!(var!(CommitteeObj), "code", var!(CommitteeCode))
            )),
            // Date range properties
            triple!(var!(Session), "date_range", var!(DateRangeObj)),
            triple!(var!(DateRangeObj), "start", var!(SessionStart)),
            triple!(var!(DateRangeObj), "end", var!(SessionEnd)),
            // Title filtering with regex
            regex!(data!("TITLE_PATTERN"), var!(SessionTitle), var!(TitleMatch)),
            regex!(
                data!("PUB_TITLE_PATTERN"),
                var!(PublicationTitle),
                var!(PubTitleMatch)
            ),
            // Date range filtering
            compare!((var!(SessionEnd)) > (data!("DATE_START"))),
            compare!((var!(SessionStart)) < (data!("DATE_END")))
        )
    );

    // Verify the query structure
    match &query {
        Query::Select(select) => {
            // Check variables
            assert_eq!(select.variables.len(), 5);
            assert_eq!(select.variables[0], "SessionId");
            assert_eq!(select.variables[1], "PublicationTitle");
            assert_eq!(select.variables[2], "CommitteeId");
            assert_eq!(select.variables[3], "CommitteeName");
            assert_eq!(select.variables[4], "CommitteeCode");

            // Check main And query
            match &*select.query {
                Query::And(and) => {
                    assert_eq!(and.and.len(), 14);

                    // Verify first triple is the rdf:type
                    match &and.and[0] {
                        Query::Triple(t) => {
                            assert!(
                                matches!(t.predicate, NodeValue::Node(ref s) if s == "rdf:type")
                            );
                            assert!(
                                matches!(t.object, Value::Node(ref s) if s == "@schema:ReviewSession")
                            );
                        }
                        _ => panic!("Expected Triple for rdf:type"),
                    }

                    // Verify optional committee block
                    match &and.and[6] {
                        Query::WoqlOptional(opt) => match &*opt.query {
                            Query::And(inner_and) => {
                                assert_eq!(inner_and.and.len(), 4);
                            }
                            _ => panic!("Expected And inside Optional"),
                        },
                        _ => panic!("Expected Optional query at position 6"),
                    }

                    // Verify regex patterns
                    match &and.and[10] {
                        Query::Regexp(r) => {
                            assert!(matches!(r.pattern, DataValue::Data(_)));
                            assert!(
                                matches!(r.string, DataValue::Variable(ref s) if s == "SessionTitle")
                            );
                            assert!(r.result.is_some());
                        }
                        _ => panic!("Expected Regexp for title pattern"),
                    }

                    // Verify date comparisons
                    match &and.and[12] {
                        Query::Greater(_) => {}
                        _ => panic!("Expected Greater comparison for date"),
                    }

                    match &and.and[13] {
                        Query::Less(_) => {}
                        _ => panic!("Expected Less comparison for date"),
                    }
                }
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }

    // Test DSL rendering
    let dsl = query.to_dsl();
    println!("Generated DSL:\n{}", dsl);

    // Basic DSL content verification
    assert!(dsl.contains("select("));
    assert!(dsl.contains("SessionId"));
    assert!(dsl.contains("@schema:ReviewSession"));
    assert!(dsl.contains("opt("));
    assert!(dsl.contains("regexp("));
    assert!(dsl.contains("greater("));
    assert!(dsl.contains("less("));
}

#[test]
fn test_review_session_with_limit_offset() {
    // Create the base query
    let base_query = select!(
        [SessionId, PublicationTitle],
        and!(
            type_!(var!(Session), "@schema:ReviewSession"),
            triple!(var!(Session), "@schema:id", var!(SessionId)),
            triple!(var!(Session), "publication_id", var!(PublicationId)),
            triple!(var!(Publication), "@schema:id", var!(PublicationId)),
            triple!(var!(Publication), "title", var!(PublicationTitle))
        )
    );

    // Apply limit and offset (in WOQL these are typically applied as wrappers)
    // Note: For actual limit/offset in the JSON format shown, you'd need to
    // extend the macros or use a builder pattern. Here we show the concept:
    let limited_query = limit!(10, base_query);

    // Verify structure
    match limited_query {
        Query::Limit(l) => {
            assert_eq!(l.limit, 10);
            match &*l.query {
                Query::Select(s) => {
                    assert_eq!(s.variables.len(), 2);
                }
                _ => panic!("Expected Select inside Limit"),
            }
        }
        _ => panic!("Expected Limit query"),
    }
}

#[test]
fn test_review_session_builder_style() {
    // Alternative approach showing how you might build this query programmatically
    // with dynamic filters

    let mut conditions = vec![
        type_!(var!(Session), "@schema:ReviewSession"),
        triple!(var!(Session), "@schema:id", var!(SessionId)),
        triple!(var!(Session), "title", var!(SessionTitle)),
        triple!(var!(Session), "publication_id", var!(PublicationId)),
        triple!(var!(Publication), "@schema:id", var!(PublicationId)),
        triple!(var!(Publication), "title", var!(PublicationTitle)),
    ];

    // Add optional committee info
    conditions.push(optional!(and!(
        triple!(var!(Publication), "committee", var!(CommitteeObj)),
        triple!(var!(CommitteeObj), "id", var!(CommitteeId)),
        triple!(var!(CommitteeObj), "name", var!(CommitteeName)),
        triple!(var!(CommitteeObj), "code", var!(CommitteeCode))
    )));

    // Add date range
    conditions.push(triple!(var!(Session), "date_range", var!(DateRangeObj)));
    conditions.push(triple!(var!(DateRangeObj), "start", var!(SessionStart)));
    conditions.push(triple!(var!(DateRangeObj), "end", var!(SessionEnd)));

    // Add filters
    let title_pattern = ".*test.*"; // In real code: filter.title.map(|t| format!(".*{}.*", t))
    conditions.push(regex!(
        data!(title_pattern),
        var!(SessionTitle),
        var!(TitleMatch)
    ));

    // Date range filters (placeholders would be actual DateTime values)
    conditions.push(compare!(
        (var!(SessionEnd)) > (data!("2024-01-01T00:00:00Z"))
    ));
    conditions.push(compare!(
        (var!(SessionStart)) < (data!("2024-12-31T23:59:59Z"))
    ));

    // Build final query
    let query = Query::Select(terminusdb_woql2::control::Select {
        variables: vec![
            "SessionId".to_string(),
            "PublicationTitle".to_string(),
            "CommitteeId".to_string(),
            "CommitteeName".to_string(),
            "CommitteeCode".to_string(),
        ],
        query: Box::new(Query::And(terminusdb_woql2::query::And { and: conditions })),
    });

    // Verify we built it correctly
    match query {
        Query::Select(s) => {
            assert_eq!(s.variables.len(), 5);
            match &*s.query {
                Query::And(a) => {
                    assert_eq!(a.and.len(), 13);
                }
                _ => panic!("Expected And query"),
            }
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_review_session_with_real_substitutions() {
    // Example showing how the query would look with actual runtime values
    // simulating the filter object and substitutions mentioned in the JSON

    // Simulate filter struct
    struct ReviewSessionFilter {
        title: Option<String>,
        publication_title: Option<String>,
        date_range: Option<DateRange>,
        status: Vec<String>,
    }

    struct DateRange {
        start: String, // Would be DateTime in real code
        end: String,
    }

    let filter = ReviewSessionFilter {
        title: Some("Annual".to_string()),
        publication_title: Some("Science".to_string()),
        date_range: Some(DateRange {
            start: "2024-01-01T00:00:00Z".to_string(),
            end: "2024-12-31T23:59:59Z".to_string(),
        }),
        status: vec!["active".to_string(), "pending".to_string()],
    };

    let page = 1;
    let page_size = 20;

    // Build patterns from filter
    let title_pattern = filter
        .title
        .map(|t| format!(".*{}.*", t))
        .unwrap_or_else(|| ".*".to_string());
    let pub_title_pattern = filter
        .publication_title
        .map(|t| format!(".*{}.*", t))
        .unwrap_or_else(|| ".*".to_string());

    let date_start = filter
        .date_range
        .as_ref()
        .map(|d| d.start.clone())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    let date_end = filter
        .date_range
        .as_ref()
        .map(|d| d.end.clone())
        .unwrap_or_else(|| "2999-12-31T23:59:59Z".to_string());

    // Build the query with actual values
    let mut query_conditions = vec![
        type_!(var!(Session), "@schema:ReviewSession"),
        triple!(var!(Session), "@schema:id", var!(SessionId)),
        triple!(var!(Session), "title", var!(SessionTitle)),
        triple!(var!(Session), "publication_id", var!(PublicationId)),
        triple!(var!(Publication), "@schema:id", var!(PublicationId)),
        triple!(var!(Publication), "title", var!(PublicationTitle)),
        optional!(and!(
            triple!(var!(Publication), "committee", var!(CommitteeObj)),
            triple!(var!(CommitteeObj), "id", var!(CommitteeId)),
            triple!(var!(CommitteeObj), "name", var!(CommitteeName)),
            triple!(var!(CommitteeObj), "code", var!(CommitteeCode))
        )),
        triple!(var!(Session), "date_range", var!(DateRangeObj)),
        triple!(var!(DateRangeObj), "start", var!(SessionStart)),
        triple!(var!(DateRangeObj), "end", var!(SessionEnd)),
        regex!(data!(title_pattern), var!(SessionTitle), var!(TitleMatch)),
        regex!(
            data!(pub_title_pattern),
            var!(PublicationTitle),
            var!(PubTitleMatch)
        ),
        compare!((var!(SessionEnd)) > (data!(date_start))),
        compare!((var!(SessionStart)) < (data!(date_end))),
    ];

    // Add status filter if present (demonstrating OR clause building)
    if !filter.status.is_empty() {
        let status_conditions: Vec<Query> = filter
            .status
            .into_iter()
            .map(|status| triple!(var!(Session), "status", data!(status)))
            .collect();

        if status_conditions.len() == 1 {
            query_conditions.push(status_conditions.into_iter().next().unwrap());
        } else {
            query_conditions.push(Query::Or(terminusdb_woql2::query::Or {
                or: status_conditions,
            }));
        }
    }

    // Build complete query
    let base_query = select!(
        [
            SessionId,
            PublicationTitle,
            CommitteeId,
            CommitteeName,
            CommitteeCode
        ],
        Query::And(terminusdb_woql2::query::And {
            and: query_conditions
        })
    );

    // Apply pagination (Note: WOQL doesn't have built-in OFFSET,
    // so in practice you'd use Start and Limit)
    let offset_value = (page - 1) * page_size;
    let paginated_query = limit!(
        page_size as u64,
        if offset_value > 0 {
            Query::Start(terminusdb_woql2::misc::Start {
                start: offset_value as u64,
                query: Box::new(base_query),
            })
        } else {
            base_query
        }
    );

    // Verify the structure
    let dsl = paginated_query.to_dsl();
    assert!(dsl.contains(".*Annual.*"));
    assert!(dsl.contains(".*Science.*"));
    assert!(dsl.contains("2024-01-01"));
    assert!(dsl.contains("2024-12-31"));
    assert!(dsl.contains("limit(20"));
}
