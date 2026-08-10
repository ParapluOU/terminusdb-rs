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
    use crate::collection::{Length, Sum};
    use crate::misc::{Collect, RangeMax, RangeMin, TripleCount};
    use crate::order::{GroupBy, Order};
    use crate::prelude::{DataValue, NodeValue, Query, Triple, True, When};
    use crate::query::And;
    use crate::value::Value as WoqlValue;
    use terminusdb_schema::{FromTDBInstance, ToJson, ToTDBInstance};

    #[test]
    fn when_uses_authoritative_prolog_wire_shape_and_domain_decoder() {
        let query = Query::If(When {
            query: Box::new(Query::True(True {})),
            consequent: Box::new(Query::True(True {})),
        });
        let json = query.to_woql_json();
        assert_eq!(json["@type"], "When");
        assert!(json.get("query").is_some());
        assert!(json.get("consequent").is_some());
        assert!(json.get("test").is_none());
        assert!(json.get("then").is_none());
        assert!(json.get("else").is_none());

        let decoded = Query::from_json(json).expect("domain decoder accepts Prolog When JSON");
        assert!(matches!(decoded, Query::If(_)));
    }

    #[test]
    fn order_by_uses_authoritative_prolog_wire_shape_and_domain_decoder() {
        // This is the shape accepted by json_woql.pl: OrderBy(ordering, query), with
        // OrderTemplate(variable, "asc"|"desc"). Query proofs consume the decoded
        // domain AST below; JSON-LD is never the proof statement.
        let decoded = Query::from_json(serde_json::json!({
            "@type": "OrderBy",
            "ordering": [
                {
                    "@type": "OrderTemplate",
                    "variable": "secondary",
                    "order": "desc"
                },
                {
                    "@type": "OrderTemplate",
                    "variable": "primary",
                    "order": "asc"
                }
            ],
            "query": { "@type": "True" }
        }))
        .expect("domain decoder accepts Prolog OrderBy JSON");
        let Query::OrderBy(order_by) = decoded else {
            panic!("expected decoded OrderBy domain AST");
        };
        assert_eq!(order_by.ordering.len(), 2);
        assert_eq!(order_by.ordering[0].variable, "secondary");
        assert_eq!(order_by.ordering[0].order, Order::Desc);
        assert_eq!(order_by.ordering[1].variable, "primary");
        assert_eq!(order_by.ordering[1].order, Order::Asc);
        assert!(matches!(*order_by.query, Query::True(_)));
    }

    #[test]
    fn start_limit_use_authoritative_prolog_wire_shape_and_domain_decoder() {
        let decoded = Query::from_json(serde_json::json!({
            "@type": "Limit",
            "limit": 2,
            "query": {
                "@type": "Start",
                "start": 1,
                "query": { "@type": "True" }
            }
        }))
        .expect("domain decoder accepts Prolog Limit(Start(Query)) JSON");
        let Query::Limit(limit) = decoded else {
            panic!("expected decoded Limit domain AST");
        };
        assert_eq!(limit.limit, 2);
        let Query::Start(start) = *limit.query else {
            panic!("expected nested Start domain AST");
        };
        assert_eq!(start.start, 1);
        assert!(matches!(*start.query, Query::True(_)));
    }

    #[test]
    fn triple_count_uses_domain_decoder_and_prolog_resource_shape() {
        let query = Query::TripleCount(TripleCount {
            resource: "admin/db/local/branch/main/instance".into(),
            count: DataValue::Variable("Count".into()),
        });
        let json = query.to_woql_json();
        assert_eq!(json["@type"], "TripleCount");
        assert_eq!(json["resource"], "admin/db/local/branch/main/instance");

        let decoded = Query::from_json(json)
            .expect("domain decoder accepts the leading Prolog TripleCount shape");
        let Query::TripleCount(decoded) = decoded else {
            panic!("expected decoded TripleCount domain AST");
        };
        assert_eq!(decoded.resource, "admin/db/local/branch/main/instance");
        assert_eq!(decoded.count, DataValue::Variable("Count".into()));
    }

    #[test]
    fn immediate_group_by_sum_uses_canonical_domain_decoder_shapes() {
        let query = Query::And(And {
            and: vec![
                Query::GroupBy(GroupBy {
                    template: WoqlValue::Variable("score".into()),
                    group_by: vec!["category".into()],
                    grouped_value: WoqlValue::Variable("scores".into()),
                    query: Box::new(Query::True(True {})),
                }),
                Query::Sum(Sum {
                    list: DataValue::Variable("scores".into()),
                    result: DataValue::Variable("total".into()),
                }),
            ],
        });
        let json = query.to_woql_json();
        assert_eq!(json["and"][0]["@type"], "GroupBy");
        assert_eq!(json["and"][0]["group_by"][0], "category");
        assert!(json["and"][0].get("grouped").is_some());
        assert_eq!(json["and"][1]["@type"], "Sum");
        assert!(json["and"][1].get("list").is_some());
        assert!(json["and"][1].get("result").is_some());

        let decoded = Query::from_json(json)
            .expect("domain decoder accepts canonical immediate GroupBy followed by Sum");
        assert_eq!(decoded, query);

        let Query::And(mut global) = query else {
            unreachable!()
        };
        let Query::GroupBy(group) = &mut global.and[0] else {
            unreachable!()
        };
        group.group_by.clear();
        let global = Query::And(global);
        let json = global.to_woql_json();
        assert_eq!(json["and"][0]["group_by"], serde_json::json!([]));
        assert_eq!(
            Query::from_json(json).expect("domain decoder accepts zero-key GroupBy"),
            global,
        );
    }

    #[test]
    fn immediate_group_by_extrema_preserve_zero_and_two_declared_keys() {
        for keys in [Vec::new(), vec!["right".into(), "left".into()]] {
            let query = Query::And(And {
                and: vec![
                    Query::GroupBy(GroupBy {
                        template: WoqlValue::Variable("score".into()),
                        group_by: keys.clone(),
                        grouped_value: WoqlValue::Variable("scores".into()),
                        query: Box::new(Query::True(True {})),
                    }),
                    Query::RangeMin(RangeMin {
                        list: DataValue::Variable("scores".into()),
                        result: DataValue::Variable("extremum".into()),
                    }),
                ],
            });
            let json = query.to_woql_json();
            assert_eq!(json["and"][0]["group_by"], serde_json::json!(keys));
            assert_eq!(json["and"][1]["@type"], "RangeMin");
            assert_eq!(
                Query::from_json(json)
                    .expect("domain decoder accepts bounded immediate grouped extrema"),
                query,
            );
        }
    }

    #[test]
    fn immediate_group_by_length_uses_canonical_domain_decoder_shapes() {
        let query = Query::And(And {
            and: vec![
                Query::GroupBy(GroupBy {
                    template: WoqlValue::Variable("score".into()),
                    group_by: vec!["right".into(), "left".into()],
                    grouped_value: WoqlValue::Variable("scores".into()),
                    query: Box::new(Query::True(True {})),
                }),
                Query::Length(Length {
                    list: DataValue::Variable("scores".into()),
                    length: DataValue::Variable("cardinality".into()),
                }),
            ],
        });
        let json = query.to_woql_json();
        assert_eq!(json["and"][1]["@type"], "Length");
        assert!(json["and"][1].get("list").is_some());
        assert!(json["and"][1].get("length").is_some());
        assert_eq!(
            Query::from_json(json)
                .expect("domain decoder accepts canonical immediate GroupBy followed by Length"),
            query,
        );
    }

    #[test]
    fn immediate_collect_length_uses_canonical_domain_decoder_shapes() {
        let query = Query::And(And {
            and: vec![
                Query::Collect(Collect {
                    template: WoqlValue::Variable("object".into()),
                    into: WoqlValue::Variable("objects".into()),
                    query: Box::new(Query::True(True {})),
                }),
                Query::Length(Length {
                    list: DataValue::Variable("objects".into()),
                    length: DataValue::Variable("cardinality".into()),
                }),
            ],
        });
        let json = query.to_woql_json();
        assert_eq!(json["and"][0]["@type"], "Collect");
        assert!(json["and"][0].get("template").is_some());
        assert!(json["and"][0].get("into").is_some());
        assert!(json["and"][0].get("query").is_some());
        assert_eq!(json["and"][1]["@type"], "Length");
        assert_eq!(
            Query::from_json(json)
                .expect("domain decoder accepts canonical immediate Collect followed by Length"),
            query,
        );
    }

    #[test]
    fn immediate_collect_sum_uses_canonical_domain_decoder_shapes() {
        let query = Query::And(And {
            and: vec![
                Query::Collect(Collect {
                    template: WoqlValue::Variable("score".into()),
                    into: WoqlValue::Variable("scores".into()),
                    query: Box::new(Query::True(True {})),
                }),
                Query::Sum(Sum {
                    list: DataValue::Variable("scores".into()),
                    result: DataValue::Variable("total".into()),
                }),
            ],
        });
        let json = query.to_woql_json();
        assert_eq!(json["and"][0]["@type"], "Collect");
        assert_eq!(json["and"][1]["@type"], "Sum");
        assert_eq!(
            Query::from_json(json)
                .expect("domain decoder accepts canonical immediate Collect followed by Sum"),
            query,
        );
    }

    #[test]
    fn immediate_collect_extrema_use_canonical_domain_decoder_shapes() {
        for range in [
            Query::RangeMin(RangeMin {
                list: DataValue::Variable("scores".into()),
                result: DataValue::Variable("extreme".into()),
            }),
            Query::RangeMax(RangeMax {
                list: DataValue::Variable("scores".into()),
                result: DataValue::Variable("extreme".into()),
            }),
        ] {
            let query = Query::And(And {
                and: vec![
                    Query::Collect(Collect {
                        template: WoqlValue::Variable("score".into()),
                        into: WoqlValue::Variable("scores".into()),
                        query: Box::new(Query::True(True {})),
                    }),
                    range,
                ],
            });
            let json = query.to_woql_json();
            assert_eq!(json["and"][0]["@type"], "Collect");
            assert!(matches!(
                json["and"][1]["@type"].as_str(),
                Some("RangeMin" | "RangeMax")
            ));
            assert!(json["and"][1].get("list").is_some());
            assert!(json["and"][1].get("result").is_some());
            assert_eq!(
                Query::from_json(json)
                    .expect("domain decoder accepts canonical immediate collected extrema"),
                query,
            );
        }
    }

    #[test]
    fn once_uses_authoritative_prolog_wire_shape_and_domain_decoder() {
        let decoded = Query::from_json(serde_json::json!({
            "@type": "Once",
            "query": { "@type": "True" }
        }))
        .expect("domain decoder accepts Prolog Once JSON");
        let Query::Once(once) = decoded else {
            panic!("expected decoded Once domain AST");
        };
        assert!(matches!(*once.query, Query::True(_)));
    }

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
