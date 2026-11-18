    use super::*;
    use terminusdb_woql2::compare::Gte;
    use terminusdb_woql2::misc::{Collect, RangeMax, RangeMin};
    use terminusdb_woql2::query::And;

    fn exact_extrema(max: bool) -> Query {
        let child = Query::And(And {
            and: vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".into()),
                    predicate: NodeValue::Node("http://ex/age".into()),
                    object: Value::Variable("Age".into()),
                    graph: None,
                }),
                Query::Gte(Gte {
                    left: DataValue::Variable("Age".into()),
                    right: DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
                }),
            ],
        });
        let collect = Query::Collect(Collect {
            template: Value::Variable("Age".into()),
            into: Value::Variable("Ages".into()),
            query: Box::new(child),
        });
        let range = if max {
            Query::RangeMax(RangeMax {
                list: DataValue::Variable("Ages".into()),
                result: DataValue::Variable("Extreme".into()),
            })
        } else {
            Query::RangeMin(RangeMin {
                list: DataValue::Variable("Ages".into()),
                result: DataValue::Variable("Extreme".into()),
            })
        };
        Query::And(And {
            and: vec![collect, range],
        })
    }

    #[test]
    fn immediate_collect_extrema_compiles_to_one_exact_store_id() {
        for (max, expected_kind) in [
            (false, GroupExtremaKind::Min),
            (true, GroupExtremaKind::Max),
        ] {
            let compiled = plan_bgp(&exact_extrema(max)).unwrap();
            assert_eq!(compiled.relation.schema, ["Extreme"]);
            assert_eq!(
                compiled.relation.domains,
                [Some(EqualityValueDomain::Datatype(CanonicalDatatype::Int64))]
            );
            match compiled.relation.statement {
                CompiledRelationStatement::CollectionExtrema {
                    value_column,
                    datatype,
                    kind,
                    ..
                } => {
                    assert_eq!(value_column, 1);
                    assert_eq!(datatype, CanonicalDatatype::Int64);
                    assert_eq!(kind, expected_kind);
                }
                _ => panic!("expected collection-extrema statement"),
            }
        }
    }

    #[test]
    fn reversed_mismatched_and_unbound_collection_extrema_reject_closed() {
        let mut reversed = exact_extrema(false);
        if let Query::And(and) = &mut reversed {
            and.and.swap(0, 1);
        }
        assert!(matches!(
            plan_bgp(&reversed),
            Err(BgpPlanError::CollectionExtremaUnsupportedShape)
        ));

        let mut mismatched = exact_extrema(false);
        if let Query::And(and) = &mut mismatched {
            if let Query::RangeMin(range) = &mut and.and[1] {
                range.list = DataValue::Variable("Other".into());
            }
        }
        assert!(matches!(
            plan_bgp(&mismatched),
            Err(BgpPlanError::CollectionExtremaUnsupportedShape)
        ));

        let mut unbound = exact_extrema(false);
        if let Query::And(and) = &mut unbound {
            if let Query::Collect(collect) = &mut and.and[0] {
                collect.template = Value::Variable("Missing".into());
            }
        }
        assert!(matches!(
            plan_bgp(&unbound),
            Err(BgpPlanError::CollectionExtremaUnsupportedValue)
        ));
    }

    #[test]
    fn floating_collection_extrema_rejects_until_exact_semantics_are_pinned() {
        let mut query = exact_extrema(false);
        if let Query::And(and) = &mut query {
            if let Query::Collect(collect) = &mut and.and[0] {
                if let Query::And(child) = collect.query.as_mut() {
                    if let Query::Gte(bound) = &mut child.and[1] {
                        bound.right = DataValue::Data(XSDAnySimpleType::Float(0.0));
                    }
                }
            }
        }
        assert!(matches!(
            plan_bgp(&query),
            Err(BgpPlanError::CollectionExtremaUnsupportedValue)
        ));
    }
