//! AST-level tests: XPath string -> IR and XPath string -> WOQL `Query`.
//! These prove the compiler shape without needing a running database.

use terminusdb_woql2::prelude::*;
use terminusdb_xpath::{compile, ir, to_ir, XPathError};

/// `to_ir` projects the flagship path onto the expected IR.
#[test]
fn flagship_lowers_to_expected_ir() {
    let q = to_ir(r#"db("name")/document("MyModel/1234")/submodel/@prop"#).unwrap();
    assert_eq!(
        q.head,
        ir::ContextHead::Document {
            db: Some("name".to_string()),
            id: "MyModel/1234".to_string(),
        }
    );
    assert_eq!(
        q.steps,
        vec![
            ir::Step {
                axis: ir::Axis::Child,
                test: ir::NodeTest::Name("submodel".to_string()),
                predicates: vec![],
            },
            ir::Step {
                axis: ir::Axis::Attribute,
                test: ir::NodeTest::Name("prop".to_string()),
                predicates: vec![],
            },
        ]
    );
}

/// The flagship path compiles to `Select([x1], And[Triple, Triple])`.
#[test]
fn flagship_compiles_to_triple_chain() {
    let c = compile(r#"document("MyModel/1234")/submodel/@prop"#).unwrap();
    assert_eq!(c.using_db, None);
    assert_eq!(c.result_var, "x1");

    let Query::Select(sel) = &c.query else {
        panic!("expected Select, got {:?}", c.query);
    };
    assert_eq!(sel.variables, vec!["x1".to_string()]);

    let Query::And(and) = &*sel.query else {
        panic!("expected And");
    };
    assert_eq!(and.and.len(), 2);

    // document -> submodel (object-property hop)
    let Query::Triple(t0) = &and.and[0] else {
        panic!("expected Triple");
    };
    assert_eq!(t0.subject, NodeValue::Node("MyModel/1234".to_string()));
    assert_eq!(t0.predicate, NodeValue::Node("@schema:submodel".to_string()));
    assert_eq!(t0.object, Value::Variable("x0".to_string()));

    // submodel -> @prop (value property, the result)
    let Query::Triple(t1) = &and.and[1] else {
        panic!("expected Triple");
    };
    assert_eq!(t1.subject, NodeValue::Variable("x0".to_string()));
    assert_eq!(t1.predicate, NodeValue::Node("@schema:prop".to_string()));
    assert_eq!(t1.object, Value::Variable("x1".to_string()));
}

/// `document(...)` accepts both a short id and a full IRI (with `://`); both
/// parse as string literals and pass through as the subject node verbatim.
#[test]
fn document_accepts_short_and_full_iri() {
    let short = to_ir(r#"document("MyModel/1234")/@name"#).unwrap();
    assert_eq!(
        short.head,
        ir::ContextHead::Document {
            db: None,
            id: "MyModel/1234".to_string()
        }
    );

    let full = to_ir(r#"document("terminusdb:///data/MyModel/1234")/@name"#).unwrap();
    assert_eq!(
        full.head,
        ir::ContextHead::Document {
            db: None,
            id: "terminusdb:///data/MyModel/1234".to_string()
        }
    );
}

/// `db("...")` is exposed as `using_db` and wraps into WOQL `Using`.
#[test]
fn db_sets_using_and_wraps() {
    let c = compile(r#"db("mydb")/document("X/1")/foo"#).unwrap();
    assert_eq!(c.using_db, Some("mydb".to_string()));

    let Query::Using(using) = c.into_using_query() else {
        panic!("expected Using wrapper");
    };
    assert_eq!(using.collection, "mydb");
    assert!(matches!(*using.query, Query::Select(_)));
}

/// A relative path (no `document(...)`) starts from a fresh subject variable.
#[test]
fn relative_path_starts_from_variable() {
    let c = compile("submodel/@prop").unwrap();
    let Query::Select(sel) = &c.query else {
        panic!("expected Select");
    };
    let Query::And(and) = &*sel.query else {
        panic!("expected And");
    };
    let Query::Triple(t0) = &and.and[0] else {
        panic!("expected Triple");
    };
    assert!(matches!(t0.subject, NodeValue::Variable(_)));
}

/// An equality predicate emits the relative triple plus an `Equals` clause.
#[test]
fn equality_predicate_emits_equals() {
    let c = compile(r#"document("X/1")/person[@name = "Jane"]/age"#).unwrap();
    let Query::Select(sel) = &c.query else {
        panic!("expected Select");
    };
    let Query::And(and) = &*sel.query else {
        panic!("expected And");
    };
    assert!(
        and.and.iter().any(|q| matches!(q, Query::Equals(_))),
        "expected an Equals clause from the predicate, got {:?}",
        and.and
    );
}

/// A numeric `>` predicate emits a `Greater` clause.
#[test]
fn greater_predicate_emits_greater() {
    let c = compile(r#"document("X/1")/person[age > 21]"#).unwrap();
    let Query::Select(sel) = &c.query else {
        panic!("expected Select");
    };
    let Query::And(and) = &*sel.query else {
        panic!("expected And");
    };
    assert!(and.and.iter().any(|q| matches!(q, Query::Greater(_))));
}

/// `//c` compiles to a WOQL `Path` (descendant) clause: a star over any
/// predicate, then `c`.
#[test]
fn descendant_compiles_to_path() {
    let c = compile(r#"document("X/1")//c"#).unwrap();
    let Query::Select(sel) = &c.query else {
        panic!("expected Select");
    };
    let Query::And(and) = &*sel.query else {
        panic!("expected And");
    };
    assert!(and.and.iter().any(|q| matches!(q, Query::Path(_))));
}

/// The typed builder produces the SAME IR as parsing the equivalent string —
/// so the string parser and the builder are two front-ends to one compiler.
#[test]
fn builder_matches_string_form() {
    use terminusdb_xpath::builder::{attr, child, descendant};

    // submodel/@prop  — via `/` operator overloading
    assert_eq!(
        (child("submodel") / attr("prop")).to_ir(),
        to_ir("submodel/@prop").unwrap()
    );

    // @name  (relative attribute)
    assert_eq!(attr("name").to_ir(), to_ir("@name").unwrap());

    // person[@name = "Jane"]/age  — predicate via `.filter`, chain via `/`
    assert_eq!(
        (child("person").filter(attr("name").eq("Jane")) / child("age")).to_ir(),
        to_ir(r#"person[@name = "Jane"]/age"#).unwrap()
    );

    // employer[@founded > 1990]  (numeric comparison predicate)
    assert_eq!(
        child("employer").filter(attr("founded").gt(1990)).to_ir(),
        to_ir("employer[@founded > 1990]").unwrap()
    );

    // a//city  (descendant), two ways: descendant() function and the `>>` operator.
    assert_eq!(
        (child("a") / descendant("city")).to_ir(),
        to_ir("a//city").unwrap()
    );
    assert_eq!(
        (child("a") >> child("city")).to_ir(),
        to_ir("a//city").unwrap()
    );

    // `>>` binds looser than `/`: (doc/a) >> (b/@c) == a//b/@c
    assert_eq!(
        (child("a") >> child("b") / attr("c")).to_ir(),
        to_ir("a//b/@c").unwrap()
    );
}

/// `xpath!("...{}...", arg)` formats then compiles, with a runtime parse check.
#[test]
fn xpath_format_macro() {
    use terminusdb_xpath::xpath;

    // Formatted template equals the directly-compiled string form.
    let formatted = xpath!("{}/@{}", "submodel", "prop").unwrap();
    let direct = to_ir("submodel/@prop").unwrap();
    assert_eq!(formatted.query, compile("submodel/@prop").unwrap().query);
    let _ = direct;

    // Interpolating a document id.
    let compiled = xpath!(r#"document("{}")/submodel/@prop"#, "MyModel/1234").unwrap();
    assert_eq!(compiled.result_var, "x1");

    // The runtime parse check: a malformed template is an Err, not a panic.
    assert!(matches!(
        xpath!("{}", "///"),
        Err(XPathError::Parse(_))
    ));
}

/// Constructs outside the supported subset are rejected explicitly.
#[test]
fn unsupported_constructs_are_rejected() {
    assert!(matches!(compile("1 + 2"), Err(XPathError::Unsupported(_))));
    assert!(matches!(
        compile("document(\"X/1\")/parent::a"),
        Err(XPathError::Unsupported(_))
    ));
    // Garbage syntax is a parse error, not an unsupported error.
    assert!(matches!(compile("///"), Err(XPathError::Parse(_))));
}
