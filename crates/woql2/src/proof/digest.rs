//! Query digest — a stable fingerprint of a compiled plan.
//!
//! [`compiled_query_digest`] folds a [`CompiledBgp`] (its schema, every plan node, and all
//! embedded constants) into a single SHA-256 [`ProofHash`]. The digest is what binds a
//! proof to the EXACT query it answers: the verifier recompiles the query independently,
//! digests it, and checks the proof commits to the same digest — so a proof for one query
//! cannot be replayed as if it answered another. Nothing here proves anything; it only
//! hashes plan structure deterministically (the `digest_*` helpers walk each plan/expr node).

use super::*;

pub(crate) fn digest_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Bundle-identity digest for a compiled `ReadDocument` (increment 2b).
///
/// A `ReadDocument` proof is a BUNDLE of ordinary sub-proofs (one instance bound-subject BGP, the
/// schema-graph frame BGPs, and one ordered-list walk per list property). Each sub-proof already
/// carries its OWN [`compiled_query_digest`] that binds it to its exact `(subject, predicate)` /
/// closure query, so this fold does not re-hash the sub-plans. It only pins the ONE thing that is a
/// pure function of the surface query — the bound document `id` — under a distinct domain tag, so a
/// bundle cannot be re-aimed at a different document. The class `C`, property set, and list count are
/// DATA (discovered at prove, re-authenticated at verify from the root-bound frame) and deliberately
/// NOT folded here.
pub fn read_document_digest(id: &[u8]) -> ProofHash {
    let mut hasher = Sha256::new();
    hasher.update(b"terminusdb-woql2/read-document/v1");
    digest_bytes(&mut hasher, id);
    hasher.finalize().into()
}

/// Stable identity of all public WOQL interpretation metadata layered over the Store
/// plan. This prevents reusing one envelope under a different schema, projection,
/// collection, count binding, or `Distinct` interpretation.
pub fn compiled_query_digest(compiled: &CompiledBgp) -> std::io::Result<ProofHash> {
    let mut hasher = Sha256::new();
    hasher.update(b"terminusdb-woql2/query-proof-envelope/v2");
    hasher.update(relation_query_digest(&compiled.relation)?);
    match &compiled.projection {
        None => hasher.update([0]),
        Some(indices) => {
            hasher.update([1]);
            hasher.update((indices.len() as u64).to_le_bytes());
            for index in indices {
                hasher.update((*index as u64).to_le_bytes());
            }
        }
    }
    for value in [&compiled.collection, &compiled.count_variable] {
        match value {
            None => hasher.update([0]),
            Some(value) => {
                hasher.update([1]);
                digest_bytes(&mut hasher, value.as_bytes());
            }
        }
    }
    hasher.update([compiled.full_row_distinct as u8]);
    Ok(hasher.finalize().into())
}

pub(crate) fn relation_query_digest(relation: &CompiledRelation) -> std::io::Result<ProofHash> {
    if relation.domains.len() != relation.schema.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL relation domain metadata shape mismatch",
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"terminusdb-woql2/relation/v1");
    hasher.update((relation.schema.len() as u64).to_le_bytes());
    for name in &relation.schema {
        digest_bytes(&mut hasher, name.as_bytes());
    }
    hasher.update((relation.domains.len() as u64).to_le_bytes());
    for domain in &relation.domains {
        digest_equality_domain(&mut hasher, *domain);
    }
    for descriptor in relation_result_descriptors(relation)? {
        hasher.update([match descriptor.namespace {
            ResultIdNamespace::NodeValue => 0,
            ResultIdNamespace::Predicate => 1,
            ResultIdNamespace::GeneratedDecimalU64 => 2,
            ResultIdNamespace::GeneratedDecimalI64 => 3,
        }]);
        hasher.update([descriptor.nullable as u8]);
    }
    match &relation.statement {
        CompiledRelationStatement::LayerTripleCount => {
            hasher.update(b"/layer-triple-count");
        }
        CompiledRelationStatement::GeneratedU64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => {
            hasher.update(b"/generated-u64-filter");
            hasher.update(relation_query_digest(input)?);
            hasher.update(column.to_le_bytes());
            hasher.update([match op {
                GeneratedU64ComparisonOp::Eq => 0,
                GeneratedU64ComparisonOp::Lt => 1,
                GeneratedU64ComparisonOp::Le => 2,
                GeneratedU64ComparisonOp::Gt => 3,
                GeneratedU64ComparisonOp::Ge => 4,
            }]);
            hasher.update(constant.to_le_bytes());
            hasher.update([*constant_on_left as u8]);
        }
        CompiledRelationStatement::GeneratedI64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => {
            hasher.update(b"/generated-i64-filter");
            hasher.update(relation_query_digest(input)?);
            hasher.update(column.to_le_bytes());
            hasher.update([match op {
                GeneratedU64ComparisonOp::Eq => 0,
                GeneratedU64ComparisonOp::Lt => 1,
                GeneratedU64ComparisonOp::Le => 2,
                GeneratedU64ComparisonOp::Gt => 3,
                GeneratedU64ComparisonOp::Ge => 4,
            }]);
            hasher.update(constant.to_le_bytes());
            hasher.update([*constant_on_left as u8]);
        }
        CompiledRelationStatement::Bgp(plan) => {
            hasher.update(b"/bgp");
            hasher.update(plan_digest(plan)?);
        }
        CompiledRelationStatement::BagUnion { branches, mappings } => {
            hasher.update(b"/bag-union");
            hasher.update((branches.len() as u64).to_le_bytes());
            for (branch, mapping) in branches.iter().zip(mappings) {
                hasher.update(relation_query_digest(branch)?);
                hasher.update((mapping.len() as u64).to_le_bytes());
                for index in mapping {
                    hasher.update((*index as u64).to_le_bytes());
                }
            }
        }
        CompiledRelationStatement::BagProjection { input, indices } => {
            hasher.update(b"/bag-projection");
            hasher.update(relation_query_digest(input)?);
            hasher.update((indices.len() as u64).to_le_bytes());
            for index in indices {
                hasher.update((*index as u64).to_le_bytes());
            }
        }
        CompiledRelationStatement::Distinct { input, key_columns } => {
            hasher.update(b"/distinct-unordered");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for column in key_columns {
                hasher.update((*column as u64).to_le_bytes());
            }
        }
        CompiledRelationStatement::When { expanded } => {
            hasher.update(b"/when");
            hasher.update(relation_query_digest(expanded)?);
        }
        CompiledRelationStatement::LeftAnti {
            left,
            right,
            key_pairs,
        } => {
            hasher.update(b"/left-anti");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            digest_key_pairs(&mut hasher, key_pairs);
        }
        CompiledRelationStatement::InnerJoin { left, right, key_pairs } => {
            hasher.update(b"/inner-join");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            digest_key_pairs(&mut hasher, key_pairs);
        }
        CompiledRelationStatement::CrossRootHashJoin { left, right, left_key_col, right_key_col } => {
            hasher.update(b"/cross-root-hash-join");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            hasher.update((*left_key_col as u64).to_le_bytes());
            hasher.update((*right_key_col as u64).to_le_bytes());
        }
        CompiledRelationStatement::LeftOuter {
            left,
            right,
            key_pairs,
        } => {
            hasher.update(b"/left-outer");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            digest_key_pairs(&mut hasher, key_pairs);
        }
        CompiledRelationStatement::Equality { input, constraint } => {
            hasher.update(b"/equality");
            hasher.update(relation_query_digest(input)?);
            digest_equality_plan(&mut hasher, constraint);
        }
        CompiledRelationStatement::TupleDisequality { input, pairs } => {
            hasher.update(b"/tuple-disequality");
            hasher.update(relation_query_digest(input)?);
            digest_tuple_disequality_pairs(&mut hasher, pairs);
        }
        CompiledRelationStatement::ExactScalarOrder { input, datatype, direction } => {
            hasher.update(b"/exact-scalar-order");
            hasher.update(relation_query_digest(input)?);
            hasher.update([*datatype as u8]);
            hasher.update([match direction { OrderDirection::Ascending => 0, OrderDirection::Descending => 1 }]);
        }
        CompiledRelationStatement::ExactMultiOrder { input, key_indices, datatypes, directions } => {
            hasher.update(b"/exact-multi-order"); hasher.update(relation_query_digest(input)?);
            hasher.update((datatypes.len() as u64).to_le_bytes());
            for datatype in datatypes { hasher.update([*datatype as u8]); }
            hasher.update((key_indices.len() as u64).to_le_bytes());
            for (key_index, direction) in key_indices.iter().zip(directions) {
                hasher.update((*key_index as u64).to_le_bytes());
                hasher.update([match direction { OrderDirection::Ascending => 0, OrderDirection::Descending => 1 }]);
            }
        }
        CompiledRelationStatement::ExactOrderedSlice { input, offset, limit, once } => {
            hasher.update(if *once {
                &b"/once-ordered-slice"[..]
            } else {
                &b"/exact-ordered-slice"[..]
            });
            hasher.update(relation_query_digest(input)?);
            hasher.update(offset.to_le_bytes());
            match limit {
                None => hasher.update([0]),
                Some(limit) => {
                    hasher.update([1]);
                    hasher.update(limit.to_le_bytes());
                }
            }
        }
        CompiledRelationStatement::GroupExtrema {
            input,
            key_columns,
            value_column,
            datatype,
            kind,
        } => {
            hasher.update(b"/group-extrema");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for key_column in key_columns {
                hasher.update((*key_column as u64).to_le_bytes());
            }
            hasher.update((*value_column as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
            hasher.update([match kind {
                GroupExtremaKind::Min => 0,
                GroupExtremaKind::Max => 1,
            }]);
        }
        CompiledRelationStatement::TransitiveClosure {
            edge,
            seed,
            reflexive,
            inverted,
            graph,
        } => {
            // K-INDEPENDENT digest: only the closure PATTERN (edge, seed, star/plus, direction). The
            // number of rounds K is a prover choice carried in the proof, never in this identity, so the
            // verifier reconstructs this digest from its own trusted AST without knowing K.
            hasher.update(b"/transitive-closure");
            hasher.update(plan_digest(edge)?);
            digest_canonical_object(&mut hasher, seed);
            hasher.update([*reflexive as u8]);
            hasher.update([*inverted as u8]);
            // SCHEMA-GRAPH FOUNDATION — bind the graph selector ADDITIVELY. `Instance` (every
            // pre-existing path closure) folds NOTHING, so its digest is byte-identical to today; only a
            // `Schema` closure folds a domain-separated tag. This authenticates the read-graph choice
            // (the verifier reconstructs the same tag from the trusted query) without perturbing any
            // instance-only proof identity, and makes a schema closure's identity distinct from an
            // otherwise-identical instance closure so neither can be substituted for the other.
            if let ProofGraph::Schema = graph {
                hasher.update(b"/graph-schema");
            }
        }
        CompiledRelationStatement::OrderedListWalk {
            subject,
            predicate,
            graph,
        } => {
            // K-INDEPENDENT digest: only the walk PATTERN (the bound subject + the list predicate).
            // The number of cons cells K is a prover choice carried in the proof, never in this
            // identity, so the verifier reconstructs the same digest from its own trusted AST without
            // knowing K. Binding (subject, predicate) HERE is what stops a valid walk proof from being
            // re-aimed at a DIFFERENT (subject, predicate) list — the store's verify rejects any proof
            // whose query digest does not match this reconstruction.
            hasher.update(b"/ordered-list-walk");
            digest_canonical_object(&mut hasher, subject);
            // Length-prefix the predicate bytes so no two (subject, predicate) pairs can collide by
            // shifting bytes across the field boundary.
            hasher.update((predicate.len() as u64).to_le_bytes());
            hasher.update(predicate.as_slice());
            // SCHEMA-GRAPH FOUNDATION — bind the graph selector ADDITIVELY, exactly like the closure
            // arm: `Instance` folds NOTHING (byte-identical instance identity), only `Schema` folds a
            // domain tag. OrderedListWalk is always `Instance` in this increment.
            if let ProofGraph::Schema = graph {
                hasher.update(b"/graph-schema");
            }
        }
        CompiledRelationStatement::GroupSum {
            input,
            key_columns,
            value_column,
            datatype,
        } => {
            hasher.update(b"/group-sum");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for key_column in key_columns {
                hasher.update((*key_column as u64).to_le_bytes());
            }
            hasher.update((*value_column as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
        }
        CompiledRelationStatement::GroupCount { input, key_columns } => {
            hasher.update(b"/group-count");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for key_column in key_columns {
                hasher.update((*key_column as u64).to_le_bytes());
            }
        }
        CompiledRelationStatement::CollectionCount { input } => {
            hasher.update(b"/collection-count");
            hasher.update(relation_query_digest(input)?);
        }
        CompiledRelationStatement::CollectionSum {
            input,
            template,
            datatype,
        } => {
            hasher.update(b"/collection-sum");
            hasher.update(relation_query_digest(input)?);
            match template {
                CollectionSumTemplate::Column(column) => {
                    hasher.update([0]);
                    hasher.update((*column as u64).to_le_bytes());
                }
                CollectionSumTemplate::UnsignedConstant(value) => {
                    hasher.update([1]);
                    hasher.update(value.to_le_bytes());
                }
                CollectionSumTemplate::SignedConstant(value) => {
                    hasher.update([2]);
                    hasher.update(value.to_le_bytes());
                }
            }
            hasher.update([*datatype as u8]);
        }
        CompiledRelationStatement::CollectionExtrema {
            input,
            value_column,
            datatype,
            kind,
        } => {
            hasher.update(b"/collection-extrema");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*value_column as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
            hasher.update([match kind {
                GroupExtremaKind::Min => 0,
                GroupExtremaKind::Max => 1,
            }]);
        }
        CompiledRelationStatement::Eval { input, expr } => {
            hasher.update(b"/eval-arithmetic-i64");
            hasher.update(relation_query_digest(input)?);
            digest_eval_expr(&mut hasher, expr);
        }
        CompiledRelationStatement::Typecast {
            input,
            source_column,
            source_datatype,
            target_datatype,
        } => {
            // Bind the source column, its pinned datatype, and the target datatype so a proof
            // for one cast (e.g. int→long) cannot be replayed as another (e.g. long→byte) or
            // over a different source column.
            hasher.update(b"/typecast-integer-width");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*source_column as u64).to_le_bytes());
            hasher.update([*source_datatype as u8]);
            hasher.update([*target_datatype as u8]);
        }
        CompiledRelationStatement::Concat { input, elements, .. } => {
            // Bind the operator kind, the child, and the EXACT element sequence (order +
            // literal-vs-column + literal bytes + column index) so a proof for one concat
            // cannot be replayed as a different string op or a reordered/re-literalled concat.
            // The result variable NAME is not bound (it is a label, not security-relevant — the
            // output is fully determined by the recompute), matching Eval's digest discipline.
            hasher.update(b"/string-concat-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((elements.len() as u64).to_le_bytes());
            for element in elements {
                digest_concat_element(&mut hasher, element);
            }
        }
        CompiledRelationStatement::Join {
            input,
            elements,
            separator,
            ..
        } => {
            // DISTINCT tag from concat so a join can never be replayed as a concat (even a
            // single-element join, whose interposed sequence equals a one-element concat). Bind
            // the child, the exact element sequence, AND the separator under its own `/sep` tag
            // so a proof cannot be replayed as a different join (reordered/re-literalled elements
            // or a swapped separator). The result variable NAME is a label, not bound (the output
            // is fully determined by the recompute), matching Concat/Eval digest discipline.
            hasher.update(b"/string-join-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((elements.len() as u64).to_le_bytes());
            for element in elements {
                digest_concat_element(&mut hasher, element);
            }
            hasher.update(b"/sep");
            digest_concat_element(&mut hasher, separator);
        }
        CompiledRelationStatement::Substring {
            input,
            string_column,
            before,
            length,
            ..
        } => {
            // DISTINCT tag; bind the child, the sliced column, and the constant character offsets
            // so a proof cannot be replayed as a different string op or a different-offset slice.
            hasher.update(b"/string-substring-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*string_column as u64).to_le_bytes());
            hasher.update((*before as u64).to_le_bytes());
            hasher.update((*length as u64).to_le_bytes());
        }
        CompiledRelationStatement::Pad {
            input,
            string_column,
            pad_char,
            target_len,
            ..
        } => {
            // DISTINCT tag; bind the child, the padded column, the exact pad-char bytes, and the
            // target length so a proof cannot be replayed as a different string op or a
            // different-char/different-length pad.
            hasher.update(b"/string-pad-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*string_column as u64).to_le_bytes());
            hasher.update((*target_len as u64).to_le_bytes());
            digest_bytes(&mut hasher, pad_char);
        }
        CompiledRelationStatement::HashKey {
            input,
            base,
            args,
            ..
        } => {
            // DISTINCT tag; bind the child, the (raw-prepended) base, and the EXACT arg sequence
            // (order + literal-vs-column + literal bytes + column index) so a proof cannot be
            // replayed as a different string op or a reordered/re-based/re-argumented hash.
            hasher.update(b"/string-idgen-hash-sha256");
            hasher.update(relation_query_digest(input)?);
            hasher.update(b"/base");
            digest_concat_element(&mut hasher, base);
            hasher.update((args.len() as u64).to_le_bytes());
            for arg in args {
                digest_concat_element(&mut hasher, arg);
            }
        }
        CompiledRelationStatement::FloorEval { input, expr } => {
            // DISTINCT tag; bind the child + the floored expression so a floor proof cannot be
            // replayed as a different op or a different argument. The result variable NAME is a
            // label (the output is fully determined by the recompute), matching Eval's discipline.
            hasher.update(b"/decimal-floor-i64");
            hasher.update(relation_query_digest(input)?);
            digest_decimal_expr(&mut hasher, expr);
        }
        CompiledRelationStatement::RationalArith { input, expr, .. } => {
            // DISTINCT tag from every other op; bind the child + the EXACT rational expression
            // (operator, operand order, column indices, literal bytes) so a proof cannot be
            // replayed as a different arithmetic. The result variable NAME is a label (the output
            // is fully determined by the recompute), matching Concat/Eval digest discipline.
            hasher.update(b"/decimal-rational-arith");
            hasher.update(relation_query_digest(input)?);
            digest_decimal_expr(&mut hasher, expr);
        }
        CompiledRelationStatement::CalendarInt {
            input,
            date_column,
            outputs,
        } => {
            // DISTINCT tag; bind the child, the authenticated operand column, and the EXACT output
            // function sequence (order + kind) so a proof cannot be replayed as a different calendar
            // op, a different operand column, or a reordered/re-functioned output set. The result
            // variable NAMES are labels (the outputs are fully determined by the recompute), matching
            // Eval's digest discipline; the appended descriptors are already re-hashed above.
            hasher.update(b"/calendar-int-i64");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*date_column as u64).to_le_bytes());
            hasher.update((outputs.len() as u64).to_le_bytes());
            for function in outputs {
                hasher.update([match function {
                    CalendarIntFn::Weekday => 0u8,
                    CalendarIntFn::WeekdaySundayStart => 1u8,
                    CalendarIntFn::IsoWeekYear => 2u8,
                    CalendarIntFn::IsoWeekWeek => 3u8,
                }]);
            }
        }
        CompiledRelationStatement::CalendarDate {
            input,
            op,
            input_column,
            ..
        } => {
            // DISTINCT tag; bind the child, the op kind, and the authenticated operand column so a
            // proof cannot be replayed as a different date op or over a different operand column. The
            // result variable NAME is a label (the output is fully determined by the recompute),
            // matching Concat/RationalArith digest discipline.
            hasher.update(b"/calendar-date");
            hasher.update(relation_query_digest(input)?);
            hasher.update([match op {
                CalendarDateFn::DayAfter => 0u8,
                CalendarDateFn::DayBefore => 1u8,
                CalendarDateFn::MonthStartDate => 2u8,
                CalendarDateFn::MonthEndDate => 3u8,
            }]);
            hasher.update((*input_column as u64).to_le_bytes());
        }
        CompiledRelationStatement::TypecastNumberToString {
            input,
            source_column,
            ..
        } => {
            // DISTINCT tag; bind the child + the authenticated numeric source column so a proof
            // cannot be replayed as a different op or over a different source column. The source
            // DATATYPE is NOT bound (it is read from the root-authenticated dictionary at recompute,
            // like Concat's operand columns); the result variable NAME is a label (the output is
            // fully determined by the recompute), matching Concat/RationalArith digest discipline.
            hasher.update(b"/typecast-num-to-string");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*source_column as u64).to_le_bytes());
        }
        CompiledRelationStatement::TypecastStringToNumber {
            input,
            source_column,
            target,
            ..
        } => {
            // DISTINCT tag; bind the child, the authenticated xsd:string source column, AND the
            // numeric target discriminant so a proof for string→integer cannot be replayed as
            // string→decimal (they parse different grammars and deliver different datatypes). The
            // result variable NAME is a label (output fully determined by the recompute).
            hasher.update(b"/typecast-string-to-num");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*source_column as u64).to_le_bytes());
            hasher.update([match target {
                StringNumericTarget::Integer => 0u8,
                StringNumericTarget::Decimal => 1u8,
            }]);
        }
    }
    Ok(hasher.finalize().into())
}

/// Bind the exact rational arithmetic tree into the relation's query identity so a proof cannot be
/// replayed under a different expression, operand column, literal, or operator/operand order. Each
/// node carries a DISTINCT leading tag (anti-substitution), disjoint from [`digest_eval_expr`]'s
/// tags because this whole statement already lives under a distinct `/decimal-*` statement tag.
pub(crate) fn digest_decimal_expr(hasher: &mut Sha256, expr: &DecimalExpr) {
    match expr {
        DecimalExpr::Column { index } => {
            hasher.update([0u8]);
            hasher.update((*index as u64).to_le_bytes());
        }
        DecimalExpr::IntegerLiteral(value) => {
            hasher.update([1u8]);
            hasher.update(value.to_le_bytes());
        }
        DecimalExpr::DecimalLiteral(lexical) => {
            hasher.update([2u8]);
            digest_bytes(hasher, lexical.as_bytes());
        }
        DecimalExpr::Plus(left, right) => {
            hasher.update([3u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Minus(left, right) => {
            hasher.update([4u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Times(left, right) => {
            hasher.update([5u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Divide(left, right) => {
            hasher.update([6u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Exp(base, exponent) => {
            hasher.update([7u8]);
            digest_decimal_expr(hasher, base);
            digest_decimal_expr(hasher, exponent);
        }
        DecimalExpr::Floor(argument) => {
            hasher.update([8u8]);
            digest_decimal_expr(hasher, argument);
        }
    }
}

/// Bind one concat element into the relation's query identity (anti-substitution): a literal's
/// exact bytes, or a column's exact index, each under a distinct tag so a literal cannot be
/// confused with a column and vice versa.
pub(crate) fn digest_concat_element(hasher: &mut Sha256, element: &ConcatElement) {
    match element {
        ConcatElement::Literal(bytes) => {
            hasher.update([0u8]);
            digest_bytes(hasher, bytes);
        }
        ConcatElement::Column { index } => {
            hasher.update([1u8]);
            hasher.update((*index as u64).to_le_bytes());
        }
    }
}

/// Bind the exact arithmetic tree into the relation's query identity so a proof cannot be
/// replayed under a different expression, operand column, datatype, or literal.
pub(crate) fn digest_eval_expr(hasher: &mut Sha256, expr: &EvalExpr) {
    match expr {
        EvalExpr::Column { index, datatype } => {
            hasher.update([0u8]);
            hasher.update((*index as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
        }
        EvalExpr::Literal(value) => {
            hasher.update([1u8]);
            hasher.update(value.to_le_bytes());
        }
        EvalExpr::Plus(left, right) => {
            hasher.update([2u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        EvalExpr::Minus(left, right) => {
            hasher.update([3u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        EvalExpr::Times(left, right) => {
            hasher.update([4u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        // Distinct tags 5/6 for Div/Exp: a proof for `a div b` (or `a ** b`) cannot be replayed
        // as `a - b`, `a * b`, or with the operands swapped — the exact operator and left/right
        // operand order are bound into the relation's query identity (anti-substitution).
        EvalExpr::Div(left, right) => {
            hasher.update([5u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        EvalExpr::Exp(base, exponent) => {
            hasher.update([6u8]);
            digest_eval_expr(hasher, base);
            digest_eval_expr(hasher, exponent);
        }
    }
}

pub(crate) fn digest_equality_domain(hasher: &mut Sha256, domain: Option<EqualityValueDomain>) {
    match domain {
        None => hasher.update([0]),
        Some(EqualityValueDomain::Predicate) => hasher.update([1]),
        Some(EqualityValueDomain::Node) => hasher.update([2]),
        Some(EqualityValueDomain::Datatype(datatype)) => {
            hasher.update([3]);
            hasher.update([datatype as u8]);
        }
    }
}

pub(crate) fn digest_canonical_object(hasher: &mut Sha256, value: &CanonicalObject) {
    match value {
        CanonicalObject::Node(iri) => {
            hasher.update([0]);
            digest_bytes(hasher, iri);
        }
        CanonicalObject::Value { datatype, lexical } => {
            hasher.update([1, *datatype as u8]);
            digest_bytes(hasher, lexical);
        }
    }
}

pub(crate) fn digest_equality_plan(hasher: &mut Sha256, plan: &RelationEqualityPlan) {
    match plan {
        RelationEqualityPlan::ColumnConstant {
            column,
            domain,
            value,
            constant_on_left,
        } => {
            hasher.update([0]);
            hasher.update((*column as u64).to_le_bytes());
            hasher.update([*constant_on_left as u8]);
            digest_equality_domain(hasher, Some(*domain));
            digest_canonical_object(hasher, value);
        }
        RelationEqualityPlan::Columns {
            left,
            right,
            domain,
        } => {
            hasher.update([1]);
            hasher.update((*left as u64).to_le_bytes());
            hasher.update((*right as u64).to_le_bytes());
            digest_equality_domain(hasher, Some(*domain));
        }
        RelationEqualityPlan::Constants { left, right } => {
            hasher.update([2]);
            digest_canonical_object(hasher, left);
            digest_canonical_object(hasher, right);
        }
    }
}

pub(crate) fn digest_tuple_disequality_pairs(hasher: &mut Sha256, pairs: &[TupleDisequalityPairPlan]) {
    let node = |hasher: &mut Sha256, node: &TupleDisequalityNode| match node {
        TupleDisequalityNode::Column(column) => {
            hasher.update([0]);
            hasher.update((*column as u64).to_le_bytes());
        }
        TupleDisequalityNode::Constant(value) => {
            hasher.update([1]);
            digest_canonical_object(hasher, value);
        }
    };
    hasher.update((pairs.len() as u64).to_le_bytes());
    for pair in pairs {
        node(hasher, &pair.a0);
        node(hasher, &pair.a1);
        node(hasher, &pair.b0);
        node(hasher, &pair.b1);
    }
}

pub(crate) fn digest_key_pairs(hasher: &mut Sha256, key_pairs: &[(usize, usize)]) {
    hasher.update((key_pairs.len() as u64).to_le_bytes());
    for (left, right) in key_pairs {
        hasher.update((*left as u64).to_le_bytes());
        hasher.update((*right as u64).to_le_bytes());
    }
}

