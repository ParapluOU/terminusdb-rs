use terminusdb_woql_dsl::parse_woql_dsl;
use terminusdb_woql2::query::Query;
use terminusdb_woql2::control::Select;
use terminusdb_woql2::query::And;

#[test]
fn test_complex_query() {
    let dsl = r#"
select(
    [$Name, $Age],
    and(
        triple($Person, "rdf:type", "@schema:Person"),
        triple($Person, "@schema:name", $Name),
        triple($Person, "@schema:age", $Age),
        greater($Age, 18)
    )
)
"#;

    let query = parse_woql_dsl(dsl).unwrap();

    match query {
        Query::Select(select) => {
            assert_eq!(select.variables.len(), 2);
            assert_eq!(select.variables[0], "Name");
            assert_eq!(select.variables[1], "Age");

            match select.query.as_ref() {
                Query::And(and) => {
                    assert_eq!(and.and.len(), 4);
                }
                _ => panic!("Expected And query inside Select"),
            }
        }
        _ => panic!("Expected Select query"),
    }
}

#[test]
fn test_nested_query() {
    let dsl = r#"
or(
    and(
        triple($Person, "@schema:age", $Age),
        greater($Age, 18)
    ),
    triple($Person, "@schema:isAdult", true)
)
"#;

    let query = parse_woql_dsl(dsl).unwrap();

    match query {
        Query::Or(or) => {
            assert_eq!(or.or.len(), 2);
        }
        _ => panic!("Expected Or query"),
    }
}