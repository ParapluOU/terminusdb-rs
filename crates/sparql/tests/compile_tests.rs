//! Offline structural tests: SPARQL -> WOQL shape assertions, no database.
//!
//! These assert on the compiled `Query`'s DSL rendering (a stable, readable
//! projection of the WOQL AST) and on the IR, so they're fast and hermetic. The
//! live behavioural suite is in `spec.rs`.

use terminusdb_sparql::{compile, ir, to_ir, SparqlError};
use terminusdb_woql2::dsl::ToDSL;

const PREFIX: &str = "PREFIX s: <http://terminusdb.com/schema#>";

fn dsl(sparql: &str) -> String {
    compile(sparql)
        .unwrap_or_else(|e| panic!("compile `{sparql}`: {e}"))
        .query
        .to_dsl()
}

#[test]
fn basic_bgp_projects_and_ands_triples() {
    let q = format!("{PREFIX} SELECT ?name WHERE {{ ?p a s:Person . ?p s:name ?name }}");
    assert_eq!(
        dsl(&q),
        r#"select([$name], and(triple($p, "rdf:type", "@schema:Person"), triple($p, "@schema:name", $name)))"#
    );
}

#[test]
fn a_keyword_maps_to_rdf_type() {
    // SPARQL `a` expands to the rdf:type IRI; a full schema class IRI maps to
    // the @schema: prefix.
    let q = format!("{PREFIX} SELECT ?p WHERE {{ ?p a s:Person }}");
    assert!(dsl(&q).contains(r#"triple($p, "rdf:type", "@schema:Person")"#));
}

#[test]
fn filter_comparisons_map_to_woql_operators() {
    let q = format!(
        "{PREFIX} SELECT ?n WHERE {{ ?p s:name ?n . ?p s:age ?a . \
         FILTER(?a >= 18 && ?a < 65) }}"
    );
    let d = dsl(&q);
    assert!(d.contains("greater_or_equal($a, 18)") || d.contains("gte($a, 18)"), "got: {d}");
    assert!(d.contains("less($a, 65)"), "got: {d}");
}

#[test]
fn not_equals_becomes_not_eq() {
    let q = format!("{PREFIX} SELECT ?n WHERE {{ ?p s:name ?n . FILTER(?n != \"Jane\") }}");
    let d = dsl(&q);
    assert!(d.contains("not(eq($n, \"Jane\"))"), "got: {d}");
}

#[test]
fn optional_becomes_opt() {
    let q = format!(
        "{PREFIX} SELECT ?n ?nick WHERE {{ ?p s:name ?n . OPTIONAL {{ ?p s:nickname ?nick }} }}"
    );
    let d = dsl(&q);
    assert!(d.contains("opt(triple($p, \"@schema:nickname\", $nick))"), "got: {d}");
}

#[test]
fn union_becomes_or() {
    let q = format!(
        "{PREFIX} SELECT ?n WHERE {{ {{ ?p s:name ?n }} UNION {{ ?p s:label ?n }} }}"
    );
    let d = dsl(&q);
    assert!(d.starts_with("select([$n], or("), "got: {d}");
}

#[test]
fn solution_modifiers_nest_canonically() {
    let q = format!(
        "{PREFIX} SELECT DISTINCT ?n WHERE {{ ?p s:name ?n }} ORDER BY DESC(?n) LIMIT 5 OFFSET 2"
    );
    let d = dsl(&q);
    // Limit( Start( Distinct( Select( OrderBy( core ) ) ) ) )
    assert!(d.starts_with("limit(5, start(2, distinct([$n], select([$n], order_by([desc($n)],"), "got: {d}");
}

#[test]
fn select_star_projects_all_pattern_variables() {
    let q = format!("{PREFIX} SELECT * WHERE {{ ?p a s:Person . ?p s:name ?name }}");
    let compiled = compile(&q).unwrap();
    let mut vars = compiled.variables.clone();
    vars.sort();
    assert_eq!(vars, vec!["name".to_string(), "p".to_string()]);
}

#[test]
fn to_ir_exposes_the_intermediate_representation() {
    let q = format!("{PREFIX} SELECT ?n WHERE {{ ?p s:name ?n }}");
    let ir = to_ir(&q).unwrap();
    assert_eq!(ir.projection, vec!["n".to_string()]);
    assert!(matches!(ir.pattern, ir::GraphPattern::Bgp(ref t) if t.len() == 1));
}

#[test]
fn non_select_forms_are_rejected() {
    let ask = format!("{PREFIX} ASK {{ ?p a s:Person }}");
    assert!(matches!(
        compile(&ask),
        Err(SparqlError::UnsupportedForm(_))
    ));
}

#[test]
fn unsupported_constructs_are_named() {
    // Aggregates are out of the v1 subset.
    let q = format!("{PREFIX} SELECT (COUNT(?p) AS ?c) WHERE {{ ?p a s:Person }}");
    match compile(&q) {
        Err(SparqlError::Unsupported(_)) => {}
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn syntax_errors_are_parse_errors() {
    assert!(matches!(
        compile("SELECT ?x WHERE { this is not sparql"),
        Err(SparqlError::Parse(_))
    ));
}
