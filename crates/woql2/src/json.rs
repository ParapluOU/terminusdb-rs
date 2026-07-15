//! WOQL-specific JSON-LD serialization.
//!
//! The AST derives its JSON via `ToInstance`, which serializes a `None` optional
//! field as an explicit `"key": null`. TerminusDB rejects null-valued WOQL
//! properties — sometimes loudly ("Not well formed WOQL JSON-LD"), and the
//! meaning of an *absent* optional is never "null". Two examples that bit us:
//!
//! - A triple's `graph: None` must mean the default `instance` graph.
//! - A path's optional `predicate` (any predicate) / `path` (output list) must
//!   be **omitted** — an omitted path predicate is precisely the "any predicate"
//!   wildcard; sent as `null` it is rejected.
//!
//! [`normalize_woql_json`] fixes both: it drops null-valued properties (so an
//! absent optional is absent), except `graph`, which becomes `"instance"`. It is
//! applied centrally by the client at the HTTP send funnel (`query_raw` and
//! friends), so every query path is covered no matter how its JSON was produced.
//! [`Query::to_woql_json`] is a convenience that produces the same normalized
//! JSON for debugging/preview.

use serde_json::Value;
use terminusdb_schema::{ToJson, ToTDBInstance};

use crate::prelude::Query;

impl Query {
    /// Serialize this query to the WOQL JSON-LD the server will receive
    /// ([`normalize_woql_json`] applied). Handy for previewing/logging the exact
    /// payload; the client applies the same normalization on send, so calling
    /// this is not required for correctness.
    pub fn to_woql_json(&self) -> Value {
        let mut json = self.to_instance(None).to_json();
        normalize_woql_json(&mut json);
        json
    }
}

/// Recursively normalize serialized WOQL JSON for sending:
///
/// - a null `graph` becomes the default `"instance"` graph;
/// - every other null-valued property is **removed** (an absent optional must be
///   absent, not `null` — e.g. an omitted path `predicate` means "any predicate").
///
/// Safe to run over arbitrary WOQL JSON.
pub fn normalize_woql_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let null_keys: Vec<String> = map
                .iter()
                .filter(|(_, v)| v.is_null())
                .map(|(k, _)| k.clone())
                .collect();
            for key in null_keys {
                if key == "graph" {
                    map.insert(key, Value::String("instance".to_string()));
                } else {
                    map.remove(&key);
                }
            }
            for v in map.values_mut() {
                normalize_woql_json(v);
            }
        }
        Value::Array(items) => {
            for v in items.iter_mut() {
                normalize_woql_json(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::{NodeValue, Query, Triple};
    use crate::value::Value as WoqlValue;
    use terminusdb_schema::{ToJson, ToTDBInstance};

    #[test]
    fn none_graph_serializes_as_instance() {
        let q = Query::Triple(Triple {
            subject: NodeValue::Variable("S".to_string()),
            predicate: NodeValue::Node("p".to_string()),
            object: WoqlValue::Variable("O".to_string()),
            graph: None,
        });
        let json = q.to_woql_json();
        assert_eq!(json["graph"], "instance", "None graph should default to instance");
        assert!(json["graph"].is_string());
    }

    #[test]
    fn none_graph_is_instance_at_source_without_normalization() {
        // Thanks to `#[tdb(default)]` on Triple.graph, even a bare
        // to_instance().to_json() (no funnel walk) emits "instance", not null.
        let q = Query::Triple(Triple {
            subject: NodeValue::Variable("S".to_string()),
            predicate: NodeValue::Node("p".to_string()),
            object: WoqlValue::Variable("O".to_string()),
            graph: None,
        });
        let raw = q.to_instance(None).to_json();
        assert_eq!(raw["graph"], "instance");
        assert!(!raw["graph"].is_null());
    }

    #[test]
    fn explicit_instance_graph_is_preserved() {
        use terminusdb_schema::GraphType;
        let q = Query::Triple(Triple {
            subject: NodeValue::Variable("S".to_string()),
            predicate: NodeValue::Node("p".to_string()),
            object: WoqlValue::Variable("O".to_string()),
            graph: Some(GraphType::Instance),
        });
        assert_eq!(q.to_woql_json()["graph"], "instance");
    }

    #[test]
    fn nested_triples_are_normalized() {
        // graph:null nested inside And/Select must also be fixed.
        let inner = Query::Triple(Triple {
            subject: NodeValue::Variable("S".to_string()),
            predicate: NodeValue::Node("p".to_string()),
            object: WoqlValue::Variable("O".to_string()),
            graph: None,
        });
        let q = Query::Select(crate::control::Select {
            variables: vec!["O".to_string()],
            query: Box::new(Query::And(crate::query::And { and: vec![inner] })),
        });
        let json = q.to_woql_json();
        assert_eq!(json["query"]["and"][0]["graph"], "instance");
    }

    #[test]
    fn null_path_optionals_are_omitted_and_pattern_flattens() {
        use crate::path::{PathPattern, PathPredicate, PathStar};
        use crate::query::Path;

        // `//`-style: star over an "any predicate" (predicate: None), no output.
        let q = Query::Path(Path {
            subject: WoqlValue::Node("X".to_string()),
            pattern: PathPattern::Star(PathStar {
                star: Box::new(PathPattern::Predicate(PathPredicate { predicate: None })),
            }),
            object: WoqlValue::Variable("O".to_string()),
            path: None,
        });
        let json = q.to_woql_json();

        // Optional `path` output is omitted, not sent as null.
        assert!(
            json.as_object().unwrap().get("path").is_none(),
            "null `path` must be omitted, got {json}"
        );
        // Abstract PathPattern flattens to the concrete class (no wrapper).
        assert_eq!(json["pattern"]["@type"], "PathStar");
        // An omitted `predicate` is the "any predicate" wildcard.
        let any = &json["pattern"]["star"];
        assert_eq!(any["@type"], "PathPredicate");
        assert!(
            any.as_object().unwrap().get("predicate").is_none(),
            "any-predicate must have no `predicate` key, got {any}"
        );
    }

    /// Control: `#[tdb(default)]` is what flips `None` from null to the default.
    /// An annotated `Option<GraphType>` field serializes `None` as its default;
    /// an un-annotated one still serializes `None` as null.
    #[test]
    fn tdb_default_attr_controls_none_serialization() {
        use terminusdb_schema::GraphType;
        use terminusdb_schema_derive::TerminusDBModel;

        #[derive(TerminusDBModel, Clone)]
        #[allow(dead_code)]
        struct GraphDefaultProbe {
            #[tdb(default)]
            with_default: Option<GraphType>,
            without_default: Option<GraphType>,
        }

        let probe = GraphDefaultProbe {
            with_default: None,
            without_default: None,
        };
        let json = probe.to_instance(None).to_json();
        assert_eq!(json["with_default"], "instance", "annotated None -> default");
        assert!(json["without_default"].is_null(), "un-annotated None -> null");
    }
}
