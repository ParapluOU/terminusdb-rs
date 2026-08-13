    use super::*;
    use crate::compare::Gte;
    use crate::misc::{RangeMax, RangeMin};
    use crate::order::GroupBy;
    use crate::query::And;

    fn exact_min() -> Query {
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
                    right: DataValue::Data(XSDAnySimpleType::UnsignedInt(0)),
                }),
            ],
        });
        Query::And(And {
            and: vec![
                Query::GroupBy(GroupBy {
                    template: Value::Variable("Age".into()),
                    group_by: vec!["Person".into()],
                    grouped_value: Value::Variable("Ages".into()),
                    query: Box::new(child),
                }),
                Query::RangeMin(RangeMin {
                    list: DataValue::Variable("Ages".into()),
                    result: DataValue::Variable("Youngest".into()),
                }),
            ],
        })
    }

    fn two_key_max() -> Query {
        let child = Query::And(And {
            and: vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Left".into()),
                    predicate: NodeValue::Node("http://ex/link".into()),
                    object: Value::Variable("Right".into()),
                    graph: None,
                }),
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Right".into()),
                    predicate: NodeValue::Node("http://ex/score".into()),
                    object: Value::Variable("Score".into()),
                    graph: None,
                }),
                Query::Gte(Gte {
                    left: DataValue::Variable("Score".into()),
                    right: DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
                }),
            ],
        });
        Query::And(And {
            and: vec![
                Query::GroupBy(GroupBy {
                    template: Value::Variable("Score".into()),
                    group_by: vec!["Right".into(), "Left".into()],
                    grouped_value: Value::Variable("Scores".into()),
                    query: Box::new(child),
                }),
                Query::RangeMax(RangeMax {
                    list: DataValue::Variable("Scores".into()),
                    result: DataValue::Variable("Best".into()),
                }),
            ],
        })
    }

    #[test]
    fn exact_scalar_group_min_is_compiled_without_exposing_hidden_list() {
        let compiled = plan_bgp(&exact_min()).unwrap();
        assert_eq!(compiled.relation.schema, ["Person", "Youngest"]);
        match compiled.relation.statement {
            CompiledRelationStatement::GroupExtrema {
                key_columns,
                value_column,
                datatype,
                kind,
                ..
            } => {
                assert_eq!(key_columns, [0]);
                assert_ne!(key_columns[0], value_column);
                assert_eq!(datatype, CanonicalDatatype::UInt32);
                assert_eq!(kind, GroupExtremaKind::Min);
            }
            _ => panic!("expected group-extrema statement"),
        }
    }

    #[test]
    fn composite_key_or_unconsumed_hidden_list_rejects() {
        let mut composite = exact_min();
        if let Query::And(and) = &mut composite {
            if let Query::GroupBy(group) = &mut and.and[0] {
                group.group_by.push("Age".into());
            }
        }
        assert!(matches!(
            plan_bgp(&composite),
            Err(BgpPlanError::GroupExtremaUnsupportedShape)
        ));
        assert!(matches!(
            plan_bgp(&Query::GroupBy(GroupBy {
                template: Value::Variable("Age".into()),
                group_by: vec!["Person".into()],
                grouped_value: Value::Variable("Ages".into()),
                query: Box::new(exact_min()),
            })),
            Err(BgpPlanError::UnsupportedConstruct { .. })
        ));
    }


    #[test]
    fn zero_and_two_node_key_extrema_preserve_declared_priority() {
        let mut zero = exact_min();
        if let Query::And(and) = &mut zero {
            if let Query::GroupBy(group) = &mut and.and[0] {
                group.group_by.clear();
            }
        }
        let compiled = plan_bgp(&zero).unwrap();
        assert_eq!(compiled.relation.schema, ["Youngest"]);
        assert!(matches!(
            compiled.relation.statement,
            CompiledRelationStatement::GroupExtrema { ref key_columns, .. }
                if key_columns.is_empty()
        ));

        let compiled = plan_bgp(&two_key_max()).unwrap();
        assert_eq!(compiled.relation.schema, ["Right", "Left", "Best"]);
        assert!(matches!(
            compiled.relation.statement,
            CompiledRelationStatement::GroupExtrema { ref key_columns, kind: GroupExtremaKind::Max, .. }
                if key_columns == &[1, 0]
        ));
    }

    #[test]
    fn reversed_float_and_numeric_group_keys_reject_closed() {
        let mut reversed = exact_min();
        if let Query::And(and) = &mut reversed {
            and.and.swap(0, 1);
        }
        assert!(matches!(
            plan_bgp(&reversed),
            Err(BgpPlanError::GroupExtremaUnsupportedShape)
        ));

        let mut floating = exact_min();
        if let Query::And(and) = &mut floating {
            if let Query::GroupBy(group) = &mut and.and[0] {
                if let Query::And(child) = group.query.as_mut() {
                    if let Query::Gte(bound) = &mut child.and[1] {
                        bound.right = DataValue::Data(XSDAnySimpleType::Float(0.0));
                    }
                }
            }
        }
        assert!(matches!(
            plan_bgp(&floating),
            Err(BgpPlanError::GroupExtremaUnsupportedValue)
        ));

        let mut numeric_key = exact_min();
        if let Query::And(and) = &mut numeric_key {
            if let Query::GroupBy(group) = &mut and.and[0] {
                group.group_by = vec!["Weight".into()];
                if let Query::And(child) = group.query.as_mut() {
                    child.and.push(Query::Triple(Triple {
                        subject: NodeValue::Variable("Person".into()),
                        predicate: NodeValue::Node("http://ex/weight".into()),
                        object: Value::Variable("Weight".into()),
                        graph: None,
                    }));
                    child.and.push(Query::Gte(Gte {
                        left: DataValue::Variable("Weight".into()),
                        right: DataValue::Data(XSDAnySimpleType::UnsignedInt(0)),
                    }));
                }
            }
        }
        assert!(matches!(
            plan_bgp(&numeric_key),
            Err(BgpPlanError::GroupExtremaUnsupportedShape)
        ));
    }
