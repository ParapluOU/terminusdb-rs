//! Offline catalog tests: feed hand-built schema-graph JSON to `Catalog::build`
//! and assert on the resulting table/column set, collisions, omissions, and that
//! DataFusion planning succeeds/fails with precise catalog-aware errors.

use serde_json::{json, Value};
use terminusdb_sql::{Catalog, ColumnKind, SqlError, TableMeta};

/// A schema with datatype, object, enum, multi-valued, rejected-type, abstract,
/// and subdocument features.
fn sample_docs() -> Vec<Value> {
    vec![
        json!({"@type": "@context", "@base": "i/", "@schema": "s#"}),
        // Abstract parent — not a table, contributes an inherited column.
        json!({"@id": "Named", "@type": "Class", "@abstract": [], "label": "xsd:string"}),
        // Subdocument — not a top-level table.
        json!({"@id": "Address", "@type": "Class", "@subdocument": [], "@key": {"@type": "Random"}, "city": "xsd:string"}),
        json!({"@id": "Color", "@type": "Enum", "@value": ["red", "green", "blue"]}),
        json!({"@id": "Company", "@type": "Class", "name": "xsd:string"}),
        json!({
            "@id": "Person", "@type": "Class", "@inherits": ["Named"],
            "name": "xsd:string",
            "age": {"@type": "Optional", "@class": "xsd:integer"},
            "employer": "Company",
            "home": "Address",
            "fav_color": "Color",
            "tags": {"@type": "Set", "@class": "xsd:string"},
            "dur": "xsd:duration"
        }),
    ]
}

fn person(cat: &Catalog) -> TableMeta {
    cat.table("person").expect("person table").clone()
}

#[test]
fn tables_are_concrete_classes_only() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let mut names: Vec<_> = cat.tables().map(|t| t.sql_name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["company", "person"]); // no Named (abstract), no Address (subdoc)
}

#[test]
fn person_columns_include_id_and_supported_props() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let p = person(&cat);
    let mut cols: Vec<_> = p.columns.iter().map(|c| c.sql_name.clone()).collect();
    cols.sort();
    // iri (synthetic subject) + inherited label + name + age + employer + fav_color.
    // tags (multi-valued), dur (rejected), home (subdocument) are omitted.
    assert_eq!(
        cols,
        vec!["age", "employer", "fav_color", "iri", "label", "name"]
    );
}

#[test]
fn iri_is_the_subject_column() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let p = person(&cat);
    let iri = p.column("iri").unwrap();
    assert!(iri.is_id());
    assert!(!iri.nullable);
    assert!(iri.predicate.is_none());
}

#[test]
fn object_property_is_a_joinable_ref() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let p = person(&cat);
    let employer = p.column("employer").unwrap();
    match &employer.kind {
        ColumnKind::ObjectRef { target_class_iri } => assert_eq!(target_class_iri, "Company"),
        other => panic!("expected ObjectRef, got {other:?}"),
    }
    assert_eq!(employer.predicate.as_deref(), Some("@schema:employer"));
}

#[test]
fn enum_property_is_a_string_with_values() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let p = person(&cat);
    match &p.column("fav_color").unwrap().kind {
        ColumnKind::Enum { values } => assert_eq!(values, &vec!["red", "green", "blue"]),
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn optional_property_is_nullable_and_required_is_not() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let p = person(&cat);
    assert!(p.column("age").unwrap().nullable, "age is Optional");
    assert!(!p.column("name").unwrap().nullable, "name is exactly-one");
}

#[test]
fn inherited_property_is_present() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let p = person(&cat);
    assert!(p.column("label").is_some(), "label inherited from Named");
}

#[test]
fn omitted_properties_are_recorded_with_reasons() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let p = person(&cat);
    let omitted: std::collections::HashMap<_, _> = p
        .omitted
        .iter()
        .map(|o| (o.sql_name.clone(), o.reason.to_string()))
        .collect();
    assert!(omitted.get("tags").unwrap().contains("multi-valued"));
    assert!(omitted.get("dur").unwrap().contains("duration"));
    assert!(omitted.get("home").unwrap().contains("subdocument"));
}

#[test]
fn table_collision_is_a_hard_error_naming_both() {
    let docs = vec![
        json!({"@id": "Person", "@type": "Class", "name": "xsd:string"}),
        json!({"@id": "person", "@type": "Class", "name": "xsd:string"}),
    ];
    match Catalog::build("c0", &docs) {
        Err(SqlError::IdentifierCollision { sql, first, second }) => {
            assert_eq!(sql, "person");
            assert!([&first, &second].contains(&&"Person".to_string()));
            assert!([&first, &second].contains(&&"person".to_string()));
        }
        other => panic!("expected collision, got {other:?}"),
    }
}

#[test]
fn column_collision_is_a_hard_error() {
    let docs = vec![json!({
        "@id": "T", "@type": "Class",
        "first-name": "xsd:string",
        "first_name": "xsd:string"  // hyphen sanitises to underscore -> both first_name
    })];
    assert!(matches!(
        Catalog::build("c0", &docs),
        Err(SqlError::IdentifierCollision { .. })
    ));
}

#[test]
fn property_colliding_with_synthetic_iri_is_rejected() {
    let docs = vec![json!({"@id": "T", "@type": "Class", "iri": "xsd:string"})];
    assert!(matches!(
        Catalog::build("c0", &docs),
        Err(SqlError::IdentifierCollision { .. })
    ));
}

#[test]
fn a_real_id_property_coexists_with_the_synthetic_iri() {
    // The common `id: EntityIDFor<Self>` pattern yields an `id` datatype property;
    // it must be exposed as its own column alongside the synthetic `iri`.
    let docs = vec![json!({"@id": "T", "@type": "Class", "id": "xsd:string", "name": "xsd:string"})];
    let cat = Catalog::build("c0", &docs).unwrap();
    let t = cat.table("t").unwrap();
    assert!(t.column("iri").unwrap().is_id());
    assert!(!t.column("id").unwrap().is_id(), "schema `id` is a normal column");
}

// --- planning against the catalog (DataFusion does the checking) ---

#[test]
fn plans_a_simple_select() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    assert!(cat.plan("SELECT iri, name FROM person").is_ok());
}

#[test]
fn plans_a_join_on_object_ref() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    let plan = cat.plan(
        "SELECT p.name, c.name FROM person p JOIN company c ON p.employer = c.iri",
    );
    assert!(plan.is_ok(), "join plan failed: {plan:?}");
}

#[test]
fn selecting_an_omitted_column_is_precise() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    match cat.plan("SELECT tags FROM person") {
        Err(SqlError::UnsupportedColumn { table, column, reason }) => {
            assert_eq!(table, "person");
            assert_eq!(column, "tags");
            assert!(reason.contains("multi-valued"));
        }
        other => panic!("expected UnsupportedColumn, got {other:?}"),
    }
}

#[test]
fn selecting_a_rejected_type_column_is_precise() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    match cat.plan("SELECT dur FROM person") {
        Err(SqlError::UnsupportedColumn { column, reason, .. }) => {
            assert_eq!(column, "dur");
            assert!(reason.contains("duration"));
        }
        other => panic!("expected UnsupportedColumn, got {other:?}"),
    }
}

#[test]
fn referencing_a_subdocument_class_as_a_table_is_precise() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    match cat.plan("SELECT city FROM address") {
        Err(SqlError::UnsupportedTable { table, reason }) => {
            assert_eq!(table, "address");
            assert!(reason.contains("subdocument"));
        }
        other => panic!("expected UnsupportedTable, got {other:?}"),
    }
}

#[test]
fn unknown_table_is_a_plan_error() {
    let cat = Catalog::build("c0", &sample_docs()).unwrap();
    assert!(matches!(
        cat.plan("SELECT * FROM nonexistent"),
        Err(SqlError::Plan(_))
    ));
}
