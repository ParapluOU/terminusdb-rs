//! Verifier-recompute helpers — the "expected columns / objects" the verifier rederives.
//!
//! Many WOQL constructs are proven by the "verifier recompute" strategy: instead of a
//! bespoke cryptographic gadget, the verifier RE-DERIVES the expected result column from the
//! root-authenticated input columns and checks it matches. This submodule holds those pure
//! recomputation routines — the `expected_*_columns` / `expected_*_objects` functions for
//! Eval arithmetic, typecasts, string ops (concat / join / substring / pad / hashkey),
//! group and collection aggregates (sum / count / min / max), calendar and date functions,
//! and the exact decimal / rational value layer — plus the [`CompiledRelation`] → plan /
//! descriptor shaping (`relation_result_descriptors`, `relation_proof_plan`) that drives
//! them. Pure arithmetic and encoding; no cryptography lives here.
//!
//! "Verifier recompute" is why these routines can be pure arithmetic: they carry no prover
//! witness, so nothing here needs a bespoke cryptographic gadget. The verifier re-derives the
//! expected column/objects from the ROOT-AUTHENTICATED input columns and requires an exact
//! match — a deterministic function of pinned inputs leaves a malicious prover no latitude. See
//! [`terminus_store::proof::primer`] §6 (gadgets) and §11 (soundness vs fidelity) for how this
//! sits under the store's cryptography.

use super::*;

/// The per-output-column [`ResultColumnDescriptor`] (id namespace + nullability) a
/// [`CompiledRelation`] delivers, computed structurally from its statement. Drives result
/// decoding and resolution; folded into the query digest so the delivered column shape is bound.
pub(crate) fn relation_result_descriptors(
    relation: &CompiledRelation,
) -> std::io::Result<Vec<ResultColumnDescriptor>> {
    match &relation.statement {
        CompiledRelationStatement::LayerTripleCount => Ok(vec![ResultColumnDescriptor {
            namespace: ResultIdNamespace::GeneratedDecimalU64,
            nullable: false,
        }]),
        CompiledRelationStatement::GeneratedU64Filter { input, .. }
        | CompiledRelationStatement::GeneratedI64Filter { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::Bgp(plan) => {
            terminus_store::proof::query::result_column_descriptors(plan)
        }
        CompiledRelationStatement::TransitiveClosure { .. } => {
            // The reachable set is one nonnullable node column.
            Ok(vec![ResultColumnDescriptor {
                namespace: ResultIdNamespace::NodeValue,
                nullable: false,
            }])
        }
        // The all-pairs reachability relation is two nonnullable node columns [src, dst] — identical
        // delivered shape whether the edge is a single predicate or a DERIVED wildcard/compound edge
        // (only the edge differs, not the result).
        CompiledRelationStatement::TransitiveClosurePairs { .. }
        | CompiledRelationStatement::TransitiveClosurePairsCompound { .. } => {
            Ok(vec![
                ResultColumnDescriptor {
                    namespace: ResultIdNamespace::NodeValue,
                    nullable: false,
                },
                ResultColumnDescriptor {
                    namespace: ResultIdNamespace::NodeValue,
                    nullable: false,
                },
            ])
        }
        CompiledRelationStatement::TransitiveClosureCompound { .. } => {
            // Identical delivered shape to the single-seed closure: the reachable set is one
            // nonnullable node column (only the compound EDGE differs, not the result).
            Ok(vec![ResultColumnDescriptor {
                namespace: ResultIdNamespace::NodeValue,
                nullable: false,
            }])
        }
        CompiledRelationStatement::OrderedListWalk { .. }
        // The ordered array element sequence is likewise one nonnullable node/value column (the
        // value-in-index-order column — identical delivered shape to the list walk).
        | CompiledRelationStatement::OrderedArrayWalk { .. } => {
            // The ordered element sequence is one nonnullable node/value column (identical shape to
            // the closure's reachable set — one node column, resolved through the dictionary).
            Ok(vec![ResultColumnDescriptor {
                namespace: ResultIdNamespace::NodeValue,
                nullable: false,
            }])
        }
        CompiledRelationStatement::BagUnion { branches, mappings } => {
            let mut expected = None;
            for (branch, mapping) in branches.iter().zip(mappings) {
                let descriptors = relation_result_descriptors(branch)?;
                let mapped = mapping
                    .iter()
                    .map(|index| {
                        descriptors.get(*index).copied().ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "WOQL union schema mapping out of range",
                            )
                        })
                    })
                    .collect::<std::io::Result<Vec<_>>>()?;
                if expected.as_ref().is_some_and(|prior| prior != &mapped) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL union branch descriptor mismatch",
                    ));
                }
                expected = Some(mapped);
            }
            Ok(expected.unwrap_or_default())
        }
        CompiledRelationStatement::BagProjection { input, indices } => {
            let descriptors = relation_result_descriptors(input)?;
            indices
                .iter()
                .map(|index| {
                    descriptors.get(*index).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL bag projection index out of range",
                        )
                    })
                })
                .collect()
        }
        CompiledRelationStatement::Distinct { input, key_columns } => {
            let descriptors = relation_result_descriptors(input)?;
            key_columns
                .iter()
                .map(|index| {
                    descriptors.get(*index).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL distinct key column out of range",
                        )
                    })
                })
                .collect()
        }
        CompiledRelationStatement::When { expanded } => relation_result_descriptors(expanded),
        CompiledRelationStatement::LeftAnti { left, .. } => relation_result_descriptors(left),
        CompiledRelationStatement::InnerJoin { left, right, key_pairs } => {
            let mut descriptors = relation_result_descriptors(left)?;
            let right_descriptors = relation_result_descriptors(right)?;
            let shared_right: Vec<_> = key_pairs.iter().map(|pair| pair.1).collect();
            descriptors.extend(right_descriptors.into_iter().enumerate().filter_map(
                |(index, descriptor)| (!shared_right.contains(&index)).then_some(descriptor)
            ));
            Ok(descriptors)
        }
        CompiledRelationStatement::CrossRootHashJoin { right, right_key_col, .. } => {
            // Output = the INSTANCE-side (`right`) columns minus the class key column.
            let right_descriptors = relation_result_descriptors(right)?;
            Ok(right_descriptors
                .into_iter()
                .enumerate()
                .filter_map(|(index, descriptor)| (index != *right_key_col).then_some(descriptor))
                .collect())
        }
        CompiledRelationStatement::LeftOuter {
            left,
            right,
            key_pairs,
        } => {
            let mut descriptors = relation_result_descriptors(left)?;
            let right_descriptors = relation_result_descriptors(right)?;
            let shared_right: Vec<_> = key_pairs.iter().map(|pair| pair.1).collect();
            descriptors.extend(right_descriptors.into_iter().enumerate().filter_map(
                |(index, descriptor)| {
                    (!shared_right.contains(&index)).then_some(ResultColumnDescriptor {
                        namespace: descriptor.namespace,
                        nullable: true,
                    })
                },
            ));
            Ok(descriptors)
        }
        CompiledRelationStatement::Equality { input, .. } => relation_result_descriptors(input),
        CompiledRelationStatement::TupleDisequality { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::ExactScalarOrder { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::ExactMultiOrder { input, .. } => relation_result_descriptors(input),
        CompiledRelationStatement::ExactOrderedSlice { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::GroupExtrema {
            input, key_columns, ..
        } => {
            let child = relation_result_descriptors(input)?;
            let mut result = key_columns.iter().map(|key_column| {
                child.get(*key_column).copied().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "group-extrema key column out of range",
                    )
                })
            }).collect::<std::io::Result<Vec<_>>>()?;
            result.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::NodeValue,
                nullable: false,
            });
            Ok(result)
        }
        CompiledRelationStatement::GroupSum {
            input, key_columns, datatype, ..
        } => {
            let child = relation_result_descriptors(input)?;
            let generated = ResultColumnDescriptor {
                namespace: if matches!(
                    datatype,
                    CanonicalDatatype::Int8
                        | CanonicalDatatype::Int16
                        | CanonicalDatatype::Int32
                        | CanonicalDatatype::Int64
                ) {
                    ResultIdNamespace::GeneratedDecimalI64
                } else {
                    ResultIdNamespace::GeneratedDecimalU64
                },
                nullable: false,
            };
            let mut result = key_columns
                .iter()
                .map(|key_column| {
                    child.get(*key_column).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "group-sum key column out of range",
                        )
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?;
            result.push(generated);
            Ok(result)
        }
        CompiledRelationStatement::GroupCount { input, key_columns } => {
            let child = relation_result_descriptors(input)?;
            let mut result = key_columns
                .iter()
                .map(|key_column| {
                    child.get(*key_column).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "group-count key column out of range",
                        )
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?;
            result.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::GeneratedDecimalU64,
                nullable: false,
            });
            Ok(result)
        }
        CompiledRelationStatement::CollectionCount { .. } => Ok(vec![ResultColumnDescriptor {
            namespace: ResultIdNamespace::GeneratedDecimalU64,
            nullable: false,
        }]),
        CompiledRelationStatement::CollectionSum { datatype, .. } => {
            Ok(vec![ResultColumnDescriptor {
                namespace: if matches!(
                    datatype,
                    CanonicalDatatype::Int8
                        | CanonicalDatatype::Int16
                        | CanonicalDatatype::Int32
                        | CanonicalDatatype::Int64
                ) {
                    ResultIdNamespace::GeneratedDecimalI64
                } else {
                    ResultIdNamespace::GeneratedDecimalU64
                },
                nullable: false,
            }])
        }
        CompiledRelationStatement::CollectionExtrema {
            input, value_column, ..
        } => {
            let child = relation_result_descriptors(input)?;
            Ok(vec![*child.get(*value_column).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "collection-extrema value column out of range",
                )
            })?])
        }
        CompiledRelationStatement::Eval { input, .. } => {
            // Every child column is carried through unchanged; the appended derived column
            // is a signed generated integer (Eval accumulates in signed i128 and narrows to
            // the i64 generated domain, so `Minus` results and negatives are representable).
            let mut descriptors = relation_result_descriptors(input)?;
            descriptors.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::GeneratedDecimalI64,
                nullable: false,
            });
            Ok(descriptors)
        }
        CompiledRelationStatement::FloorEval { input, .. } => {
            // Identical shape to `Eval`: child columns pass through, and `floor(x)` appends ONE
            // signed generated-integer column (a floored rational is integral; negatives are
            // representable in the i64 generated domain).
            let mut descriptors = relation_result_descriptors(input)?;
            descriptors.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::GeneratedDecimalI64,
                nullable: false,
            });
            Ok(descriptors)
        }
        CompiledRelationStatement::Typecast {
            input,
            target_datatype,
            ..
        } => {
            // Child columns pass through; the appended cast column uses the SIGNED generated
            // namespace for signed targets and the UNSIGNED one for unsigned targets (an
            // unsigned target like xsd:unsignedLong can hold values above `i64::MAX`).
            let mut descriptors = relation_result_descriptors(input)?;
            descriptors.push(ResultColumnDescriptor {
                namespace: integer_target_namespace(*target_datatype)?,
                nullable: false,
            });
            Ok(descriptors)
        }
        CompiledRelationStatement::Concat { input, .. } => {
            // The committed result columns are exactly the child's operand columns; the concat
            // result is TERMINAL/revealed and is NOT a committed column (a synthesized string
            // does not encode into a field element, and nothing downstream consumes it). So the
            // descriptors are the child's, unchanged — keeping schema/descriptor/column shapes
            // aligned for the generic resolve path.
            relation_result_descriptors(input)
        }
        // Join/Substring, like Concat, carry the child's committed columns through VERBATIM (their
        // synthesized result is terminal/revealed, not a committed column) — descriptors are the
        // child's, unchanged.
        CompiledRelationStatement::Join { input, .. }
        | CompiledRelationStatement::Substring { input, .. }
        // Pad/HashKey, like Concat, carry the child's committed columns through VERBATIM (their
        // synthesized result is terminal/revealed, not a committed column).
        | CompiledRelationStatement::Pad { input, .. }
        | CompiledRelationStatement::HashKey { input, .. }
        // RationalArith, like Concat, carries the child's committed columns through VERBATIM — its
        // synthesized xsd:decimal result is terminal/revealed, not a committed column.
        | CompiledRelationStatement::RationalArith { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::CalendarInt { input, outputs, .. } => {
            // Child columns pass through; ONE signed generated-integer column is appended PER output
            // (weekday/iso-week values are small positive integers, but the signed domain matches
            // Eval's and keeps the descriptor family uniform).
            let mut descriptors = relation_result_descriptors(input)?;
            for _ in outputs {
                descriptors.push(ResultColumnDescriptor {
                    namespace: ResultIdNamespace::GeneratedDecimalI64,
                    nullable: false,
                });
            }
            Ok(descriptors)
        }
        // CalendarDate, like Concat/RationalArith, carries the child's committed columns through
        // VERBATIM — its synthesized xsd:date result is terminal/revealed, not a committed column.
        CompiledRelationStatement::CalendarDate { input, .. }
        // TypecastNumberToString/TypecastStringToNumber, like Concat/RationalArith, carry the child's
        // committed columns through VERBATIM — the cast result is terminal/revealed, not committed.
        | CompiledRelationStatement::TypecastNumberToString { input, .. }
        | CompiledRelationStatement::TypecastStringToNumber { input, .. } => {
            relation_result_descriptors(input)
        }
    }
}

/// Lower a [`CompiledRelation`] (with its precomputed `query_digest`) into the store
/// [`RelationProofPlan`] the prover proves and the verifier re-checks. Recurses into child
/// relations (each with its own digest). The verifier-recompute-only statements (the value/eval/
/// string/calendar forms) have no store plan node and are handled by the query wrapper, not here.
pub(crate) fn relation_proof_plan(
    relation: &CompiledRelation,
    query_digest: ProofHash,
) -> std::io::Result<RelationProofPlan> {
    let child = |relation: &CompiledRelation| {
        let digest = relation_query_digest(relation)?;
        relation_proof_plan(relation, digest)
    };
    let kind = match &relation.statement {
        CompiledRelationStatement::LayerTripleCount => RelationProofPlanKind::LayerTripleCount,
        CompiledRelationStatement::GeneratedU64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => RelationProofPlanKind::GeneratedU64Filter {
            input: Box::new(child(input)?),
            column: *column,
            op: *op,
            constant: *constant,
            constant_on_left: *constant_on_left,
        },
        CompiledRelationStatement::GeneratedI64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => RelationProofPlanKind::GeneratedI64Filter {
            input: Box::new(child(input)?),
            column: *column,
            op: *op,
            constant: *constant,
            constant_on_left: *constant_on_left,
        },
        CompiledRelationStatement::Bgp(plan) => RelationProofPlanKind::Bgp(plan.clone()),
        CompiledRelationStatement::BagUnion { branches, mappings } => {
            RelationProofPlanKind::BagUnion {
                branches: branches.iter().map(child).collect::<std::io::Result<_>>()?,
                mappings: mappings.clone(),
            }
        }
        CompiledRelationStatement::BagProjection { input, indices } => {
            RelationProofPlanKind::BagProjection {
                input: Box::new(child(input)?),
                indices: indices.clone(),
            }
        }
        CompiledRelationStatement::Distinct { input, key_columns } => {
            RelationProofPlanKind::Distinct {
                input: Box::new(child(input)?),
                key_columns: key_columns.clone(),
            }
        }
        CompiledRelationStatement::When { expanded } => {
            return relation_proof_plan(expanded, query_digest);
        }
        CompiledRelationStatement::LeftAnti {
            left,
            right,
            key_pairs,
        } => RelationProofPlanKind::LeftAnti {
            left: Box::new(child(left)?),
            right: Box::new(child(right)?),
            key_pairs: key_pairs.clone(),
        },
        CompiledRelationStatement::InnerJoin { left, right, key_pairs } => {
            RelationProofPlanKind::InnerJoin {
                left: Box::new(child(left)?), right: Box::new(child(right)?),
                key_pairs: key_pairs.clone(),
            }
        }
        CompiledRelationStatement::CrossRootHashJoin { left, right, left_key_col, right_key_col } => {
            RelationProofPlanKind::CrossRootHashJoin {
                left: Box::new(child(left)?),
                right: Box::new(child(right)?),
                left_key_col: *left_key_col,
                right_key_col: *right_key_col,
            }
        }
        CompiledRelationStatement::Equality { input, constraint } => {
            RelationProofPlanKind::Equality {
                input: Box::new(child(input)?),
                constraint: constraint.clone(),
            }
        }
        CompiledRelationStatement::TupleDisequality { input, pairs } => {
            RelationProofPlanKind::ExactTupleDisequality {
                input: Box::new(child(input)?),
                pairs: pairs.clone(),
            }
        }
        CompiledRelationStatement::ExactScalarOrder { input, datatype, direction } => {
            RelationProofPlanKind::ExactScalarOrder {
                input: Box::new(child(input)?),
                datatype: *datatype,
                direction: *direction,
            }
        }
        CompiledRelationStatement::ExactMultiOrder { input, key_indices, datatypes, directions } => {
            RelationProofPlanKind::ExactMultiOrder { input: Box::new(child(input)?), datatypes: datatypes.clone(), key_indices: key_indices.clone(), directions: directions.clone() }
        }
        CompiledRelationStatement::ExactOrderedSlice { input, offset, limit, .. } => {
            RelationProofPlanKind::ExactOrderedSlice {
                input: Box::new(child(input)?),
                offset: *offset,
                limit: *limit,
            }
        }
        CompiledRelationStatement::LeftOuter {
            left,
            right,
            key_pairs,
        } => RelationProofPlanKind::LeftOuter {
            left: Box::new(child(left)?),
            right: Box::new(child(right)?),
            key_pairs: key_pairs.clone(),
        },
        CompiledRelationStatement::GroupExtrema {
            input,
            key_columns,
            value_column,
            datatype,
            kind,
        } => RelationProofPlanKind::GroupExtrema {
            input: Box::new(child(input)?),
            key_columns: key_columns.clone(),
            value_column: *value_column,
            datatype: *datatype,
            kind: *kind,
        },
        CompiledRelationStatement::TransitiveClosure {
            edge,
            seed,
            reflexive,
            inverted,
            // The graph selector is NOT carried into the store's `RelationProofPlanKind`: the store
            // recursion is graph-blind and reads whichever single `LayerCommitment` (instance or schema)
            // the prove/verify ENTRY selects for this query (see `relation_graph`). The selector's
            // soundness is anchored by (a) its fold into `relation_query_digest` above and (b) the
            // verifier picking the matching commitment + `commitment_root` from the trusted query.
            graph: _,
        } => RelationProofPlanKind::TransitiveClosure {
            // The store closure edge is now a general `RelationProofPlan`; a single-predicate edge is a
            // `Bgp` leaf carrying the closure's OWN query digest as its identity tag — byte-identical to
            // the previous store-side `closure_edge_plan(edge, closure_digest)` wrapping, so every
            // existing single-predicate closure proof/digest is unchanged.
            edge: Box::new(RelationProofPlan {
                query_digest,
                kind: RelationProofPlanKind::Bgp(edge.clone()),
            }),
            seed: seed.clone(),
            reflexive: *reflexive,
            inverted: *inverted,
        },
        CompiledRelationStatement::TransitiveClosureCompound {
            edge,
            seed,
            reflexive,
            inverted,
            // The graph selector is NOT carried into the store plan kind (identical to the single-seed
            // closure): the store recursion is graph-blind, and the selector's soundness is anchored by
            // its digest fold (above) + the verifier picking the matching commitment root.
            graph: _,
        } => RelationProofPlanKind::TransitiveClosure {
            // The compound edge is a general committed relation (deduped bag-union / projected join),
            // lowered like any other child so it carries its own per-node digest. The store closure
            // driver takes it BY COMMITMENT — the SAME `TransitiveClosure` node, only `E` differs.
            edge: Box::new(child(edge)?),
            seed: seed.clone(),
            reflexive: *reflexive,
            inverted: *inverted,
        },
        CompiledRelationStatement::TransitiveClosurePairs {
            edge,
            inverted,
            // The graph selector is NOT carried into the store plan kind (identical to the single-seed
            // closure above): the store recursion is graph-blind and reads whichever single
            // `LayerCommitment` the prove/verify ENTRY selects (see `relation_graph`). Its soundness
            // is anchored by (a) the digest fold in `relation_query_digest` and (b) the verifier picking
            // the matching commitment + `commitment_root` from the trusted query.
            graph: _,
        } => RelationProofPlanKind::TransitiveClosurePairs {
            // The store pairs edge is now a general `RelationProofPlan`; a single-predicate edge is a
            // `Bgp` leaf carrying the closure's OWN query digest as its identity tag — byte-identical to
            // the previous store-side `closure_edge_plan(edge, closure_digest)` wrapping, so every
            // existing all-pairs single-predicate closure proof/digest is unchanged.
            edge: Box::new(RelationProofPlan {
                query_digest,
                kind: RelationProofPlanKind::Bgp(edge.clone()),
            }),
            inverted: *inverted,
        },
        CompiledRelationStatement::TransitiveClosurePairsCompound {
            edge,
            inverted,
            // The graph selector is NOT carried into the store plan kind (identical to the compound
            // single-seed closure): the store recursion is graph-blind, and the selector's soundness is
            // anchored by its digest fold (above) + the verifier picking the matching commitment root.
            graph: _,
        } => RelationProofPlanKind::TransitiveClosurePairs {
            // The DERIVED all-pairs edge is a general committed relation (a wildcard full-triple
            // projection / a deduped bag-union / a projected join), lowered like any other child so it
            // carries its own per-node digest. The store pairs driver takes it BY COMMITMENT — the SAME
            // `TransitiveClosurePairs` node, only `E` differs.
            edge: Box::new(child(edge)?),
            inverted: *inverted,
        },
        CompiledRelationStatement::OrderedListWalk {
            subject,
            predicate,
            // The graph selector is NOT carried into the store's plan kind (exactly as the closure
            // above drops it): the store recursion is graph-blind and reads whichever single
            // `LayerCommitment` the prove/verify ENTRY selects (see `relation_graph`). Its
            // soundness is anchored by (a) the digest fold in `relation_query_digest` and (b) the
            // verifier picking the matching commitment + `commitment_root` from the trusted query.
            // (OrderedListWalk is always `Instance` in this increment.)
            graph: _,
        } => RelationProofPlanKind::OrderedListWalk {
            subject: subject.clone(),
            predicate: predicate.clone(),
        },
        CompiledRelationStatement::OrderedArrayWalk {
            subject,
            predicate,
            // The graph selector is NOT carried into the store's plan kind (exactly as the list walk
            // above drops it): the store recursion is graph-blind and reads whichever single
            // `LayerCommitment` the prove/verify ENTRY selects (see `relation_graph`). Its soundness
            // is anchored by (a) the digest fold in `relation_query_digest` and (b) the verifier picking
            // the matching commitment + `commitment_root` from the trusted query. (OrderedArrayWalk is
            // always `Instance` in this increment.)
            graph: _,
        } => RelationProofPlanKind::OrderedArrayWalk {
            subject: subject.clone(),
            predicate: predicate.clone(),
        },
        CompiledRelationStatement::GroupSum {
            input,
            key_columns,
            value_column,
            datatype,
        } => RelationProofPlanKind::GroupSum {
            input: Box::new(child(input)?),
            key_columns: key_columns.clone(),
            value_column: *value_column,
            datatype: *datatype,
        },
        CompiledRelationStatement::GroupCount { input, key_columns } => {
            RelationProofPlanKind::GroupCount {
                input: Box::new(child(input)?),
                key_columns: key_columns.clone(),
            }
        }
        CompiledRelationStatement::CollectionCount { input } => {
            RelationProofPlanKind::CollectionCount {
                input: Box::new(child(input)?),
            }
        }
        CompiledRelationStatement::CollectionSum {
            input,
            template,
            datatype,
        } => RelationProofPlanKind::CollectionSum {
            input: Box::new(child(input)?),
            template: *template,
            datatype: *datatype,
        },
        CompiledRelationStatement::CollectionExtrema {
            input,
            value_column,
            datatype,
            kind,
        } => RelationProofPlanKind::CollectionExtrema {
            input: Box::new(child(input)?),
            value_column: *value_column,
            datatype: *datatype,
            kind: *kind,
        },
        // Eval is a woql2-side verifier-recompute layered OVER a Store-proven child; it has
        // no Store `RelationProofPlanKind`. `CompiledBgp::prove_with_context` intercepts a
        // top-level Eval and proves its child through the Store, so this arm is never reached
        // for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Eval { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Eval relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Typecast, like Eval, is a woql2-side verifier-recompute layered OVER a Store-proven
        // child; `CompiledBgp::prove_with_context` intercepts a top-level Typecast, so this arm
        // is never reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Typecast { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Typecast relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Concat, like Eval/Typecast, is a woql2-side verifier-recompute over a Store-proven
        // child; `CompiledBgp::prove_with_context` intercepts a top-level Concat, so this arm is
        // never reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Concat { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Concat relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Join/Substring are woql2-side verifier-recompute ops (like Concat): the top-level
        // `prove_with_context` intercepts them, so this recursive-planner arm is never reached for
        // a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Join { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Join relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::Substring { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Pad/HashKey are woql2-side verifier-recompute ops (like Concat): the top-level
        // `prove_with_context` intercepts them, so this recursive-planner arm is never reached for
        // a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Pad { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Pad relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::HashKey { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL HashKey relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // FloorEval/RationalArith are woql2-side verifier-recompute ops (like Eval/Concat): the
        // top-level `prove_with_context` intercepts them, so this recursive-planner arm is never
        // reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::FloorEval { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL floor relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::RationalArith { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL rational-arithmetic relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Calendar ops are woql2-side verifier-recompute layered OVER a Store-proven child; the
        // top-level `prove_with_context` intercepts them, so these recursive-planner arms are never
        // reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::CalendarInt { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar-integer relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Number↔string Typecasts are woql2-side verifier-recompute ops (like Concat): the top-level
        // `prove_with_context` intercepts them, so these recursive-planner arms are never reached for
        // a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::TypecastNumberToString { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL number→string Typecast relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::TypecastStringToNumber { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL string→number Typecast relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::CalendarDate { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar-date relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
    };
    Ok(RelationProofPlan { query_digest, kind })
}

/// Evaluate one generated-decimal comparison `value op constant` (or `constant op value` when
/// `constant_on_left`) — the per-row predicate the generated-column filters recompute.
pub(crate) fn generated_u64_comparison(
    value: u64,
    op: GeneratedU64ComparisonOp,
    constant: u64,
    constant_on_left: bool,
) -> bool {
    let (left, right) = if constant_on_left {
        (constant, value)
    } else {
        (value, constant)
    };
    match op {
        GeneratedU64ComparisonOp::Eq => left == right,
        GeneratedU64ComparisonOp::Lt => left < right,
        GeneratedU64ComparisonOp::Le => left <= right,
        GeneratedU64ComparisonOp::Gt => left > right,
        GeneratedU64ComparisonOp::Ge => left >= right,
    }
}

/// Recompute the columns a [`CompiledRelationStatement::GeneratedU64Filter`] delivers: keep the
/// child rows whose unsigned generated-decimal `column` passes `generated_u64_comparison`,
/// compacted stably. The verifier re-runs this over the child's authenticated columns.
pub(crate) fn expected_generated_u64_filter_columns(
    input: &ProvedBgp,
    column: usize,
    op: GeneratedU64ComparisonOp,
    constant: u64,
    constant_on_left: bool,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let source = input.result_columns.get(column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-u64 comparison column is out of range",
        )
    })?;
    if source.len() < input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-u64 comparison input column is short",
        ));
    }
    let mut keep = Vec::new();
    for (row, field) in source.iter().take(input.result_len).enumerate() {
        let value = generated_decimal_u64_value(*field)?;
        if generated_u64_comparison(value, op, constant, constant_on_left) {
            keep.push(row);
        }
    }
    input
        .result_columns
        .iter()
        .map(|input_column| {
            keep.iter()
                .map(|row| {
                    input_column.get(*row).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "generated-u64 comparison input column is short",
                        )
                    })
                })
                .collect()
        })
        .collect()
}

/// As `expected_generated_u64_filter_columns` but for a SIGNED generated-decimal `column`
/// compared against an `i64` `constant` (a [`CompiledRelationStatement::GeneratedI64Filter`]).
pub(crate) fn expected_generated_i64_filter_columns(
    input: &ProvedBgp,
    column: usize,
    op: GeneratedU64ComparisonOp,
    constant: i64,
    constant_on_left: bool,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let source = input.result_columns.get(column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-i64 comparison column is out of range",
        )
    })?;
    if source.len() < input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-i64 comparison input column is short",
        ));
    }
    let mut keep = Vec::new();
    for (row, field) in source.iter().take(input.result_len).enumerate() {
        let value = generated_decimal_i64_value(*field)?;
        let (left, right) = if constant_on_left {
            (constant, value)
        } else {
            (value, constant)
        };
        let selected = match op {
            GeneratedU64ComparisonOp::Eq => left == right,
            GeneratedU64ComparisonOp::Lt => left < right,
            GeneratedU64ComparisonOp::Le => left <= right,
            GeneratedU64ComparisonOp::Gt => left > right,
            GeneratedU64ComparisonOp::Ge => left >= right,
        };
        if selected {
            keep.push(row);
        }
    }
    input
        .result_columns
        .iter()
        .map(|input_column| {
            keep.iter()
                .map(|row| {
                    input_column.get(*row).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "generated-i64 comparison input column is short",
                        )
                    })
                })
                .collect()
        })
        .collect()
}

/// A per-row group-sum operand decoded into its exact signedness domain, so an unsigned family
/// (`xsd:nonNegativeInteger`/`unsignedLong`/…) accumulates in `u64` and a signed family in `i128`
/// without cross-domain coercion.
#[derive(Clone, Copy)]
pub(crate) enum GroupSumValue {
    /// An unsigned fixed-width integer operand.
    Unsigned(u64),
    /// A signed fixed-width integer operand.
    Signed(i128),
}

impl GroupSumValue {
    fn zero(datatype: CanonicalDatatype) -> std::io::Result<Self> {
        match datatype {
            CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64 => Ok(Self::Unsigned(0)),
            CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64 => Ok(Self::Signed(0)),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum datatype is outside the integral subset",
            )),
        }
    }

    fn checked_add(self, rhs: Self) -> std::io::Result<Self> {
        match (self, rhs) {
            (Self::Unsigned(left), Self::Unsigned(right)) => left
                .checked_add(right)
                .map(Self::Unsigned)
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum unsigned result exceeds u64",
                )),
            (Self::Signed(left), Self::Signed(right)) => left
                .checked_add(right)
                .map(Self::Signed)
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum signed accumulator exceeds i128",
                )),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum mixes signed and unsigned values",
            )),
        }
    }

    fn field(self) -> std::io::Result<ArkFr> {
        match self {
            Self::Unsigned(value) => Ok(ArkFr::from_u64(value)),
            Self::Signed(value) => Ok(generated_decimal_i64_field(i64::try_from(value).map_err(
                |_| std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum signed result exceeds i64",
                ),
            )?)),
        }
    }

    fn cmp_same_domain(self, rhs: Self) -> std::io::Result<std::cmp::Ordering> {
        match (self, rhs) {
            (Self::Unsigned(left), Self::Unsigned(right)) => Ok(left.cmp(&right)),
            (Self::Signed(left), Self::Signed(right)) => Ok(left.cmp(&right)),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "collection-extrema mixes signed and unsigned values",
            )),
        }
    }
}

/// Decode a root-authenticated result value into a `GroupSumValue` in the expected exact
/// fixed-width integral `datatype`'s signedness domain, failing closed on any other datatype
/// (so a sum never silently coerces a wrong-typed operand).
pub(crate) fn integral_group_sum_value(
    value: &CanonicalResultValue,
    datatype: CanonicalDatatype,
) -> std::io::Result<GroupSumValue> {
    let CanonicalResultValue::Object(CanonicalObject::Value {
        datatype: actual,
        lexical,
    }) = value
    else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-sum input is not a typed Store literal",
        ));
    };
    if *actual != datatype {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-sum input has the wrong exact datatype",
        ));
    }
    Ok(match datatype {
        CanonicalDatatype::UInt8 if lexical.len() == 1 => GroupSumValue::Unsigned(u64::from(lexical[0])),
        CanonicalDatatype::UInt16 if lexical.len() == 2 => {
            GroupSumValue::Unsigned(u64::from(u16::from_be_bytes(lexical.as_slice().try_into().unwrap())))
        }
        CanonicalDatatype::UInt32 if lexical.len() == 4 => {
            GroupSumValue::Unsigned(u64::from(u32::from_be_bytes(lexical.as_slice().try_into().unwrap())))
        }
        CanonicalDatatype::UInt64 if lexical.len() == 8 => {
            GroupSumValue::Unsigned(u64::from_be_bytes(lexical.as_slice().try_into().unwrap()))
        }
        CanonicalDatatype::Int8 if lexical.len() == 1 => {
            GroupSumValue::Signed(i128::from((lexical[0] ^ 0x80) as i8))
        }
        CanonicalDatatype::Int16 if lexical.len() == 2 => {
            let encoded = u16::from_be_bytes(lexical.as_slice().try_into().unwrap());
            GroupSumValue::Signed(i128::from((encoded ^ (1 << 15)) as i16))
        }
        CanonicalDatatype::Int32 if lexical.len() == 4 => {
            let encoded = u32::from_be_bytes(lexical.as_slice().try_into().unwrap());
            GroupSumValue::Signed(i128::from((encoded ^ (1 << 31)) as i32))
        }
        CanonicalDatatype::Int64 if lexical.len() == 8 => {
            let encoded = u64::from_be_bytes(lexical.as_slice().try_into().unwrap());
            GroupSumValue::Signed(i128::from((encoded ^ (1 << 63)) as i64))
        }
        CanonicalDatatype::UInt8
        | CanonicalDatatype::UInt16
        | CanonicalDatatype::UInt32
        | CanonicalDatatype::UInt64
        | CanonicalDatatype::Int8
        | CanonicalDatatype::Int16
        | CanonicalDatatype::Int32
        | CanonicalDatatype::Int64 => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum input has a noncanonical lexical width",
            ))
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum datatype is outside the integral subset",
            ))
        }
    })
}

/// Recompute the `[keys.., sum]` columns a [`CompiledRelationStatement::GroupSum`] delivers:
/// resolve the child's authenticated values, group by `key_columns`, and sum `value_column` in
/// the `datatype`'s exact integral domain. The verifier re-runs this over the root; a wrong-typed
/// operand fails closed.
pub(crate) fn expected_group_sum_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    key_columns: &[usize],
    value_column: usize,
    datatype: CanonicalDatatype,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(
        layer,
        layer.resolution_root(),
        &input.result_columns,
        &descriptors,
    )?;
    let numbers = values.get(value_column).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "group-sum value out of range")
    })?;
    if numbers.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-sum input length mismatch",
        ));
    }
    if key_columns.is_empty() {
        if input.result_len == 0 {
            return Ok(vec![Vec::new()]);
        }
        let mut sum = GroupSumValue::zero(datatype)?;
        for number in numbers {
            sum = sum.checked_add(integral_group_sum_value(number, datatype)?)?;
        }
        return Ok(vec![vec![sum.field()?]]);
    }
    let keys = key_columns
        .iter()
        .map(|key_column| {
            let keys = values.get(*key_column).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "group-sum key out of range")
            })?;
            let fields = input.result_columns.get(*key_column).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "group-sum key field out of range")
            })?;
            if keys.len() != input.result_len || fields.len() != input.result_len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum key length mismatch",
                ));
            }
            Ok((keys, fields))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut sums = std::collections::BTreeMap::<Vec<Vec<u8>>, (Vec<ArkFr>, GroupSumValue)>::new();
    for row in 0..input.result_len {
        let mut canonical = Vec::with_capacity(keys.len());
        let mut fields = Vec::with_capacity(keys.len());
        for (values, key_fields) in &keys {
            let CanonicalResultValue::Object(CanonicalObject::Node(iri)) = &values[row] else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum key is not a node IRI",
                ));
            };
            canonical.push(iri.clone());
            fields.push(key_fields[row]);
        }
        let value = integral_group_sum_value(&numbers[row], datatype)?;
        let entry = sums
            .entry(canonical)
            .or_insert((fields.clone(), GroupSumValue::zero(datatype)?));
        if entry.0 != fields {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum canonical node tuple maps to multiple Store IDs",
            ));
        }
        entry.1 = entry.1.checked_add(value)?;
    }
    let mut output = vec![Vec::with_capacity(sums.len()); key_columns.len() + 1];
    for (fields, sum) in sums.values() {
        for (column, field) in output.iter_mut().zip(fields) {
            column.push(*field);
        }
        output[key_columns.len()].push(sum.field()?);
    }
    Ok(output)
}

/// Recompute the `[keys.., extremum]` columns a [`CompiledRelationStatement::GroupExtrema`]
/// delivers: group the child's authenticated rows by `key_columns` and take the per-group MIN/MAX
/// (`kind`) of `value_column` under the order-preserving `datatype`. Re-run by the verifier.
pub(crate) fn expected_group_extrema_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    key_columns: &[usize],
    value_column: usize,
    datatype: CanonicalDatatype,
    kind: GroupExtremaKind,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(layer, layer.resolution_root(), &input.result_columns, &descriptors)?;
    let numbers = values.get(value_column).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema value out of range")
    })?;
    let fields = input.result_columns.get(value_column).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema field out of range")
    })?;
    if numbers.len() != input.result_len || fields.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-extrema input length mismatch",
        ));
    }
    let select = |rows: &[usize]| -> std::io::Result<Option<ArkFr>> {
        let mut selected: Option<(ArkFr, GroupSumValue)> = None;
        for row in rows {
            let value = integral_group_sum_value(&numbers[*row], datatype)?;
            let replace = match &selected {
                None => true,
                Some((_, current)) => match kind {
                    GroupExtremaKind::Min => value.cmp_same_domain(*current)? == std::cmp::Ordering::Less,
                    GroupExtremaKind::Max => value.cmp_same_domain(*current)? == std::cmp::Ordering::Greater,
                },
            };
            if replace {
                selected = Some((fields[*row], value));
            }
        }
        Ok(selected.map(|(field, _)| field))
    };
    if key_columns.is_empty() {
        let rows = (0..input.result_len).collect::<Vec<_>>();
        return Ok(vec![select(&rows)?.into_iter().collect()]);
    }
    let keys = key_columns.iter().map(|key_column| {
        let values = values.get(*key_column).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema key out of range")
        })?;
        let fields = input.result_columns.get(*key_column).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema key field out of range")
        })?;
        if values.len() != input.result_len || fields.len() != input.result_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-extrema key length mismatch",
            ));
        }
        Ok((values, fields))
    }).collect::<std::io::Result<Vec<_>>>()?;
    let mut groups = std::collections::BTreeMap::<Vec<Vec<u8>>, (Vec<ArkFr>, Vec<usize>)>::new();
    for row in 0..input.result_len {
        let mut canonical = Vec::with_capacity(keys.len());
        let mut key_fields = Vec::with_capacity(keys.len());
        for (key_values, fields) in &keys {
            let CanonicalResultValue::Object(CanonicalObject::Node(iri)) = &key_values[row] else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-extrema key is not a node IRI",
                ));
            };
            canonical.push(iri.clone());
            key_fields.push(fields[row]);
        }
        let entry = groups.entry(canonical).or_insert((key_fields.clone(), Vec::new()));
        if entry.0 != key_fields {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-extrema canonical node tuple maps to multiple Store IDs",
            ));
        }
        entry.1.push(row);
    }
    let mut output = vec![Vec::with_capacity(groups.len()); key_columns.len() + 1];
    for (key_fields, rows) in groups.values() {
        for (column, field) in output.iter_mut().zip(key_fields) {
            column.push(*field);
        }
        output[key_columns.len()].push(select(rows)?.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "empty group-extrema group")
        })?);
    }
    Ok(output)
}

/// Recompute the `[keys.., count]` columns a [`CompiledRelationStatement::GroupCount`] delivers:
/// group the child's authenticated rows by `key_columns` and count each group (empty `key_columns`
/// = the whole-relation cardinality). Re-run by the verifier.
pub(crate) fn expected_group_count_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    key_columns: &[usize],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    if key_columns.is_empty() {
        return expected_cardinality_column(input.result_len, false);
    }
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(
        layer,
        layer.resolution_root(),
        &input.result_columns,
        &descriptors,
    )?;
    let keys = key_columns
        .iter()
        .map(|key_column| {
            let values = values.get(*key_column).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "group-count key out of range")
            })?;
            let fields = input.result_columns.get(*key_column).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-count key field out of range",
                )
            })?;
            if values.len() != input.result_len || fields.len() != input.result_len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-count key length mismatch",
                ));
            }
            Ok((values, fields))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut groups = std::collections::BTreeMap::<Vec<Vec<u8>>, (Vec<ArkFr>, u64)>::new();
    for row in 0..input.result_len {
        let mut canonical = Vec::with_capacity(keys.len());
        let mut fields = Vec::with_capacity(keys.len());
        for (values, key_fields) in &keys {
            let CanonicalResultValue::Object(CanonicalObject::Node(iri)) = &values[row] else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-count key is not a node IRI",
                ));
            };
            canonical.push(iri.clone());
            fields.push(key_fields[row]);
        }
        let entry = groups.entry(canonical).or_insert((fields.clone(), 0));
        if entry.0 != fields {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-count canonical node tuple maps to multiple Store IDs",
            ));
        }
        entry.1 = entry.1.checked_add(1).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "group-count exceeds u64")
        })?;
    }
    let mut output = vec![Vec::with_capacity(groups.len()); key_columns.len() + 1];
    for (fields, count) in groups.values() {
        for (column, field) in output.iter_mut().zip(fields) {
            column.push(*field);
        }
        output[key_columns.len()].push(ArkFr::from_u64(*count));
    }
    Ok(output)
}

/// Decode ONE root-authenticated operand value to its exact integer, as `i128`.
///
/// Reuses the audited fixed-width lexical decoder `integral_group_sum_value` (the same one
/// Sum/GroupSum use). That decoder also asserts the AUTHENTICATED dictionary value carries
/// exactly `datatype`, so a compiler datatype claim that disagrees with the trusted root
/// value rejects closed here rather than decoding a different integer. Every supported
/// fixed-width integer (`u8..u64`, `i8..i64`) fits `i128` exactly.
pub(crate) fn eval_operand_i128(
    value: &CanonicalResultValue,
    datatype: CanonicalDatatype,
) -> std::io::Result<i128> {
    Ok(match integral_group_sum_value(value, datatype)? {
        GroupSumValue::Unsigned(v) => i128::from(v),
        GroupSumValue::Signed(v) => v,
    })
}

/// Re-evaluate the arithmetic tree for ONE row over the authenticated operand columns.
///
/// All accumulation is CHECKED `i128`: an intermediate that escapes `i128` rejects closed
/// (`i128` cannot overflow silently here — every operand is a fixed-width integer and the
/// tree is bounded, but `Times` chains are still guarded). The declared-width narrowing to
/// `i64` happens once at the top (see `expected_eval_columns`).
pub(crate) fn eval_expr_row(
    expr: &EvalExpr,
    values: &[Vec<CanonicalResultValue>],
    row: usize,
) -> std::io::Result<i128> {
    match expr {
        EvalExpr::Column { index, datatype } => {
            let column = values.get(*index).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval operand column out of range",
                )
            })?;
            let value = column.get(row).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval operand column is short",
                )
            })?;
            eval_operand_i128(value, *datatype)
        }
        EvalExpr::Literal(value) => Ok(*value),
        EvalExpr::Plus(left, right) => eval_expr_row(left, values, row)?
            .checked_add(eval_expr_row(right, values, row)?)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval addition overflows the i128 accumulator",
                )
            }),
        EvalExpr::Minus(left, right) => eval_expr_row(left, values, row)?
            .checked_sub(eval_expr_row(right, values, row)?)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval subtraction overflows the i128 accumulator",
                )
            }),
        EvalExpr::Times(left, right) => eval_expr_row(left, values, row)?
            .checked_mul(eval_expr_row(right, values, row)?)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval multiplication overflows the i128 accumulator",
                )
            }),
        EvalExpr::Div(left, right) => {
            let dividend = eval_expr_row(left, values, row)?;
            let divisor = eval_expr_row(right, values, row)?;
            // Div-by-zero: Prolog `X div 0` THROWS (`evaluation_error(zero_divisor)`), so the
            // WOQL query fails. FAIL CLOSED for this row (do NOT drop it — that would silently
            // change the relation). Static zero literals are also rejected at compile time.
            if divisor == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Div by zero: Prolog `div/2` throws, so the query fails closed",
                ));
            }
            // FLOOR division (SWI-Prolog `div/2`, rounds toward −∞). Rust `/` truncates toward
            // zero, so correct the quotient down by one when the truncated remainder is nonzero
            // and its sign differs from the divisor's (the exact floor-vs-truncate gap; this is
            // NOT `div_euclid`, which would round `7 div -2` to −3 instead of the correct −4).
            // `checked_div`/`checked_rem` also fail closed on the sole i128 overflow case
            // (`i128::MIN / -1`), unreachable for fixed-width operands but guarded anyway.
            let truncated = dividend.checked_div(divisor).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Div overflows the i128 accumulator",
                )
            })?;
            let remainder = dividend.checked_rem(divisor).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Div overflows the i128 accumulator",
                )
            })?;
            if remainder != 0 && ((remainder < 0) != (divisor < 0)) {
                truncated.checked_sub(1).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Eval Div floor correction overflows the i128 accumulator",
                    )
                })
            } else {
                Ok(truncated)
            }
        }
        EvalExpr::Exp(base, exponent) => {
            let base = eval_expr_row(base, values, row)?;
            let exponent = eval_expr_row(exponent, values, row)?;
            // Only NON-NEGATIVE INTEGER exponents give an INTEGER result under Prolog `**`
            // (with `prefer_rationals`); a negative exponent yields a rational, which is outside
            // this integer value layer. FAIL CLOSED on a negative or absurdly large exponent
            // (`u32::try_from` rejects both — an exponent past `u32::MAX` cannot possibly fit
            // the declared width and would only overflow below). This also covers a runtime
            // column exponent that turns out negative: the query is rejected, never coerced.
            let exponent = u32::try_from(exponent).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Exp requires a non-negative integer exponent within u32 (a negative exponent is rational; a huge exponent overflows)",
                )
            })?;
            // Checked integer power in i128: an overflow beyond i128 FAILS CLOSED here, and the
            // final narrowing to the declared i64 width (in `expected_eval_columns`) rejects any
            // result outside that width — no field wraparound at any stage.
            base.checked_pow(exponent).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Exp overflows the i128 accumulator",
                )
            })
        }
    }
}

/// VERIFIER RECOMPUTE for WOQL `Eval`: carry the child columns through unchanged and append
/// the derived integer column, re-derived elementwise from ROOT-AUTHENTICATED operands.
///
/// SOUNDNESS: `resolve_result_values` binds every child column value to the trusted root's
/// node/value dictionary (the same authentication Sum uses), so an operand's value is not
/// prover-chosen — it is exactly the store-derived value at the trusted root. The tree is
/// then re-evaluated deterministically in checked `i128` and narrowed to the declared `i64`
/// width; an out-of-`i64` result rejects closed (NO field wraparound). The caller compares
/// the delivered result column against this recomputation, so there is no biasable witness.
/// The child columns are passed through verbatim (they are already root-bound), so the outer
/// equality check also pins them.
pub(crate) fn expected_eval_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    expr: &EvalExpr,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    // Authenticate EVERY child column against the trusted root (ties operands to the root).
    let values = resolve_result_values(layer, layer.resolution_root(), &input.result_columns, &descriptors)?;
    if input.result_columns.iter().any(|column| column.len() != input.result_len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Eval child column length mismatch",
        ));
    }
    // Pass the child columns through unchanged; the outer equality check pins them.
    let mut columns = input.result_columns.clone();
    let mut derived = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let value = eval_expr_row(expr, &values, row)?;
        // Checked-narrow to the declared i64 integer width (mirrors the signed-sum path):
        // any result outside i64 rejects closed rather than wrapping in the field.
        let narrowed = i64::try_from(value).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Eval result exceeds the declared i64 integer width",
            )
        })?;
        derived.push(generated_decimal_i64_field(narrowed));
    }
    columns.push(derived);
    Ok(columns)
}

// ============================================================================================
// Exact-rational (xsd:decimal) value layer: shared recompute for `Floor`, `Divide`, and decimal
// operands. Every function here is verifier-recompute: it consumes ONLY root-authenticated operand
// values and trusted query literals, and is deterministic — there is no prover witness.
// ============================================================================================

/// Parse a CANONICAL `xsd:decimal` lexical string (optional leading `-`, decimal digits, optional
/// `.`+digits; NO exponent) to an EXACT `rug::Rational`. This mirrors the reference
/// `casting:string_decimal_to_rational/2` (`numerator = int*10^fraclen + frac`, `denom = 10^fraclen`)
/// exactly, using GMP big integers so ANY precision is preserved. A leading-`.` form (`.5`) is
/// accepted as `0.5`. FAILS CLOSED on scientific notation, empty/garbage, or a non-digit character —
/// never guesses a value. The store's own decimals decode to this canonical form
/// (`storage_to_decimal`), and stored decimals are the only decimal OPERAND source; decimal LITERALS
/// are canonicalized by the compiler before reaching here.
pub(crate) fn parse_decimal_to_rational(text: &str) -> std::io::Result<BigRational> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "xsd:decimal lexical is not a canonical parseable decimal",
        )
    };
    let (neg, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if body.is_empty() {
        return Err(invalid());
    }
    // Split into integer / fractional digit runs; reject anything that is not pure decimal digits.
    let (int_str, frac_str) = match body.split_once('.') {
        Some((int_part, frac_part)) => (int_part, frac_part),
        None => (body, ""),
    };
    // Leading `.5` -> integer part empty; treat as 0. A trailing `5.` -> fraction empty; treat as 0.
    let int_digits = if int_str.is_empty() { "0" } else { int_str };
    if !int_digits.bytes().all(|b| b.is_ascii_digit())
        || (!frac_str.is_empty() && !frac_str.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(invalid());
    }
    let denom = BigInt::from(10).pow(frac_str.len() as u32);
    // numerator = int_digits ++ frac_str  (as one big decimal integer), signed.
    let mut digits = String::with_capacity(int_digits.len() + frac_str.len());
    digits.push_str(int_digits);
    digits.push_str(frac_str);
    let mut numer = BigInt::from_str_radix(&digits, 10).map_err(|_| invalid())?;
    if neg {
        numer = -numer;
    }
    // `BigRational::new` auto-reduces to lowest terms with a positive denominator.
    Ok(BigRational::new(numer, denom))
}

/// Decode ONE root-authenticated child value to an EXACT rational for the decimal/rational layer.
/// The exact datatype is read from the TRUSTED dictionary value (never a compiler claim): the
/// fixed-width integer family decodes via the SAME order-preserving path `Sum`/`Eval` use
/// (`integral_group_sum_value`); `xsd:decimal` decodes via the store's own lexical decoder
/// ([`storage_to_decimal`]) then `parse_decimal_to_rational`. Any other datatype, a node IRI, a
/// null, or a generated value FAILS CLOSED — the recompute never fabricates or coerces a numeric.
pub(crate) fn numeric_operand_rational(value: &CanonicalResultValue) -> std::io::Result<BigRational> {
    let non_numeric = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL decimal-layer operand row is not an authenticated numeric (integer/xsd:decimal) value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value { datatype, lexical }) = value else {
        return Err(non_numeric());
    };
    match datatype {
        CanonicalDatatype::Decimal => {
            // The store's xsd:decimal lexical is the BCD storage encoding; decode it to the
            // canonical decimal string, then to an exact rational. Always terminating/finite.
            let decoded = storage_to_decimal(&mut &lexical[..]);
            parse_decimal_to_rational(&decoded)
        }
        CanonicalDatatype::UInt8
        | CanonicalDatatype::UInt16
        | CanonicalDatatype::UInt32
        | CanonicalDatatype::UInt64
        | CanonicalDatatype::Int8
        | CanonicalDatatype::Int16
        | CanonicalDatatype::Int32
        | CanonicalDatatype::Int64 => match integral_group_sum_value(value, *datatype)? {
            GroupSumValue::Unsigned(v) => Ok(BigRational::from_integer(BigInt::from(v))),
            GroupSumValue::Signed(v) => {
                // Every supported signed fixed-width value fits i128 exactly (already is one).
                Ok(BigRational::from_integer(BigInt::from(v)))
            }
        },
        _ => Err(non_numeric()),
    }
}

/// Re-evaluate a [`DecimalExpr`] for ONE row over the root-authenticated operand columns, in EXACT
/// `rug::Rational`. All the fail-closed cases (zero divisor, non-numeric/unparseable operand,
/// out-of-scope Exp exponent) surface here as errors — never a dropped or fabricated row.
pub(crate) fn eval_decimal_expr_row(
    expr: &DecimalExpr,
    values: &[Vec<CanonicalResultValue>],
    row: usize,
) -> std::io::Result<BigRational> {
    match expr {
        DecimalExpr::Column { index } => {
            let column = values.get(*index).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL decimal-layer operand column out of range",
                )
            })?;
            let value = column.get(row).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL decimal-layer operand column is short",
                )
            })?;
            numeric_operand_rational(value)
        }
        DecimalExpr::IntegerLiteral(value) => Ok(BigRational::from_integer(BigInt::from(*value))),
        DecimalExpr::DecimalLiteral(lexical) => parse_decimal_to_rational(lexical),
        DecimalExpr::Plus(left, right) => Ok(eval_decimal_expr_row(left, values, row)?
            + eval_decimal_expr_row(right, values, row)?),
        DecimalExpr::Minus(left, right) => Ok(eval_decimal_expr_row(left, values, row)?
            - eval_decimal_expr_row(right, values, row)?),
        DecimalExpr::Times(left, right) => Ok(eval_decimal_expr_row(left, values, row)?
            * eval_decimal_expr_row(right, values, row)?),
        DecimalExpr::Divide(left, right) => {
            let dividend = eval_decimal_expr_row(left, values, row)?;
            let divisor = eval_decimal_expr_row(right, values, row)?;
            // Zero divisor: Prolog `rdiv`/`is` THROWS (`evaluation_error(zero_divisor)`), so the
            // WOQL query fails. FAIL CLOSED for this row (never drop it; never panic on the
            // divide). Static zero literal divisors are also rejected at compile time.
            if divisor.numer().is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Divide by zero: Prolog `rdiv` throws, so the query fails closed",
                ));
            }
            Ok(dividend / divisor)
        }
        DecimalExpr::Exp(base, exponent) => {
            let base = eval_decimal_expr_row(base, values, row)?;
            let exponent = eval_decimal_expr_row(exponent, values, row)?;
            // Only a NON-NEGATIVE INTEGER exponent gives an exact rational power in this layer;
            // the compiler already required a static non-negative integer literal, but re-check the
            // recomputed rational to be safe (a fractional/negative exponent FAILS CLOSED).
            if !exponent.is_integer() || exponent.numer().is_negative() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Exp in the decimal layer requires a non-negative integer exponent",
                ));
            }
            let e = exponent.numer().to_u32().filter(|e| *e <= DECIMAL_LAYER_MAX_EXP).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Exp exponent is too large for the decimal layer",
                )
            })?;
            // base^e by exact repeated multiplication (base^0 = 1); rationals stay exact.
            let mut acc = BigRational::one();
            for _ in 0..e {
                acc = acc * base.clone();
            }
            Ok(acc)
        }
        DecimalExpr::Floor(argument) => {
            // floor toward −∞ (SWI `floor/1`); `BigRational::floor` returns the integral rational.
            let value = eval_decimal_expr_row(argument, values, row)?;
            Ok(value.floor())
        }
    }
}

/// Render an EXACT rational to the reference's client-facing `xsd:decimal` lexical, reproducing
/// `casting:rational_to_decimal_string(R, S, 20)` BYTE-EXACT (validated differentially against
/// SWI-Prolog): sign, integer part, then up to 20 fractional digits ROUNDED HALF-UP at the 20th
/// (`(Rem*10^20*2 + Den) // (Den*2)`), with LEADING-zero padding to 20 and TRAILING zeros trimmed;
/// a whole rational renders as the bare integer (no decimal point). `rug`/GMP is the SAME big-integer
/// library SWI uses, and every operand here is non-negative (abs taken), so `//`/`mod` == floor ==
/// rug truncating division — identical arithmetic. FAILS CLOSED on the astronomically-implausible
/// carry-overflow case (the fraction rounds up to a full unit, `FracDigits == 10^20`, requiring a
/// reduced denominator ≥ 2·10^20) rather than reproduce the reference's carry BUG there.
pub(crate) const DECIMAL_LAYER_MAX_FRACTION_DIGITS: u32 = 20;
/// Upper bound on a decimal-layer `Exp` exponent (keeps exact `rug` powers from blowing up; larger
/// exponents reject closed rather than compute an astronomically large rational).
pub(crate) const DECIMAL_LAYER_MAX_EXP: u32 = 256;

/// Render an exact rational as the client-facing decimal lexical, BYTE-EXACT to the reference
/// `casting:rational_to_decimal_string(R, S, 20)`: up to `DECIMAL_LAYER_MAX_FRACTION_DIGITS`
/// fractional digits, round-half-up at the last, trailing zeros trimmed, whole rationals as the
/// bare integer. Used by both the prover and the verifier for the decimal / rational value layer
/// and the number→string cast. Fails closed on the astronomically-implausible carry-overflow case.
pub(crate) fn rational_to_decimal_string_20(value: &BigRational) -> std::io::Result<String> {
    let max_decimals = DECIMAL_LAYER_MAX_FRACTION_DIGITS;
    let negative = value.numer().sign() == Sign::Minus;
    let abs = value.abs();
    // num-rational canonicalizes: denom > 0, gcd(numer,denom) = 1, sign in numer. After abs, >= 0.
    let num = abs.numer().clone();
    let den = abs.denom().clone();
    let (int_part, remainder) = num.div_rem(&den);
    if remainder.is_zero() {
        // Whole number: no fractional part. (A negative whole renders with its sign.)
        let sign = if negative && !int_part.is_zero() {
            "-"
        } else {
            ""
        };
        return Ok(format!("{sign}{int_part}"));
    }
    let multiplier = BigInt::from(10).pow(max_decimals);
    // Round-half-up: (Rem * Mult * 2 + Den) // (Den * 2). All operands positive => BigInt `/`
    // truncates toward zero == floor == SWI `//`.
    let frac_digits: BigInt =
        (remainder * &multiplier * BigInt::from(2) + &den) / (den * BigInt::from(2));
    if frac_digits >= multiplier {
        // The fraction rounded up to a whole unit (needs denom >= 2·10^20). The reference has a
        // carry BUG here; refuse rather than emit a value that might mismatch.
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "xsd:decimal render carry overflow (denominator >= 2·10^20); refusing rather than reproduce the reference carry bug",
        ));
    }
    // Pad with leading zeros to `max_decimals`, then trim trailing zeros.
    let mut frac = frac_digits.to_string();
    if (frac.len() as u32) < max_decimals {
        let pad = (max_decimals as usize) - frac.len();
        frac = format!("{}{}", "0".repeat(pad), frac);
    }
    let trimmed = frac.trim_end_matches('0');
    // NOTE: `trimmed` CAN be empty. A tiny-but-nonzero remainder rounds to 0 at
    // `max_decimals` digits once the reduced denominator exceeds 2·10^20, so `frac_digits`
    // is 0, and after padding + trimming nothing survives — the output is then `"<int>."`
    // (e.g. `"0."`). That trailing-dot form is exactly what the SWI reference emits here
    // (confirmed byte-for-byte), so it is produced DELIBERATELY; "normalizing" it away
    // (dropping the dot, or re-adding a `0`) would DIVERGE from the reference.
    let sign = if negative { "-" } else { "" };
    Ok(format!("{sign}{int_part}.{trimmed}"))
}

/// VERIFIER RECOMPUTE for WOQL `floor(x)`: carry the child columns through unchanged and append the
/// derived integer column, re-derived elementwise from ROOT-authenticated operands in exact
/// rationals then floored toward −∞ and narrowed to the declared `i64` generated domain (a result
/// outside `i64` FAILS CLOSED — no field wraparound). Mirrors `expected_eval_columns`.
pub(crate) fn expected_floor_eval_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    expr: &DecimalExpr,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let mut columns = input.result_columns.clone();
    let mut derived = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        // `expr` is the floor ARGUMENT (the compiler stores it here and floors it explicitly), so
        // evaluate then floor toward −∞ to an integer.
        let value = eval_decimal_expr_row(expr, &values, row)?;
        let floored = value.floor().to_integer();
        let narrowed = floored.to_i64().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL floor result exceeds the declared i64 integer width",
            )
        })?;
        derived.push(generated_decimal_i64_field(narrowed));
    }
    columns.push(derived);
    Ok(columns)
}

/// VERIFIER RECOMPUTE for WOQL exact-rational arithmetic (`Divide`/decimal-operand arithmetic): the
/// per-row result is a TERMINAL, REVEALED `xsd:decimal` value (like `Concat`) — re-derive the exact
/// rational from ROOT-authenticated operands and render the canonical 20-fractional-digit lexical.
/// A zero divisor, a non-numeric operand row, or the carry-overflow case FAIL CLOSED, so a proof is
/// only ever produced for rows the reference itself would compute, and the delivered value matches.
pub(crate) fn expected_rational_arith_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    expr: &DecimalExpr,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let value = eval_decimal_expr_row(expr, &values, row)?;
        let lexical = rational_to_decimal_string_20(&value)?.into_bytes();
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::Decimal,
            lexical,
        });
    }
    Ok(out)
}

// ============================================================================================
// WOQL NUMBER↔STRING Typecast value layer (verifier-recompute; TERMINAL revealed result).
//
// Two delivery disciplines, both TERMINAL/revealed exactly like Concat/RationalArith (NO store
// change, NO commitment-format change, NO novel crypto):
//   * NUMBER→STRING (`cast_number_to_string_bytes`): decode the root-authenticated numeric source
//     and RENDER its client-facing lexical — integer family via base-10 (`format ~w`), xsd:decimal
//     via the already-SWI-validated `rational_to_decimal_string_20`. Delivered as `xsd:string`.
//   * STRING→NUMBER (`cast_string_to_integer_bigint` / `cast_string_to_decimal_rational`): PARSE
//     the root-authenticated xsd:string source with the exact reference grammar and deliver the
//     numeric value's client-facing lexical (`xsd:integer` base-10; `xsd:decimal` via
//     `rational_to_decimal_string_20`). A malformed/out-of-grammar string FAILS CLOSED (SWI
//     `typecast` throws → the query fails → the proof fails closed, never a wrong/silent value).
//
// Every grammar/render below was differentially validated against a live SWI-Prolog 10.0.2 running
// the reference predicates (`casting:typecast_switch`, `casting:string_decimal_to_rational/2`,
// `xsd_parser:integer//1`, `casting:rational_to_decimal_string/3`). SOUNDNESS is identical to
// Concat/RationalArith: the operand is pinned to the trusted root (`resolve_authenticated_child_values`),
// the cast is deterministic and byte-exact, and the delivered terminal result must equal the
// recomputation — no biasable witness.

/// Render ONE root-authenticated NUMERIC source value to its client-facing `xsd:string` lexical,
/// reproducing `casting:typecast_switch(xsd:string, SourceType, Val, _, S)` BYTE-EXACT.
///
/// * integer family (fixed-width `Int*`/`UInt*` and arbitrary `BigInt`=xsd:integer): SWI
///   `format(string(S),"~w",[Val])` = base-10 with a leading `-` for negatives and NO leading zeros;
///   Rust `to_string()` on `i128`/`u64`/`rug::Integer` reproduces this exactly.
/// * `xsd:decimal`: SWI `rational_to_decimal_string(Val, S, 20)` — the SAME renderer
///   `rational_to_decimal_string_20` the decimal/rational layer already delivers byte-exact.
///
/// FAILS CLOSED (reject-closed over approximate) on a `float`/`double` source — whose SWI `~w`
/// float formatting (shortest round-trip, forced `.0`) we do NOT reproduce — and on any non-numeric
/// source (date/boolean/langString/node), a null, or a generated scalar. There is no biasable
/// witness: the bytes come from the root-authenticated dictionary value.
pub(crate) fn cast_number_to_string_bytes(value: &CanonicalResultValue) -> std::io::Result<Vec<u8>> {
    let not_castable = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL number→string Typecast source row is not an authenticated integer/xsd:decimal value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value { datatype, lexical }) = value else {
        return Err(not_castable());
    };
    let rendered = match datatype {
        // Fixed-width integer family → base-10 (`format ~w`). `integral_group_sum_value` re-checks
        // the exact datatype AND the canonical byte width against the root-authenticated lexical.
        CanonicalDatatype::UInt8
        | CanonicalDatatype::UInt16
        | CanonicalDatatype::UInt32
        | CanonicalDatatype::UInt64
        | CanonicalDatatype::Int8
        | CanonicalDatatype::Int16
        | CanonicalDatatype::Int32
        | CanonicalDatatype::Int64 => match integral_group_sum_value(value, *datatype)? {
            GroupSumValue::Signed(v) => v.to_string(),
            GroupSumValue::Unsigned(v) => v.to_string(),
        },
        // Arbitrary-precision xsd:integer → base-10 via the store's OWN bigint decoder (the exact
        // inverse of the encoding the root-authenticated lexical carries); `rug::Integer` Display is
        // base-10 with a leading `-`, matching SWI `format ~w`.
        CanonicalDatatype::BigInt => storage_to_bigint(&mut &lexical[..]).to_string(),
        // xsd:decimal → the exact rational (store BCD lexical → canonical string → rational), rendered
        // by the SWI-validated 20-fractional-digit renderer.
        CanonicalDatatype::Decimal => {
            let rational = parse_decimal_to_rational(&storage_to_decimal(&mut &lexical[..]))?;
            rational_to_decimal_string_20(&rational)?
        }
        // float/double (SWI float `~w`) + every other datatype are reject-closed.
        _ => return Err(not_castable()),
    };
    Ok(rendered.into_bytes())
}

/// Parse an `xsd:string` operand into an `xsd:integer` (arbitrary precision), reproducing the
/// reference `xsd_parser:integer//1` grammar (`casting.pl:569` `phrase(integer(Casted),C,[])`)
/// BYTE-EXACT: `sign(+/-/none)` then `natural` (`number_string` over one-or-more digits). That is
/// exactly `^[+-]?[0-9]+$` with LEADING ZEROS allowed (`"007" -> 7`, `"-0" -> 0`, `"+5" -> 5`),
/// differentially confirmed against SWI 10.0.2. Anything else — whitespace, an empty/sign-only
/// string, a decimal point, scientific `1e5`, hex `0x1f`, digit-group `1_000` — FAILS CLOSED (SWI's
/// `phrase(...,[])` leaves input unconsumed and `typecast` throws → the proof fails closed).
pub(crate) fn cast_string_to_integer_bigint(bytes: &[u8]) -> std::io::Result<BigInt> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string→integer Typecast source is not a `[+-]?[0-9]+` integer lexical",
        )
    };
    // xsd:string content is UTF-8; a non-UTF-8 lexical never occurs for a faithful store, and if it
    // did we refuse rather than guess.
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    // All-ASCII-digit run: base-10 (leading zeros collapse in VALUE, exactly like SWI `number_string`).
    let mut value = BigInt::from_str_radix(digits, 10).map_err(|_| invalid())?;
    if negative {
        value = -value;
    }
    Ok(value)
}

/// Parse an `xsd:string` operand into an `xsd:decimal` (exact rational), reproducing a BYTE-EXACT
/// SAFE SUBSET of the reference `casting:string_decimal_to_rational/2` (`casting.pl:555`).
///
/// Accepted (all differentially confirmed against SWI 10.0.2): an optional single leading `-`, then
/// one of `D+` (integer), `D+.D+` (both runs non-empty), or `.D+` (leading dot, integer part 0). The
/// value is `sign * (intdigits ++ fracdigits) / 10^len(fracdigits)` — arithmetically IDENTICAL to
/// the reference's `IntPart*10^FracLen + FracPart` over `10^FracLen`.
///
/// REJECTED CLOSED (a strict subset of SWI's acceptance — rejecting is incompleteness-safe, never a
/// wrong value): trailing dot `D+.` (SWI's `atom_number("")` FAILS → `typecast` throws), a leading
/// `+` (SWI would route it through `atom_number`, which we do NOT reproduce), scientific `1e5`
/// (SWI's `atom_number` yields a FLOAT — a DIFFERENT value/type), hex `0x1f`, Prolog rational
/// literals `3r4`, multiple dots, whitespace, and the empty/sign-only string.
pub(crate) fn cast_string_to_decimal_rational(bytes: &[u8]) -> std::io::Result<BigRational> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string→decimal Typecast source is not a reproducible decimal lexical (reject-closed subset of string_decimal_to_rational)",
        )
    };
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    // The reference strips a leading `-` ONLY (never `+`); reproduce exactly.
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if body.is_empty() || body.starts_with('+') {
        return Err(invalid());
    }
    let (int_str, frac_str) = match body.split_once('.') {
        Some((int_part, frac_part)) => {
            // A SECOND '.' -> SWI `split_string` yields 3 parts and the clause fails. Reject.
            if frac_part.contains('.') {
                return Err(invalid());
            }
            // Trailing dot `D+.` -> SWI `atom_number("")` fails -> `typecast` throws. Reject.
            if frac_part.is_empty() {
                return Err(invalid());
            }
            (int_part, frac_part)
        }
        // No '.' -> integer form (SWI `atom_number(String)` over a pure-digit string).
        None => (body, ""),
    };
    // Leading dot `.D+` -> the reference prepends "0"; treat an empty integer part as 0.
    let int_digits = if int_str.is_empty() { "0" } else { int_str };
    if !int_digits.bytes().all(|b| b.is_ascii_digit())
        || (!frac_str.is_empty() && !frac_str.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(invalid());
    }
    let denominator = BigInt::from(10).pow(frac_str.len() as u32);
    // numerator = (int_digits ++ frac_str) as one big decimal integer, signed.
    let mut digits = String::with_capacity(int_digits.len() + frac_str.len());
    digits.push_str(int_digits);
    digits.push_str(frac_str);
    let mut numerator = BigInt::from_str_radix(&digits, 10).map_err(|_| invalid())?;
    if negative {
        numerator = -numerator;
    }
    // `BigRational::new` auto-reduces to lowest terms with a positive denominator.
    Ok(BigRational::new(numerator, denominator))
}

/// VERIFIER RECOMPUTE for a WOQL NUMBER→STRING `Typecast`: authenticate the numeric source column
/// against the trusted root, RENDER each row's client-facing `xsd:string` lexical
/// (`cast_number_to_string_bytes`), and deliver revealed `xsd:string` objects. A float/double or
/// non-numeric operand row FAILS CLOSED, so a proof is only ever produced for rows the reference
/// itself would cast, and the delivered value matches byte-for-byte.
pub(crate) fn expected_cast_to_string_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    source_column: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let column = values.get(source_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL number→string Typecast source column out of range",
        )
    })?;
    if column.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL number→string Typecast source column length mismatch",
        ));
    }
    let mut out = Vec::with_capacity(input.result_len);
    for value in column {
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: cast_number_to_string_bytes(value)?,
        });
    }
    Ok(out)
}

/// VERIFIER RECOMPUTE for a WOQL STRING→NUMBER `Typecast`: authenticate the `xsd:string` source
/// column against the trusted root (`authenticate_string_column`), PARSE each row with the exact
/// reference grammar for `target`, and deliver the numeric value's client-facing lexical
/// (`xsd:integer` base-10 tagged `BigInt`; `xsd:decimal` via `rational_to_decimal_string_20` tagged
/// `Decimal`). A malformed/out-of-grammar string FAILS CLOSED (the reference throws), so a proof is
/// only ever produced for rows the reference itself would cast, and the delivered value matches.
pub(crate) fn expected_cast_from_string_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    source_column: usize,
    target: StringNumericTarget,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    // Root-authenticated xsd:string bytes per row (a non-string operand row FAILS CLOSED here).
    let source_bytes = authenticate_string_column(&values, source_column, input.result_len)?;
    let mut out = Vec::with_capacity(input.result_len);
    for bytes in &source_bytes {
        let object = match target {
            StringNumericTarget::Integer => CanonicalObject::Value {
                datatype: CanonicalDatatype::BigInt,
                lexical: cast_string_to_integer_bigint(bytes)?.to_string().into_bytes(),
            },
            StringNumericTarget::Decimal => {
                let rational = cast_string_to_decimal_rational(bytes)?;
                CanonicalObject::Value {
                    datatype: CanonicalDatatype::Decimal,
                    lexical: rational_to_decimal_string_20(&rational)?.into_bytes(),
                }
            }
        };
        out.push(object);
    }
    Ok(out)
}

// ============================================================================================
// WOQL CALENDAR value layer (verifier-recompute; xsd:date / xsd:gYearMonth operand only).
//
// Single-input date functions over the newly-committed dates. Two delivery disciplines, exactly
// like the arithmetic/string layers (NO store change, NO commitment-format change, NO novel crypto):
//   * INTEGER results (`Weekday`/`WeekdaySundayStart`/`IsoWeek`) — delivered like `Eval`: the child
//     columns pass through and ONE derived `GeneratedDecimalI64` column is appended PER bound integer
//     (1 for Weekday/WeekdaySundayStart, 2 for IsoWeek's {year, week}); the verifier RE-DERIVES them.
//   * DATE results (`DayAfter`/`DayBefore`/`MonthStartDate`/`MonthEndDate`) — delivered like
//     `Concat`/`RationalArith`: a TERMINAL, REVEALED `xsd:date` value the verifier RE-DERIVES and
//     checks (no committed result column).
//
// Every calendar function below is ported BYTE-FOR-BYTE from the SWI-Prolog reference
// (`terminusdb/src/core/query/woql_compile.pl`, cited per fn) and was differentially validated
// against a live SWI-Prolog 10.0.2 on the reference test vectors. The operand date is authenticated
// against the trusted root (`resolve_authenticated_child_values` — the SAME authentication
// Eval/strings/decimal use), the function is recomputed deterministically, and the delivered
// terminal/derived result must equal that recomputation. There is NO prover witness.
//
// FIDELITY GUARDS (reject-closed / fail-closed rather than approximate):
//   * xsd:date operands ONLY. An `xsd:dateTime` operand is reject-closed: decoding its
//     variable-width store lexical would require reproducing the store's private dateTime decoder,
//     risking drift — reject-closed over approximate. (The reference `weekday`/`iso_week` also accept
//     dateTime; that is a coverage boundary, not a soundness gap — a non-date operand fails closed.)
//   * INPUT year >= 1: the ported integer algorithms use `//`/`mod`, which equal Rust `/`/`.rem_euclid`
//     only over NON-NEGATIVE dividends; year >= 1 keeps every division non-negative, so the port is
//     byte-exact. A year < 1 fails closed rather than risk a negative-division mismatch.
//   * DATE outputs: timezone offset 0 ONLY, output year >= 1. The client lexical is `YYYY-MM-DD`
//     (SWI `date_string`, `literals.pl:290` / `xsd_parser.pl:255`: `~|~`0t~d~4+-~|~`0t~d~2+-...`
//     with an EMPTY offset suffix for offset 0) — reproduced exactly by `format!("{:04}-{:02}-{:02}")`
//     for year >= 1. A non-zero offset (whose `±HH:MM` suffix we do not reproduce) or an out-of-range
//     output year fails closed.

/// Which single-input, INTEGER-result calendar function a [`CompiledRelationStatement::CalendarInt`]
/// output column computes. Each maps a bound `xsd:date` to one `xsd:integer`, matching the SWI
/// reference exactly. `IsoWeek` binds TWO integers, so it carries `[IsoWeekYear, IsoWeekWeek]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarIntFn {
    /// `weekday(Date, W)` (`woql_compile.pl:1657`) via Sakamoto `iso_weekday_number` (`:1637`):
    /// ISO-8601 weekday, Monday = 1 .. Sunday = 7.
    Weekday,
    /// `weekday_sunday_start(Date, W)` (`woql_compile.pl:1675`): US convention `(IsoDay mod 7) + 1`,
    /// Sunday = 1 .. Saturday = 7.
    WeekdaySundayStart,
    /// `iso_week(Date, Year, W)` (`woql_compile.pl:1744` / `iso_week_date` `:1710`): the ISO-8601
    /// week-numbering YEAR component.
    IsoWeekYear,
    /// `iso_week(Date, Year, W)`: the ISO-8601 WEEK-number component (1..53).
    IsoWeekWeek,
}

/// Which single-input, DATE-result calendar function a [`CompiledRelationStatement::CalendarDate`]
/// statement computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarDateFn {
    /// `day_after(Date, Next)` (`woql_compile.pl:1285`): the calendar day after a bound `xsd:date`.
    DayAfter,
    /// `day_before(Date, Previous)` (`woql_compile.pl:1310`): the calendar day before a bound `xsd:date`.
    DayBefore,
    /// `month_start_date(YearMonth, Date)` (`woql_compile.pl:1205`): first day of the month of a
    /// bound `xsd:gYearMonth` (`date(Y,M,1,Offset)`).
    MonthStartDate,
    /// `month_end_date(YearMonth, Date)` (`woql_compile.pl:1219`): last day of that month
    /// (`date(Y,M,days_in_month(Y,M),Offset)`).
    MonthEndDate,
}

/// SWI `is_leap_year/1` (`woql_compile.pl:1520`): the proleptic Gregorian leap rule.
pub(crate) fn cal_is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// SWI `days_in_month/3` (`woql_compile.pl:1527`). `None` for a month outside `1..=12` (a corrupt
/// operand lexical — a store `Date`/`GYearMonth` always carries a valid month, so this only guards
/// against corruption and fails closed).
pub(crate) fn cal_days_in_month(year: i64, month: u8) -> Option<u8> {
    Some(match month {
        1 => 31,
        2 => if cal_is_leap_year(year) { 29 } else { 28 },
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => return None,
    })
}

/// SWI `iso_weekday_number/4` (`woql_compile.pl:1637`, Sakamoto's algorithm): ISO weekday
/// Monday = 1 .. Sunday = 7. The caller guarantees `year >= 1` and `1 <= month <= 12`, so every
/// `//` here is over a NON-NEGATIVE dividend and Rust `/`/`.rem_euclid(7)` equal SWI's `//`/`mod`
/// byte-for-byte (they diverge only on negative operands, which the guards exclude).
pub(crate) fn cal_iso_weekday_number(year: i64, month: u8, day: u8) -> i64 {
    // `nth0(M, [0,0,3,2,5,0,3,5,1,4,6,2,4], T)` — index `month` (index 0 is unused, `month>=1`).
    const MONTH_TABLE: [i64; 13] = [0, 0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let t = MONTH_TABLE[month as usize];
    let y1 = if month < 3 { year - 1 } else { year };
    let dow = (y1 + y1 / 4 - y1 / 100 + y1 / 400 + t + i64::from(day)).rem_euclid(7);
    // DoW: 0 = Sunday .. 6 = Saturday → ISO Monday = 1 .. Sunday = 7.
    if dow == 0 {
        7
    } else {
        dow
    }
}

/// SWI `woql_weekday_sunday_start/2` (`woql_compile.pl:1675`): `USDay is (IsoDay mod 7) + 1`
/// (Sunday = 1 .. Saturday = 7). `iso >= 1`, so `.rem_euclid(7)` equals SWI `mod 7` exactly.
pub(crate) fn cal_us_weekday(year: i64, month: u8, day: u8) -> i64 {
    cal_iso_weekday_number(year, month, day).rem_euclid(7) + 1
}

/// SWI `ordinal_day/4` (`woql_compile.pl:1693`): day-of-year (1..366). `None` on a corrupt month.
pub(crate) fn cal_ordinal_day(year: i64, month: u8, day: u8) -> Option<i64> {
    let mut acc: i64 = 0;
    for m in 1..month {
        acc += i64::from(cal_days_in_month(year, m)?);
    }
    Some(acc + i64::from(day))
}

/// SWI `iso_week_date/5` (`woql_compile.pl:1710`): the ISO-8601 (week-numbering year, week number).
/// The caller guarantees `year >= 1` and `1 <= month <= 12`; every `//` is over a non-negative
/// dividend (`adj - 1 >= 0` and `thursday - 1 >= 0` in their branches), so `.div_euclid(7)` equals
/// SWI `//` byte-for-byte.
pub(crate) fn cal_iso_week_date(year: i64, month: u8, day: u8) -> Option<(i64, i64)> {
    let day_of_week = cal_iso_weekday_number(year, month, day);
    let ordinal = cal_ordinal_day(year, month, day)?;
    // Thursday of this week's ordinal day.
    let thursday_ord = ordinal + (4 - day_of_week);
    Some(if thursday_ord < 1 {
        // Thursday is in the previous year.
        let prev_year = year - 1;
        let prev_days = if cal_is_leap_year(prev_year) { 366 } else { 365 };
        let adj_ord = prev_days + thursday_ord;
        (prev_year, (adj_ord - 1).div_euclid(7) + 1)
    } else {
        let year_days = if cal_is_leap_year(year) { 366 } else { 365 };
        if thursday_ord > year_days {
            // Thursday is in the next year.
            (year + 1, 1)
        } else {
            (year, (thursday_ord - 1).div_euclid(7) + 1)
        }
    })
}

/// SWI `next_month/4` (`woql_compile.pl:1273`).
pub(crate) fn cal_next_month(year: i64, month: u8) -> (i64, u8) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// SWI `prev_month/4` (`woql_compile.pl:1276`).
pub(crate) fn cal_prev_month(year: i64, month: u8) -> (i64, u8) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

/// SWI `woql_day_after/2` forward branch (`woql_compile.pl:1285`): given a bound date, the day after.
/// `None` on a corrupt month.
pub(crate) fn cal_day_after(year: i64, month: u8, day: u8) -> Option<(i64, u8, u8)> {
    let max_day = cal_days_in_month(year, month)?;
    Some(if day < max_day {
        (year, month, day + 1)
    } else {
        let (next_year, next_month) = cal_next_month(year, month);
        (next_year, next_month, 1)
    })
}

/// SWI `woql_day_before/2` forward branch (`woql_compile.pl:1310`): given a bound date, the day before.
/// `None` on a corrupt month.
pub(crate) fn cal_day_before(year: i64, month: u8, day: u8) -> Option<(i64, u8, u8)> {
    Some(if day > 1 {
        (year, month, day - 1)
    } else {
        let (prev_year, prev_month) = cal_prev_month(year, month);
        let prev_max = cal_days_in_month(prev_year, prev_month)?;
        (prev_year, prev_month, prev_max)
    })
}

/// Decode ONE root-authenticated `xsd:date` operand row to `(year, month, day, offset)`, reproducing
/// tdb-succinct's `Date` lexical layout BYTE-FOR-BYTE — the SAME 12 bytes the store's
/// `date_lexical_to_op_i64` decodes: `year:i64` offset-binary BE (8) | `month:u8` (1) | `day:u8` (1)
/// | `offset:i16` offset-binary BE (2). FAILS CLOSED on a non-date row (node IRI, non-date value,
/// null/generated), a wrong-width lexical, or an out-of-range calendar month/day (a corrupt lexical).
pub(crate) fn cal_decode_date_operand(value: &CanonicalResultValue) -> std::io::Result<(i64, u8, u8, i16)> {
    let not_date = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand row is not an authenticated xsd:date value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value {
        datatype: CanonicalDatatype::Date,
        lexical,
    }) = value
    else {
        return Err(not_date());
    };
    let bytes: [u8; 12] = lexical.as_slice().try_into().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar xsd:date lexical is not exactly 12 bytes",
        )
    })?;
    let year = (u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]) ^ (1u64 << 63)) as i64;
    let month = bytes[8];
    let day = bytes[9];
    let offset = (u16::from_be_bytes([bytes[10], bytes[11]]) ^ (1u16 << 15)) as i16;
    // A store `Date` is always a valid calendar day; a corrupt lexical (bad month/day) fails closed.
    match cal_days_in_month(year, month) {
        Some(max) if (1..=max).contains(&day) => {}
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar xsd:date has an out-of-range month or day",
            ))
        }
    }
    Ok((year, month, day, offset))
}

/// Decode ONE root-authenticated `xsd:gYearMonth` operand row to `(year, month, offset)`, reproducing
/// tdb-succinct's `GYearMonth` lexical layout BYTE-FOR-BYTE: `year:i64` offset-binary BE (8) |
/// `month:u8` (1) | `offset:i16` offset-binary BE (2). FAILS CLOSED on a non-gYearMonth row, a
/// wrong-width lexical, or an out-of-range month.
pub(crate) fn cal_decode_gyearmonth_operand(value: &CanonicalResultValue) -> std::io::Result<(i64, u8, i16)> {
    let not_ym = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand row is not an authenticated xsd:gYearMonth value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value {
        datatype: CanonicalDatatype::GYearMonth,
        lexical,
    }) = value
    else {
        return Err(not_ym());
    };
    let bytes: [u8; 11] = lexical.as_slice().try_into().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar xsd:gYearMonth lexical is not exactly 11 bytes",
        )
    })?;
    let year = (u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]) ^ (1u64 << 63)) as i64;
    let month = bytes[8];
    let offset = (u16::from_be_bytes([bytes[9], bytes[10]]) ^ (1u16 << 15)) as i16;
    if !(1..=12).contains(&month) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar xsd:gYearMonth has an out-of-range month",
        ));
    }
    Ok((year, month, offset))
}

/// Render an output calendar date to the reference CLIENT lexical `YYYY-MM-DD`. Byte-exact to SWI
/// `date_string` (`literals.pl:290`) for `year >= 1` (zero-padded to 4+, `~4+`), month/day `~2+`, and
/// an offset-0 (empty) suffix. Reject-closed (fail closed) on `year < 1` — a negative/zero year's SWI
/// column-fill rendering (or the offset suffix) is not reproduced here rather than guessed.
pub(crate) fn cal_date_result_object(year: i64, month: u8, day: u8) -> std::io::Result<CanonicalObject> {
    if year < 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date result year < 1 is out of the byte-exact-rendered range (reject closed)",
        ));
    }
    Ok(CanonicalObject::Value {
        datatype: CanonicalDatatype::Date,
        lexical: format!("{year:04}-{month:02}-{day:02}").into_bytes(),
    })
}

/// VERIFIER RECOMPUTE for the INTEGER-result calendar ops (`Weekday`/`WeekdaySundayStart`/`IsoWeek`),
/// the calendar analogue of `expected_eval_columns`. Carry the child columns through unchanged and
/// append ONE derived `GeneratedDecimalI64` column PER `outputs` entry (1 for Weekday/WeekdaySundayStart,
/// 2 for IsoWeek's `[year, week]`), each re-derived elementwise from the ROOT-authenticated `xsd:date`
/// operand column. A non-date operand row, an `year < 1` input, or a corrupt lexical FAILS CLOSED.
pub(crate) fn expected_calendar_int_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    date_column: usize,
    outputs: &[CalendarIntFn],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let operand = values.get(date_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column out of range",
        )
    })?;
    if operand.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column length mismatch",
        ));
    }
    let mut columns = input.result_columns.clone();
    let mut derived: Vec<Vec<ArkFr>> = outputs
        .iter()
        .map(|_| Vec::with_capacity(input.result_len))
        .collect();
    for row in 0..input.result_len {
        let (year, month, day, _offset) = cal_decode_date_operand(&operand[row])?;
        // Weekday/IsoWeek ignore the timezone offset entirely (the reference `extract_ymd` uses only
        // Y/M/D), so any offset is fine; but the year must be >= 1 for the ported algorithms' `//`.
        if year < 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar integer op requires an input year >= 1 (reject closed)",
            ));
        }
        // Compute the ISO week ONCE per row (both components share it); fail closed on a corrupt month.
        let iso_week = if outputs
            .iter()
            .any(|f| matches!(f, CalendarIntFn::IsoWeekYear | CalendarIntFn::IsoWeekWeek))
        {
            Some(cal_iso_week_date(year, month, day).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL calendar iso_week has a corrupt operand month",
                )
            })?)
        } else {
            None
        };
        for (index, function) in outputs.iter().enumerate() {
            let value: i64 = match function {
                CalendarIntFn::Weekday => cal_iso_weekday_number(year, month, day),
                CalendarIntFn::WeekdaySundayStart => cal_us_weekday(year, month, day),
                CalendarIntFn::IsoWeekYear => iso_week.expect("iso_week computed above").0,
                CalendarIntFn::IsoWeekWeek => iso_week.expect("iso_week computed above").1,
            };
            derived[index].push(generated_decimal_i64_field(value));
        }
    }
    columns.extend(derived);
    Ok(columns)
}

/// VERIFIER RECOMPUTE for the DATE-result calendar ops (`DayAfter`/`DayBefore`/`MonthStartDate`/
/// `MonthEndDate`), the calendar analogue of `expected_rational_arith_objects`: a per-row TERMINAL,
/// REVEALED `xsd:date` value re-derived from the ROOT-authenticated operand (`xsd:date` for `Day*`,
/// `xsd:gYearMonth` for `Month*`). FAILS CLOSED on a wrong-typed operand, a non-zero timezone offset,
/// an `year < 1` input/output, or a corrupt lexical — a proof is only ever produced for rows whose
/// exact reference result this reproduces byte-for-byte, and the delivered value must equal it.
pub(crate) fn expected_calendar_date_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    op: CalendarDateFn,
    input_column: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let operand = values.get(input_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column out of range",
        )
    })?;
    if operand.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column length mismatch",
        ));
    }
    let nonzero_offset = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date op requires a zero timezone offset (a ±HH:MM suffix is not reproduced; reject closed)",
        )
    };
    let low_year = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date op requires year >= 1 (reject closed)",
        )
    };
    let corrupt = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date op operand has a corrupt month",
        )
    };
    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let (year, month, day) = match op {
            CalendarDateFn::DayAfter | CalendarDateFn::DayBefore => {
                let (year, month, day, offset) = cal_decode_date_operand(&operand[row])?;
                // The reference PRESERVES the offset into the output date; we only render offset 0.
                if offset != 0 {
                    return Err(nonzero_offset());
                }
                if year < 1 {
                    return Err(low_year());
                }
                match op {
                    CalendarDateFn::DayAfter => cal_day_after(year, month, day).ok_or_else(corrupt)?,
                    _ => cal_day_before(year, month, day).ok_or_else(corrupt)?,
                }
            }
            CalendarDateFn::MonthStartDate | CalendarDateFn::MonthEndDate => {
                let (year, month, offset) = cal_decode_gyearmonth_operand(&operand[row])?;
                if offset != 0 {
                    return Err(nonzero_offset());
                }
                if year < 1 {
                    return Err(low_year());
                }
                let day = match op {
                    CalendarDateFn::MonthStartDate => 1u8,
                    _ => cal_days_in_month(year, month).ok_or_else(corrupt)?,
                };
                (year, month, day)
            }
        };
        out.push(cal_date_result_object(year, month, day)?);
    }
    Ok(out)
}

/// The exact XSD value range `[min, max]` (as `i128`) and signedness of a fixed-width integer
/// datatype, or `None` for any non-fixed-width-integer datatype. These bounds are the SAME
/// ones the Prolog narrowing typecast enforces (`is_byte`/`is_short`/… in `util/types.pl`), so
/// a value outside the range is exactly what makes Prolog `typecast` throw `casting_error`.
pub(crate) fn integer_datatype_bounds(datatype: CanonicalDatatype) -> Option<(i128, i128, bool)> {
    use CanonicalDatatype::*;
    Some(match datatype {
        Int8 => (i128::from(i8::MIN), i128::from(i8::MAX), true),
        Int16 => (i128::from(i16::MIN), i128::from(i16::MAX), true),
        Int32 => (i128::from(i32::MIN), i128::from(i32::MAX), true),
        Int64 => (i128::from(i64::MIN), i128::from(i64::MAX), true),
        UInt8 => (0, i128::from(u8::MAX), false),
        UInt16 => (0, i128::from(u16::MAX), false),
        UInt32 => (0, i128::from(u32::MAX), false),
        UInt64 => (0, i128::from(u64::MAX), false),
        _ => return None,
    })
}

/// The generated result namespace a fixed-width integer TARGET type is delivered in: signed
/// targets use the signed generated domain, unsigned targets the unsigned one (an unsigned
/// target can hold values above `i64::MAX`). Fails closed for a non-integer datatype.
pub(crate) fn integer_target_namespace(datatype: CanonicalDatatype) -> std::io::Result<ResultIdNamespace> {
    match integer_datatype_bounds(datatype) {
        Some((_, _, true)) => Ok(ResultIdNamespace::GeneratedDecimalI64),
        Some((_, _, false)) => Ok(ResultIdNamespace::GeneratedDecimalU64),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast target is not a fixed-width integer datatype",
        )),
    }
}

/// VERIFIER RECOMPUTE for WOQL `Typecast` of one integer column to a fixed-width integer target:
/// carry the child columns through unchanged and append the derived cast column, re-derived
/// elementwise from the ROOT-AUTHENTICATED source column.
///
/// SOUNDNESS: `resolve_result_values` binds every child column to the trusted root (same
/// authentication Eval/Sum use); the source value is decoded via `eval_operand_i128`, which
/// also re-checks the pinned `source_datatype` against the authenticated dictionary value. An
/// integer→integer cast preserves the numeric value, so the recompute is: assert the value lies
/// within the target type's exact XSD range and re-emit it in the target's generated namespace.
/// A value outside the range FAILS CLOSED — matching Prolog `typecast`, which THROWS
/// `casting_error` on an out-of-range narrowing rather than wrapping/truncating. The caller
/// compares the delivered result column against this recomputation, so there is no biasable
/// witness; the passed-through child columns are pinned by the outer equality + `verify_columns`.
pub(crate) fn expected_typecast_column(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    source_column: usize,
    source_datatype: CanonicalDatatype,
    target_datatype: CanonicalDatatype,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    // Authenticate EVERY child column against the trusted root (ties the source to the root).
    let values = resolve_result_values(layer, layer.resolution_root(), &input.result_columns, &descriptors)?;
    if input.result_columns.iter().any(|column| column.len() != input.result_len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast child column length mismatch",
        ));
    }
    let (lo, hi, signed) = integer_datatype_bounds(target_datatype).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast target is not a fixed-width integer datatype",
        )
    })?;
    let source = values.get(source_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast source column out of range",
        )
    })?;
    // Pass the child columns through unchanged; the outer equality check pins them.
    let mut columns = input.result_columns.clone();
    let mut derived = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let value = eval_operand_i128(&source[row], source_datatype)?;
        // Range check = Prolog's narrowing `casting_error` throw: FAIL CLOSED (never wrap) when
        // the authenticated source value does not fit the target type's exact XSD range.
        if value < lo || value > hi {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Typecast value is outside the target integer type's range (Prolog throws casting_error)",
            ));
        }
        // In-range: the value is unchanged (lossless identity), re-emitted in the target's
        // generated namespace. The `try_from`s cannot fail after the range check (the range is
        // a subset of i64/u64), but are kept as a defensive second gate.
        let field = if signed {
            generated_decimal_i64_field(i64::try_from(value).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Typecast signed value exceeds i64",
                )
            })?)
        } else {
            ArkFr::from_u64(u64::try_from(value).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Typecast unsigned value exceeds u64",
                )
            })?)
        };
        derived.push(field);
    }
    columns.push(derived);
    Ok(columns)
}

/// Resolve EVERY child result column through the ROOT-authenticated node/value dictionary,
/// returning the per-row canonical values. This is the exact authentication `Eval`/`Sum`/
/// `Typecast` use ([`resolve_result_values`]): it pins `commitment_root == expected_root`,
/// re-validates the root-bound dictionary, and decodes each committed id to its store value.
/// Every child value is therefore the store's root-derived value, with no prover latitude.
pub(crate) fn resolve_authenticated_child_values(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<CanonicalResultValue>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(layer, layer.resolution_root(), &input.result_columns, &descriptors)?;
    if input
        .result_columns
        .iter()
        .any(|column| column.len() != input.result_len)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string-op child column length mismatch",
        ));
    }
    Ok(values)
}

/// Extract the per-row authenticated xsd:string BYTES of one already-resolved child column.
///
/// This is the BYTE-AUTHENTICATION FOUNDATION shared by the whole string-op family (concat
/// now; upper/lower/trim/substring later): given the root-authenticated child values (from
/// `resolve_authenticated_child_values`), require column `column` to be a nonnull xsd:string
/// VALUE in EVERY row and return its lexical bytes. FAILS CLOSED on a node IRI, a non-string
/// typed value (e.g. an integer/langString — which `interpolate_string` would STRINGIFY, an
/// out-of-scope circuit), a null, or an out-of-range column index. Because the values are
/// root-authenticated, the returned bytes are exactly the store's root-derived string content:
/// there is no biasable witness, matching the arithmetic circuits' soundness model.
pub(crate) fn authenticate_string_column(
    values: &[Vec<CanonicalResultValue>],
    column: usize,
    result_len: usize,
) -> std::io::Result<Vec<Vec<u8>>> {
    let resolved = values.get(column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string-op operand column out of range",
        )
    })?;
    if resolved.len() != result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string-op operand column length mismatch",
        ));
    }
    resolved
        .iter()
        .map(|value| match value {
            CanonicalResultValue::Object(CanonicalObject::Value {
                datatype: CanonicalDatatype::String,
                lexical,
            }) => Ok(lexical.clone()),
            // A node IRI, a non-string value, or a null is NOT a provable string operand in v1.
            // SWI `interpolate_string` would STRINGIFY a non-string (number/atom) via
            // `term_to_atom`/`atom_string`; that is a separate stringification circuit, so we
            // REJECT CLOSED here rather than guess the rendering.
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL string-op operand row is not an authenticated xsd:string value",
            )),
        })
        .collect()
}

/// VERIFIER RECOMPUTE for WOQL `concat` — the byte analogue of `expected_eval_columns`.
///
/// Re-derives the per-row concatenation from ROOT-AUTHENTICATED operand columns (+ trusted
/// query-literal bytes) by appending each element's bytes in LIST ORDER with NO separator —
/// exactly `interpolate_string/2`'s left-to-right `string_concat` over string elements. The
/// result is returned as revealed `xsd:string` [`CanonicalObject`]s (a TERMINAL value: not a
/// committed column). SOUNDNESS: the operand bytes are pinned to the trusted root via
/// `resolve_authenticated_child_values`/`authenticate_string_column` and byte concatenation
/// is deterministic, so the caller's exact-match check against the delivered result column
/// leaves no biasable witness. FAILS CLOSED (never stringifies) on any non-string operand row.
pub(crate) fn expected_concat_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    elements: &[ConcatElement],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    // Authenticate each referenced string column ONCE (root-bound bytes), keyed by column index.
    let mut column_bytes: std::collections::BTreeMap<usize, Vec<Vec<u8>>> =
        std::collections::BTreeMap::new();
    for element in elements {
        if let ConcatElement::Column { index } = element {
            if !column_bytes.contains_key(index) {
                column_bytes.insert(
                    *index,
                    authenticate_string_column(&values, *index, input.result_len)?,
                );
            }
        }
    }
    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let mut lexical: Vec<u8> = Vec::new();
        for element in elements {
            match element {
                // A trusted query literal contributes its bytes verbatim (no separator).
                ConcatElement::Literal(bytes) => lexical.extend_from_slice(bytes),
                // A root-authenticated string column contributes its row's content bytes.
                ConcatElement::Column { index } => {
                    lexical.extend_from_slice(&column_bytes[index][row]);
                }
            }
        }
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical,
        });
    }
    Ok(out)
}

/// Interpose `separator` between consecutive `elements`, exactly as `utils:intersperse/3`
/// (`utils.pl:227`): `[] -> []`, `[e0] -> [e0]`, `[e0, e1, ...] -> [e0, Sep, e1, Sep, ...]`.
/// The separator is emitted ONLY between adjacent elements (never leading/trailing, never for a
/// single-element list). The resulting `ConcatElement` sequence is then concatenated verbatim,
/// reproducing `interpolate_string/2` over `intersperse`'s output = `utils:join/3`.
pub(crate) fn interpose_join_elements(
    elements: &[ConcatElement],
    separator: &ConcatElement,
) -> Vec<ConcatElement> {
    let mut interposed = Vec::with_capacity(elements.len().saturating_mul(2));
    for (position, element) in elements.iter().enumerate() {
        if position > 0 {
            interposed.push(separator.clone());
        }
        interposed.push(element.clone());
    }
    interposed
}

/// VERIFIER RECOMPUTE for WOQL `join` — `Concat`'s byte recompute over the INTERPOSED element
/// sequence.
///
/// FIDELITY: `utils:join/3` = `interpolate_string(intersperse(Sep, List))`. Since every list
/// element and the separator are xsd:strings, `interpolate_string` concatenates them VERBATIM
/// (its `string(H)` branch — no `term_to_atom`/`atom_string` rendering), so joining is precisely
/// concatenating `intersperse(Sep, List)`. We therefore build the interposed sequence
/// (`interpose_join_elements`) and delegate to `expected_concat_objects` — the SAME
/// root-authenticating byte recompute the concat circuit uses. SOUNDNESS is inherited verbatim:
/// operand bytes are pinned to the trusted root, a non-string operand row FAILS CLOSED (never
/// stringified), and byte concatenation is deterministic.
pub(crate) fn expected_join_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    elements: &[ConcatElement],
    separator: &ConcatElement,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let interposed = interpose_join_elements(elements, separator);
    expected_concat_objects(input, input_plan, &interposed, layer, expected_root)
}

/// VERIFIER RECOMPUTE for WOQL `sub_string` — the per-row, CHARACTER-based substring.
///
/// FIDELITY (SWI builtin `sub_string/5`, character-based over code points): decode the
/// root-authenticated string column bytes as UTF-8 (a non-UTF-8 lexical FAILS CLOSED — xsd:string
/// is UTF-8, so this never occurs for a faithful store, and if it did we refuse rather than
/// mis-slice), then take the `length` Unicode scalar values starting at code-point offset
/// `before`. Rust `char` == code point for valid UTF-8, so this equals SWI's substring
/// byte-for-byte. When `before + length` exceeds the row's CHARACTER length, SWI `sub_string`
/// FAILS (drops the row) — we FAIL CLOSED here, so a proof is only produced when NO row is
/// dropped, in which case this per-row map equals the reference result exactly (never a
/// fabricated out-of-range substring). `before`/`length` are trusted compile-time constants.
///
/// SOUNDNESS: the operand bytes are pinned to the trusted root via
/// `resolve_authenticated_child_values`/`authenticate_string_column`; `before`/`length` are
/// bound into the query digest; char-slicing is deterministic — the caller's exact-match check
/// leaves no biasable witness.
pub(crate) fn expected_substring_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    string_column: usize,
    before: usize,
    length: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let column_bytes = authenticate_string_column(&values, string_column, input.result_len)?;
    let mut out = Vec::with_capacity(input.result_len);
    for row_bytes in &column_bytes {
        let text = std::str::from_utf8(row_bytes).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring operand row is not valid UTF-8",
            )
        })?;
        let total_chars = text.chars().count();
        // Non-negative already (validated at compile); `before + length` cannot overflow for any
        // realistic string but guard anyway. Out of range => reference `sub_string` FAILS =>
        // FAIL CLOSED (never fabricate a substring for a row the reference would drop).
        let end = before.checked_add(length).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring before+length overflow",
            )
        })?;
        if end > total_chars {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring before+length exceeds operand character length (out of range: reference sub_string fails closed)",
            ));
        }
        let sub: String = text.chars().skip(before).take(length).collect();
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: sub.into_bytes(),
        });
    }
    Ok(out)
}

/// VERIFIER RECOMPUTE for WOQL `pad` — `utils:pad/4` (utils.pl:320) over a root-authenticated
/// xsd:string column, LEFT-padding each row to `target_len` CODE POINTS with `pad_char`.
///
/// FIDELITY: `pad_char` is a single code point (validated at compile), stored as its exact UTF-8
/// bytes; `L2 = target_len - char_count(text)`; prepending `pad_char`'s bytes `L2` times then the
/// operand bytes reproduces `repeat_term`/`append` byte-for-byte. When `char_count(text) >
/// target_len` (`L2 < 0`) the reference `utils:pad` THROWS `bad_pad_length` (the row has no
/// solution) — we FAIL CLOSED at recompute rather than fabricate a truncated result, so a proof is
/// only ever produced when NO row is dropped, in which case the per-row map equals the reference
/// exactly. A non-UTF-8 operand row also fails closed.
pub(crate) fn expected_pad_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    string_column: usize,
    pad_char: &[u8],
    target_len: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let column_bytes = authenticate_string_column(&values, string_column, input.result_len)?;
    let mut out = Vec::with_capacity(input.result_len);
    for row_bytes in &column_bytes {
        let text = std::str::from_utf8(row_bytes).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Pad operand row is not valid UTF-8",
            )
        })?;
        // `string_length` counts CODE POINTS; `chars()` yields Unicode scalar values (= code
        // points for valid UTF-8), so this equals SWI's char length.
        let char_count = text.chars().count();
        if char_count > target_len {
            // `L2 < 0` => reference `utils:pad` throws `bad_pad_length` => FAIL CLOSED.
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Pad operand is longer than the pad target length (bad_pad_length: reference pad throws)",
            ));
        }
        let pad_count = target_len - char_count; // >= 0, and pad_char is exactly one code point
        let mut lexical: Vec<u8> = Vec::with_capacity(pad_char.len() * pad_count + row_bytes.len());
        for _ in 0..pad_count {
            lexical.extend_from_slice(pad_char); // PREPEND the padding (append(Padding, A, Total))
        }
        lexical.extend_from_slice(row_bytes);
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical,
        });
    }
    Ok(out)
}

/// `true` iff byte `b` is passed through UNENCODED by the reference `encode_id_fragment`
/// (`src/core/document/json.pl:191` = SWI `uri_encoded(segment, ...)`), EXCLUDING `+` (which
/// `encode_id_fragment` subsequently rewrites to `%2B`). The exact set was captured from the
/// reference's own SWI `uri_encoded/3` builtin (all 95 printable ASCII probed): unreserved plus a
/// subset of `sub-delims`/`pchar` (`! $ & ' ( ) * , - . ; = @ _ ~`) plus alphanumerics. Every
/// passthrough byte is single-byte ASCII, so operating BYTE-WISE is exactly equivalent to SWI's
/// per-code-point encoding: any non-ASCII byte (>= 0x80, i.e. every byte of a multi-byte UTF-8
/// sequence) is >= 0x80, never collides with a passthrough byte, and is percent-encoded — matching
/// SWI's "encode to UTF-8 then percent-encode each byte" (verified: `é` -> `%C3%A9`).
pub(crate) fn is_idgen_passthrough_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'!' | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b','
                | b'-'
                | b'.'
                | b';'
                | b'='
                | b'@'
                | b'_'
                | b'~'
        )
}

/// Reproduce the reference `encode_id_fragment/2` (`json.pl:191`) for a plain string operand:
/// `format(atom(F),'~w',[Elt])` (identity on an xsd:string's content) then `uri_encoded(segment,
/// F, P)` then `re_replace('+' -> '%2B', P)`. Passthrough bytes are emitted verbatim; `+` becomes
/// `%2B`; every other byte becomes `%XX` with UPPERCASE hex (matching SWI). The output is ALWAYS
/// pure ASCII (passthrough bytes are ASCII; encoded bytes are `%` + ASCII hex).
pub(crate) fn encode_id_fragment(bytes: &[u8]) -> Vec<u8> {
    const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if is_idgen_passthrough_byte(b) {
            out.push(b);
        } else if b == b'+' {
            // uri_encoded(segment) leaves '+' as-is; encode_id_fragment then rewrites it to %2B.
            out.extend_from_slice(b"%2B");
        } else {
            out.push(b'%');
            out.push(HEX_UPPER[(b >> 4) as usize]);
            out.push(HEX_UPPER[(b & 0x0f) as usize]);
        }
    }
    out
}

/// Resolve one `ConcatElement`'s bytes for a given row: a trusted literal's bytes, or the
/// root-authenticated string column's row content (pre-authenticated into `column_bytes`).
pub(crate) fn concat_element_row_bytes<'a>(
    element: &'a ConcatElement,
    column_bytes: &'a std::collections::BTreeMap<usize, Vec<Vec<u8>>>,
    row: usize,
) -> &'a [u8] {
    match element {
        ConcatElement::Literal(bytes) => bytes,
        ConcatElement::Column { index } => &column_bytes[index][row],
    }
}

/// VERIFIER RECOMPUTE for WOQL `hash`/`HashKey` — the exact `idgen_hash/3` recipe (json.pl:467)
/// over root-authenticated `base` + `args`:
///   suffix = args.map(encode_id_fragment).join("+");  hash = lowercase_hex(sha256(suffix));
///   id = base ++ hash.
///
/// FIDELITY: `base` is RAW-prepended (never `idgen`-encoded — `format("~w~w",[Base,Hash])`); each
/// `arg` is `encode_id_fragment`'d (NO `raw/2` transform, since `literally` already reduced every
/// operand to a plain string — `raw/2` is identity on non-dicts, so no set-sorting applies);
/// fragments are joined by a literal `'+'` (`merge_separator_split(_, '+', _)`); the hash is the
/// lowercase 64-char hex SHA-256 of the (pure-ASCII) suffix bytes; and the digest is appended to
/// base. Every byte is authenticated against the trusted root (columns) or comes from the trusted
/// compiled query (literals); the id is revealed as a `CanonicalObject::Node` (the reference binds
/// a `NodeValue`/URI).
pub(crate) fn expected_hashkey_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    base: &ConcatElement,
    args: &[ConcatElement],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    // Authenticate each referenced string column (base's and args') ONCE, keyed by column index.
    let mut column_bytes: std::collections::BTreeMap<usize, Vec<Vec<u8>>> =
        std::collections::BTreeMap::new();
    let mut authenticate = |element: &ConcatElement| -> std::io::Result<()> {
        if let ConcatElement::Column { index } = element {
            if !column_bytes.contains_key(index) {
                column_bytes.insert(
                    *index,
                    authenticate_string_column(&values, *index, input.result_len)?,
                );
            }
        }
        Ok(())
    };
    authenticate(base)?;
    for arg in args {
        authenticate(arg)?;
    }

    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        // Build the idgen suffix: encode each arg fragment, join by '+'. Pure ASCII by construction.
        let mut suffix: Vec<u8> = Vec::new();
        for (position, arg) in args.iter().enumerate() {
            if position > 0 {
                suffix.push(b'+');
            }
            let arg_bytes = concat_element_row_bytes(arg, &column_bytes, row);
            suffix.extend_from_slice(&encode_id_fragment(arg_bytes));
        }
        // sha256 of the suffix bytes, lowercase hex (== SWI crypto_data_hash(..,[algorithm(sha256)])).
        let digest_hex = hex::encode(Sha256::digest(&suffix));
        // id = raw base ++ hex digest.
        let base_bytes = concat_element_row_bytes(base, &column_bytes, row);
        let mut id: Vec<u8> = Vec::with_capacity(base_bytes.len() + digest_hex.len());
        id.extend_from_slice(base_bytes);
        id.extend_from_slice(digest_hex.as_bytes());
        out.push(CanonicalObject::Node(id));
    }
    Ok(out)
}

/// The single generated-decimal COUNT column a `CollectionCount` / empty-key `GroupCount` delivers:
/// one row holding `result_len`. `emit_zero_for_empty` chooses whether an empty collection yields a
/// `0` row or no row (matching the WOQL aggregate's empty-input behaviour).
pub(crate) fn expected_cardinality_column(
    result_len: usize,
    emit_zero_for_empty: bool,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    if result_len == 0 && !emit_zero_for_empty {
        return Ok(vec![Vec::new()]);
    }
    Ok(vec![vec![ArkFr::from_u64(u64::try_from(result_len).map_err(
        |_| std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection cardinality exceeds u64",
        ),
    )?)]])
}

/// The single generated-decimal SUM column a [`CompiledRelationStatement::CollectionSum`] delivers:
/// sum the `template` value (a child column or a constant) over every child row in the `datatype`'s
/// exact integral domain. Re-run by the verifier; a wrong-typed operand fails closed.
pub(crate) fn expected_collection_sum_column(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    template: CollectionSumTemplate,
    datatype: CanonicalDatatype,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let mut sum = GroupSumValue::zero(datatype)?;
    match template {
        CollectionSumTemplate::Column(column) => {
            let descriptors = relation_result_descriptors(input_plan)?;
            let values = resolve_result_values(
                layer,
                layer.resolution_root(),
                &input.result_columns,
                &descriptors,
            )?;
            let values = values.get(column).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "collection-sum value column out of range",
                )
            })?;
            if values.len() != input.result_len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "collection-sum input column length mismatch",
                ));
            }
            for value in values {
                sum = sum.checked_add(integral_group_sum_value(value, datatype)?)?;
            }
        }
        CollectionSumTemplate::UnsignedConstant(value) => {
            for _ in 0..input.result_len {
                sum = sum.checked_add(GroupSumValue::Unsigned(value))?;
            }
        }
        CollectionSumTemplate::SignedConstant(value) => {
            for _ in 0..input.result_len {
                sum = sum.checked_add(GroupSumValue::Signed(i128::from(value)))?;
            }
        }
    }
    Ok(vec![vec![sum.field()?]])
}

/// The single MIN/MAX (`kind`) column a [`CompiledRelationStatement::CollectionExtrema`] delivers:
/// the extremum of `value_column` over every child row under the order-preserving `datatype`.
/// Re-run by the verifier.
pub(crate) fn expected_collection_extrema_column(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    value_column: usize,
    datatype: CanonicalDatatype,
    kind: GroupExtremaKind,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(
        layer,
        layer.resolution_root(),
        &input.result_columns,
        &descriptors,
    )?;
    let values = values.get(value_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection-extrema value column out of range",
        )
    })?;
    let fields = input.result_columns.get(value_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection-extrema field column out of range",
        )
    })?;
    if values.len() != input.result_len || fields.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection-extrema input column length mismatch",
        ));
    }
    let mut selected: Option<(ArkFr, GroupSumValue)> = None;
    for (field, value) in fields.iter().zip(values) {
        let numeric = integral_group_sum_value(value, datatype)?;
        let replace = match selected {
            None => true,
            Some((_, current)) => match kind {
                GroupExtremaKind::Min => numeric.cmp_same_domain(current)?.is_lt(),
                GroupExtremaKind::Max => numeric.cmp_same_domain(current)?.is_gt(),
            },
        };
        if replace {
            selected = Some((*field, numeric));
        }
    }
    Ok(vec![selected.map(|(field, _)| vec![field]).unwrap_or_default()])
}

