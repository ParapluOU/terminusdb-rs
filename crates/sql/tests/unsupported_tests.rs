//! Every construct outside the v1 subset must be rejected loudly — either at
//! DataFusion planning (Parse/Plan) or by the emitter (Unsupported) — never
//! silently mistranslated.

use serde_json::{json, Value};
use terminusdb_sql::{compile_sql, Catalog, SqlError};

fn cat() -> Catalog {
    let docs = vec![
        json!({"@type": "@context", "@base": "i/", "@schema": "s#"}),
        json!({"@id": "Company", "@type": "Class", "name": "xsd:string"}),
        json!({
            "@id": "Person", "@type": "Class",
            "name": "xsd:string",
            "age": {"@type": "Optional", "@class": "xsd:integer"},
            "employer": "Company"
        }),
    ];
    Catalog::build("c0", &docs).unwrap()
}

fn err(sql: &str) -> SqlError {
    let c = cat();
    compile_sql(sql, &c).expect_err(&format!("expected `{sql}` to be rejected"))
}

fn assert_unsupported(sql: &str) {
    match err(sql) {
        SqlError::Unsupported(_) => {}
        other => panic!("expected Unsupported for `{sql}`, got {other:?}"),
    }
}

/// Constructs that plan fine but the emitter refuses to translate.
#[test]
fn emitter_rejected_constructs() {
    assert_unsupported("SELECT name FROM person WHERE age IS NULL");
    assert_unsupported("SELECT name FROM person WHERE age IS NOT NULL");
    assert_unsupported("SELECT name FROM person WHERE name LIKE 'J%'");
    assert_unsupported("SELECT age + 1 FROM person");
    assert_unsupported("SELECT name FROM person WHERE age IN (1, 2)");
    assert_unsupported("SELECT p.name FROM person p FULL JOIN company c ON p.employer = c.iri");
    assert_unsupported("SELECT p.name FROM person p RIGHT JOIN company c ON p.employer = c.iri");
    assert_unsupported("SELECT CASE WHEN age > 1 THEN 'a' ELSE 'b' END AS x FROM person");
    // A non-equality comparison between two columns has no equijoin translation.
    assert_unsupported("SELECT name FROM person WHERE age > age");
}

/// Garbage SQL is a parse error.
#[test]
fn garbage_sql_is_a_parse_error() {
    assert!(matches!(err("this is not valid sql"), SqlError::Parse(_)));
}

/// Aggregates are phase 2 — they are rejected (DataFusion has no aggregate
/// function registered in v1), just not necessarily by our emitter.
#[test]
fn aggregates_are_rejected() {
    assert!(compile_sql("SELECT COUNT(*) FROM person", &cat()).is_err());
    assert!(compile_sql("SELECT SUM(age) FROM person", &cat()).is_err());
}

/// UNION / set operations are rejected.
#[test]
fn set_operations_are_rejected() {
    assert!(compile_sql("SELECT name FROM person UNION SELECT name FROM company", &cat()).is_err());
}

/// Multiple statements in one string are rejected.
#[test]
fn multiple_statements_rejected() {
    assert!(matches!(
        err("SELECT name FROM person; SELECT name FROM company"),
        SqlError::Unsupported(_)
    ));
}

#[test]
fn empty_input_is_empty_error() {
    assert!(matches!(
        compile_sql("   ", &cat()).unwrap_err(),
        SqlError::Empty | SqlError::Parse(_)
    ));
    let _: Value = Value::Null;
}
