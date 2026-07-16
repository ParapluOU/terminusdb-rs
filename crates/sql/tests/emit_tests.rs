//! Golden emitter tests: compile SQL and assert on the emitted WOQL structure.
//!
//! We assert on the WOQL JSON (via `to_woql_json`) structurally rather than by
//! exact string equality, because fresh-variable names depend on DataFusion's
//! (deterministic but internal) column ordering. We check the shape: which
//! triples, comparisons, and wrappers are present, and how the projected columns'
//! variables connect to their property triples.

use serde_json::{json, Value};
use terminusdb_sql::{compile_sql, Catalog, SqlQuery};

fn catalog() -> Catalog {
    let docs = vec![
        json!({"@type": "@context", "@base": "i/", "@schema": "s#"}),
        json!({"@id": "Company", "@type": "Class", "name": "xsd:string"}),
        json!({
            "@id": "Person", "@type": "Class",
            "name": "xsd:string",
            "age": {"@type": "Optional", "@class": "xsd:integer"},
            "bio": {"@type": "Optional", "@class": "xsd:string"},
            "score": "xsd:double",
            "height": "xsd:decimal",
            "active": "xsd:boolean",
            "born": "xsd:date",
            "employer": "Company"
        }),
    ];
    Catalog::build("c0", &docs).unwrap()
}

fn compile(sql: &str) -> SqlQuery {
    compile_sql(sql, &catalog()).unwrap_or_else(|e| panic!("compile `{sql}` failed: {e}"))
}

fn wj(q: &SqlQuery) -> Value {
    q.woql.to_woql_json()
}

/// The WOQL variable a projected column maps to.
fn var_of(q: &SqlQuery, sql_name: &str) -> String {
    q.projection
        .iter()
        .find(|p| p.sql_name == sql_name)
        .unwrap_or_else(|| panic!("no projected column `{sql_name}`"))
        .woql_var
        .clone()
}

// --- generic WOQL JSON walkers -------------------------------------------------

fn find_all<'a>(v: &'a Value, ty: &str, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            if m.get("@type").and_then(Value::as_str) == Some(ty) {
                out.push(v);
            }
            for child in m.values() {
                find_all(child, ty, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|c| find_all(c, ty, out)),
        _ => {}
    }
}

fn all<'a>(v: &'a Value, ty: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    find_all(v, ty, &mut out);
    out
}

fn node_of(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.get("node").and_then(Value::as_str).map(String::from)
}
fn var_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)?
        .get("variable")
        .and_then(Value::as_str)
        .map(String::from)
}

/// (subject var, predicate node, object var, object node) for every Triple.
struct T {
    subj: Option<String>,
    pred: Option<String>,
    obj_var: Option<String>,
    obj_node: Option<String>,
}
fn triples(v: &Value) -> Vec<T> {
    all(v, "Triple")
        .into_iter()
        .map(|t| T {
            subj: var_field(t, "subject"),
            pred: node_of(t, "predicate"),
            obj_var: var_field(t, "object"),
            obj_node: node_of(t, "object"),
        })
        .collect()
}

/// The subject variable of the `rdf:type -> @schema:{class}` filter, if present.
fn type_subject(v: &Value, class_iri: &str) -> Option<String> {
    triples(v).into_iter().find_map(|t| {
        (t.pred.as_deref() == Some("rdf:type") && t.obj_node.as_deref() == Some(class_iri))
            .then_some(t.subj)
            .flatten()
    })
}

/// The object variable of the triple with the given predicate.
fn obj_var_for_pred(v: &Value, pred: &str) -> Option<String> {
    triples(v)
        .into_iter()
        .find(|t| t.pred.as_deref() == Some(pred))
        .and_then(|t| t.obj_var)
}

// --- tests ---------------------------------------------------------------------

#[test]
fn simple_select_binds_iri_to_subject_and_name_to_a_triple() {
    let q = compile("SELECT iri, name FROM person");
    let j = wj(&q);
    let subj = type_subject(&j, "@schema:Person").expect("type filter");
    // iri projects the subject variable itself.
    assert_eq!(var_of(&q, "iri"), subj);
    // name projects the object var of the @schema:name triple.
    assert_eq!(var_of(&q, "name"), obj_var_for_pred(&j, "@schema:name").unwrap());
    // No Distinct in a plain (bag) SELECT.
    assert!(all(&j, "Distinct").is_empty());
}

#[test]
fn where_greater_emits_greater_with_data_value() {
    let q = compile("SELECT name FROM person WHERE age > 21");
    let j = wj(&q);
    let age_var = obj_var_for_pred(&j, "@schema:age").unwrap();
    let g = all(&j, "Greater");
    assert_eq!(g.len(), 1);
    assert_eq!(var_field(g[0], "left").as_deref(), Some(age_var.as_str()));
    // right is a DataValue holding xsd:integer 21
    let data = &g[0]["right"]["data"];
    assert_eq!(data["@type"], "xsd:integer");
    assert_eq!(data["@value"], 21);
    // age is used by a filter → its triple is required (not Optional).
    assert!(all(&j, "Optional").is_empty());
}

#[test]
fn where_equals_string_uses_equals_value() {
    let q = compile("SELECT name FROM person WHERE name = 'Jane'");
    let j = wj(&q);
    let eqs = all(&j, "Equals");
    assert_eq!(eqs.len(), 1);
    let name_var = obj_var_for_pred(&j, "@schema:name").unwrap();
    assert_eq!(var_field(eqs[0], "left").as_deref(), Some(name_var.as_str()));
}

#[test]
fn not_equal_becomes_not_of_equals() {
    let q = compile("SELECT name FROM person WHERE age <> 21");
    let j = wj(&q);
    let nots = all(&j, "Not");
    assert_eq!(nots.len(), 1);
    // The Not wraps exactly one Equals.
    assert_eq!(all(&nots[0], "Equals").len(), 1);
}

#[test]
fn and_flattens_or_wraps() {
    let and = compile("SELECT name FROM person WHERE age > 1 AND age < 9");
    let ja = wj(&and);
    assert_eq!(all(&ja, "Greater").len(), 1);
    assert_eq!(all(&ja, "Less").len(), 1);
    assert!(all(&ja, "Or").is_empty());

    let or = compile("SELECT name FROM person WHERE age = 1 OR age = 9");
    let jo = wj(&or);
    assert_eq!(all(&jo, "Or").len(), 1);
    assert_eq!(all(&jo, "Equals").len(), 2);
}

#[test]
fn nullable_selected_column_is_optional() {
    let q = compile("SELECT bio FROM person");
    let j = wj(&q);
    let opt = all(&j, "Optional");
    assert_eq!(opt.len(), 1, "bio is Optional-cardinality → wrapped in Optional");
    // The optional wraps the @schema:bio triple.
    assert_eq!(obj_var_for_pred(opt[0], "@schema:bio").is_some(), true);
}

#[test]
fn order_limit_offset_nest_canonically() {
    let q = compile("SELECT name FROM person ORDER BY name DESC LIMIT 10 OFFSET 5");
    let j = wj(&q);
    assert_eq!(j["@type"], "Limit");
    assert_eq!(j["limit"], 10);
    assert_eq!(j["query"]["@type"], "Start");
    assert_eq!(j["query"]["start"], 5);
    assert_eq!(j["query"]["query"]["@type"], "Select");
    assert_eq!(j["query"]["query"]["query"]["@type"], "OrderBy");
    let ord = &j["query"]["query"]["query"]["ordering"][0];
    assert_eq!(ord["order"], "desc");
    assert_eq!(
        ord["variable"].as_str().unwrap(),
        var_of(&q, "name").as_str()
    );
}

#[test]
fn distinct_emits_a_distinct_node() {
    let q = compile("SELECT DISTINCT name FROM person");
    let j = wj(&q);
    assert_eq!(j["@type"], "Distinct");
    assert_eq!(all(&j, "Distinct").len(), 1);
}

#[test]
fn inner_join_unifies_object_ref_with_target_subject() {
    let q = compile("SELECT p.name, c.name FROM person p JOIN company c ON p.employer = c.iri");
    let j = wj(&q);
    // Both class filters present.
    let person_subj = type_subject(&j, "@schema:Person").unwrap();
    let company_subj = type_subject(&j, "@schema:Company").unwrap();
    assert_ne!(person_subj, company_subj, "distinct subject variables");
    // The employer object var is unified (via Equals) with the company subject.
    let employer_var = obj_var_for_pred(&j, "@schema:employer").unwrap();
    let eq = all(&j, "Equals");
    assert_eq!(eq.len(), 1);
    let (l, r) = (
        var_field(eq[0], "left").unwrap(),
        var_field(eq[0], "right").unwrap(),
    );
    assert!(
        (l == employer_var && r == company_subj) || (l == company_subj && r == employer_var),
        "Equals should unify employer var with company subject"
    );
    // No Optional/Distinct for an inner join.
    assert!(all(&j, "Optional").is_empty());
}

#[test]
fn left_join_wraps_right_side_in_one_optional() {
    let q = compile("SELECT p.name, c.name FROM person p LEFT JOIN company c ON p.employer = c.iri");
    let j = wj(&q);
    let opt = all(&j, "Optional");
    assert_eq!(opt.len(), 1, "exactly one Optional for the nullable side");
    // The company type-filter and the join Equals live inside the Optional.
    assert!(type_subject(opt[0], "@schema:Company").is_some());
    assert_eq!(all(opt[0], "Equals").len(), 1);
    // The left (person) side is outside the optional.
    assert!(type_subject(&j, "@schema:Person").is_some());
}

#[test]
fn literal_type_matrix() {
    // Integer / Decimal / DateTime serialise as typed {@type,@value} objects;
    // String / Boolean / Float / Date serialise as bare JSON scalars (a quirk of
    // this codebase's XSDAnySimpleType serialisation).

    // integer -> typed object
    let j = wj(&compile("SELECT name FROM person WHERE age > 3"));
    let d = &all(&j, "Greater")[0]["right"]["data"];
    assert_eq!(d["@type"], "xsd:integer");
    assert_eq!(d["@value"], 3);

    // xsd:double column -> Float -> bare JSON number (DataFusion types an
    // unsuffixed numeric literal as Float64, so a bare decimal literal never
    // reaches the emitter as Decimal128; decimal_from_i128 is unit-tested directly).
    let j = wj(&compile("SELECT name FROM person WHERE score > 1.5"));
    assert!(all(&j, "Greater")[0]["right"]["data"].is_number());

    // boolean -> bare JSON bool
    let j = wj(&compile("SELECT name FROM person WHERE active = true"));
    assert_eq!(all(&j, "Equals")[0]["right"]["data"], serde_json::json!(true));

    // string -> bare JSON string
    let j = wj(&compile("SELECT name FROM person WHERE name = 'Jane'"));
    assert_eq!(all(&j, "Equals")[0]["right"]["data"], serde_json::json!("Jane"));

    // date -> bare JSON string
    let j = wj(&compile("SELECT name FROM person WHERE born > DATE '2000-01-01'"));
    assert_eq!(
        all(&j, "Greater")[0]["right"]["data"],
        serde_json::json!("2000-01-01")
    );
}

#[test]
fn aliased_select_with_order_by_projected_away_column() {
    // Regression: `ORDER BY <projected-away column>` makes DataFusion insert an
    // inner projection whose aliases the outer projection references. The emitter
    // must thread those aliases back to the scan-bound variables.
    let q = compile("SELECT name AS n FROM person ORDER BY age");
    let j = wj(&q);
    // The projected column keeps its alias and binds the @schema:name triple var.
    assert_eq!(q.projection.len(), 1);
    assert_eq!(q.projection[0].sql_name, "n");
    assert_eq!(var_of(&q, "n"), obj_var_for_pred(&j, "@schema:name").unwrap());
    // ORDER BY is on the age var (which is present in the core).
    assert_eq!(all(&j, "OrderBy").len(), 1);
}

#[test]
fn bag_semantics_no_distinct_unless_requested() {
    let j = wj(&compile("SELECT name FROM person"));
    assert!(all(&j, "Distinct").is_empty());
}
