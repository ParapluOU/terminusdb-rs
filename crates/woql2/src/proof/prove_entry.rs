//! Prove / verify entry points and the portable proof envelope.
//!
//! The public surface a caller drives: [`CompiledBgp`]'s `prove` / `prove_with_context` /
//! `prove_on_layer(s)_with_context` methods (compile-then-prove against a committed layer),
//! the one-shot [`compile_and_prove`] helpers, and the envelope encode /
//! [`decode_and_verify_envelope`] round-trip that lets a proof travel to an independent
//! verifier. Also the small `prove_*_relation` adapters for the verifier-recompute forms and
//! the [`QueryProofError`] type. Parsing an envelope never confers trust — verification
//! re-checks it against an independently compiled query and a caller-chosen trusted root.

use super::*;

/// Decode and verify a portable envelope against an independently compiled query and
/// caller-selected trusted root. Parsing alone never confers verification status.
pub fn decode_and_verify_envelope(
    bytes: &[u8],
    compiled: &CompiledBgp,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<VerifiedBgpEnvelope> {
    let envelope = RecursiveRelationEnvelope::decode(bytes)?;
    let digest = compiled_query_digest(compiled)?;
    let plan = relation_proof_plan(&compiled.relation, digest)?;
    envelope.verify(layer, expected_root, &plan)?;
    let nullable = envelope
        .result_descriptors
        .iter()
        .any(|descriptor| descriptor.nullable);
    let generated = envelope.result_descriptors.iter().any(|descriptor| {
        matches!(
            descriptor.namespace,
            ResultIdNamespace::GeneratedDecimalU64
                | ResultIdNamespace::GeneratedDecimalI64
        )
    });
    let result_objects = if nullable || generated {
        Vec::new()
    } else {
        envelope
            .result_values
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| match value {
                        CanonicalResultValue::Object(object) => Ok(object.clone()),
                        CanonicalResultValue::Null => Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "null value in a nonnullable recursive result",
                        )),
                        CanonicalResultValue::GeneratedDecimalU64(_) => Err(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "generated scalar is not a Store dictionary object",
                            ),
                        ),
                        CanonicalResultValue::GeneratedDecimalI64(_) => Err(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "generated scalar is not a Store dictionary object",
                            ),
                        ),
                    })
                    .collect()
            })
            .collect::<std::io::Result<_>>()?
    };
    let result_values = (nullable || generated).then(|| envelope.result_values.clone());
    Ok(VerifiedBgpEnvelope {
        proved: proved_from_recursive(envelope),
        result_objects,
        result_values,
    })
}

/// Decode both portable inputs and verify without loading native Store witness tables.
/// The trusted root remains a separate caller-selected input; the artifact cannot choose
/// its own trust anchor merely by decoding successfully.
pub fn decode_verifier_commitment_and_verify_envelope(
    envelope_bytes: &[u8],
    compiled: &CompiledBgp,
    verifier_commitment_bytes: &[u8],
    expected_root: ProofHash,
) -> std::io::Result<VerifiedBgpEnvelope> {
    let commitment = VerifierCommitment::decode(verifier_commitment_bytes)?;
    decode_and_verify_envelope(envelope_bytes, compiled, &commitment, expected_root)
}

/// Prove a top-level WOQL `Eval`: prove the operand-binding child through the Store, then
/// APPEND the derived integer column computed by the SAME recompute the verifier re-runs.
///
/// The child's own columns and commitments are reused verbatim (they are already
/// root-authenticated by the Store proof); only the one derived column is committed here,
/// with the child's context/domain, so nothing about the existing committed columns changes.
/// Because the appended column is produced by `expected_eval_columns` — the exact function
/// `verify_relation` re-runs — a prover cannot deliver a different column and still verify.
pub(crate) fn prove_eval_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    expr: &EvalExpr,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    // Recompute (child columns ++ derived column) against THIS layer's own root; identical to
    // the verifier's re-derivation. Take the last (derived) column and commit it.
    let expected =
        expected_eval_columns(&child, input, expr, layer, layer.state.commitment_root)?;
    let derived = expected
        .last()
        .cloned()
        .expect("expected_eval_columns always appends the derived column");

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut padded = derived.clone();
    padded.resize(domain, ArkFr::zero());
    let derived_commitment = context.commit_column(&padded)?;

    let mut result_columns = child.result_columns.clone();
    result_columns.push(derived);
    let mut result_commitments = child.result_commitments.clone();
    result_commitments.push(derived_commitment);

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Eval {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `floor(x)`: prove the operand-binding child through the Store, then APPEND
/// the derived integer column computed by the SAME [`expected_floor_eval_columns`] the verifier
/// re-runs. Structurally identical to [`prove_eval_relation`] (only the recompute differs — exact
/// rationals then a floor), so a prover cannot deliver a different column and still verify, and
/// cannot prove a floor whose i64 narrowing overflows or whose operand row is non-numeric.
pub(crate) fn prove_floor_eval_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    expr: &DecimalExpr,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let expected =
        expected_floor_eval_columns(&child, input, expr, layer, layer.state.commitment_root)?;
    let derived = expected
        .last()
        .cloned()
        .expect("expected_floor_eval_columns always appends the derived column");

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut padded = derived.clone();
    padded.resize(domain, ArkFr::zero());
    let derived_commitment = context.commit_column(&padded)?;

    let mut result_columns = child.result_columns.clone();
    result_columns.push(derived);
    let mut result_commitments = child.result_commitments.clone();
    result_commitments.push(derived_commitment);

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::FloorEval {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL exact-rational arithmetic (`Divide`/decimal-operand arithmetic): prove the
/// operand-binding child through the Store, then DELIVER the per-row xsd:decimal result computed by
/// the SAME [`expected_rational_arith_objects`] the verifier re-runs. Like `Concat`, NO derived
/// column is committed — the result is TERMINAL/revealed; the child's columns/commitments are reused
/// verbatim. A zero divisor, a non-numeric operand row, or a carry-overflow render FAILS CLOSED here
/// (exactly as verify does), so a prover cannot produce a proof for a row the reference would reject.
pub(crate) fn prove_rational_arith_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    expr: &DecimalExpr,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects =
        expected_rational_arith_objects(&child, input, expr, layer, layer.state.commitment_root)?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::RationalArith {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL NUMBER→STRING `Typecast`: prove the source-binding child through the Store,
/// then DELIVER the per-row `xsd:string` cast produced by the SAME [`expected_cast_to_string_objects`]
/// the verifier re-runs. Like `Concat`/`RationalArith`, NO derived column is committed — the child's
/// columns/commitments are reused verbatim and the synthesized strings are re-checked at verify. A
/// float/double or non-numeric operand row FAILS CLOSED here (exactly as verify does).
pub(crate) fn prove_typecast_to_string_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    source_column: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_cast_to_string_objects(
        &child,
        input,
        source_column,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::TypecastNumberToString {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL STRING→NUMBER `Typecast`: prove the source-binding child through the Store,
/// then DELIVER the per-row numeric cast produced by the SAME [`expected_cast_from_string_objects`]
/// the verifier re-runs. Like `Concat`/`RationalArith`, NO derived column is committed. A
/// malformed/out-of-grammar string, or a non-string operand row, FAILS CLOSED here (exactly as
/// verify does), so a prover cannot produce a proof for a row the reference would reject.
pub(crate) fn prove_typecast_from_string_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    source_column: usize,
    target: StringNumericTarget,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_cast_from_string_objects(
        &child,
        input,
        source_column,
        target,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::TypecastStringToNumber {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL INTEGER-result calendar op (`Weekday`/`WeekdaySundayStart`/`IsoWeek`):
/// prove the operand-binding child through the Store, then APPEND the derived integer column(s)
/// computed by the SAME [`expected_calendar_int_columns`] the verifier re-runs. Structurally like
/// [`prove_eval_relation`], but — because `IsoWeek` binds TWO integers — it appends ALL `outputs.len()`
/// derived columns (each committed), NOT just the last: taking only `.last()` would silently drop a
/// column and mis-shape the proof. A non-date operand row / out-of-range year FAILS CLOSED here
/// (exactly as verify does), so a prover cannot produce a proof over a row the reference would reject.
pub(crate) fn prove_calendar_int_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    date_column: usize,
    outputs: &[CalendarIntFn],
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    // Recompute (child columns ++ one derived column PER output) against THIS layer's own root;
    // identical to the verifier's re-derivation. The trailing `outputs.len()` columns are the derived
    // ones — commit and append EACH (never just the last).
    let expected =
        expected_calendar_int_columns(&child, input, date_column, outputs, layer, layer.state.commitment_root)?;
    let derived_start = expected.len().checked_sub(outputs.len()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar-integer recompute produced fewer columns than outputs",
        )
    })?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut result_columns = child.result_columns.clone();
    let mut result_commitments = child.result_commitments.clone();
    for derived in &expected[derived_start..] {
        let mut padded = derived.clone();
        padded.resize(domain, ArkFr::zero());
        result_commitments.push(context.commit_column(&padded)?);
        result_columns.push(derived.clone());
    }

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::CalendarInt {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL DATE-result calendar op (`DayAfter`/`DayBefore`/`MonthStartDate`/
/// `MonthEndDate`): prove the operand-binding child through the Store, then DELIVER the per-row
/// `xsd:date` result computed by the SAME [`expected_calendar_date_objects`] the verifier re-runs.
/// Like `Concat`, NO derived column is committed — the child's columns/commitments are reused verbatim
/// and the synthesized date is re-checked at verify. A non-zero offset / out-of-range year / wrong-typed
/// operand FAILS CLOSED here (exactly as verify does).
pub(crate) fn prove_calendar_date_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    op: CalendarDateFn,
    input_column: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects =
        expected_calendar_date_objects(&child, input, op, input_column, layer, layer.state.commitment_root)?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::CalendarDate {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Typecast`: prove the source-binding child through the Store, then
/// APPEND the derived cast column computed by the SAME recompute the verifier re-runs.
///
/// Structurally identical to [`prove_eval_relation`]: the child's columns/commitments are reused
/// verbatim (already root-authenticated by the Store proof); only the one cast column is committed
/// here. Because it is produced by `expected_typecast_column` — the exact function
/// `verify_relation` re-runs (including the target-range fail-closed check) — a prover cannot
/// deliver a different column and still verify, and cannot prove an out-of-range narrowing at all.
pub(crate) fn prove_typecast_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    source_column: usize,
    source_datatype: CanonicalDatatype,
    target_datatype: CanonicalDatatype,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let expected = expected_typecast_column(
        &child,
        input,
        source_column,
        source_datatype,
        target_datatype,
        layer,
        layer.state.commitment_root,
    )?;
    let derived = expected
        .last()
        .cloned()
        .expect("expected_typecast_column always appends the derived column");

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut padded = derived.clone();
    padded.resize(domain, ArkFr::zero());
    let derived_commitment = context.commit_column(&padded)?;

    let mut result_columns = child.result_columns.clone();
    result_columns.push(derived);
    let mut result_commitments = child.result_commitments.clone();
    result_commitments.push(derived_commitment);

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Typecast {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Concatenate`: prove the operand-binding child through the Store, then
/// DELIVER the per-row concatenation computed by the SAME recompute the verifier re-runs.
///
/// Unlike `Eval`/`Typecast`, NO derived column is committed: the concat result is TERMINAL and
/// revealed. The child's columns/commitments are reused verbatim (already root-authenticated by
/// the Store proof); the synthesized strings are carried in the proved statement and re-derived
/// (hence re-checked) at verify. Because they are produced by `expected_concat_objects` — the
/// exact function `verify_relation` re-runs over the root-authenticated operands — a prover
/// cannot deliver a different concatenation and still verify, and cannot prove a concat over a
/// non-string operand row at all (it fails closed here, matching the verifier).
pub(crate) fn prove_concat_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    elements: &[ConcatElement],
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    // Recompute the per-row concatenation against THIS layer's own root; identical to the
    // verifier's re-derivation (fails closed on a non-string operand row, exactly as verify does).
    let result_objects =
        expected_concat_objects(&child, input, elements, layer, layer.state.commitment_root)?;

    // The committed columns/commitments are the child's, unchanged (terminal result is revealed).
    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Concat {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Join`: prove the operand-binding child through the Store, then DELIVER
/// the per-row separator-interposed join produced by the SAME recompute the verifier re-runs
/// ([`expected_join_objects`]). Like `Concat`, NO derived column is committed — the child's
/// columns/commitments are reused verbatim and the synthesized strings are re-checked at verify.
pub(crate) fn prove_join_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    elements: &[ConcatElement],
    separator: &ConcatElement,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_join_objects(
        &child,
        input,
        elements,
        separator,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Join {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Substring`: prove the operand-binding child through the Store, then
/// DELIVER the per-row char-substring produced by the SAME recompute the verifier re-runs
/// ([`expected_substring_objects`]). Fails closed here (exactly as verify does) on an out-of-range
/// or non-UTF-8 operand row, so a prover cannot produce a proof over a row the reference drops.
pub(crate) fn prove_substring_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    string_column: usize,
    before: usize,
    length: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_substring_objects(
        &child,
        input,
        string_column,
        before,
        length,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Substring {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a WOQL `Pad`: prove the operand-binding child through the Store, then DELIVER the per-row
/// left-padded strings recomputed by the SAME [`expected_pad_objects`] the verifier re-runs — no
/// new Store gadget, no committed result column, the committed columns are the child's verbatim.
pub(crate) fn prove_pad_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    string_column: usize,
    pad_char: &[u8],
    target_len: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_pad_objects(
        &child,
        input,
        string_column,
        pad_char,
        target_len,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Pad {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a WOQL `HashKey`/`hash`: prove the operand-binding child through the Store, then DELIVER
/// the per-row `idgen_hash` document-IDs recomputed by the SAME [`expected_hashkey_objects`] the
/// verifier re-runs — no new Store gadget, no committed result column.
pub(crate) fn prove_hashkey_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    base: &ConcatElement,
    args: &[ConcatElement],
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_hashkey_objects(
        &child,
        input,
        base,
        args,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::HashKey {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

pub(crate) fn proved_from_recursive(envelope: RecursiveRelationEnvelope) -> ProvedBgp {
    // The two-root cross-dictionary hash join is carried WHOLESALE (it engages two commitment roots
    // and is verified by `verify_cross_root`, not the single-graph mirror), so it is wrapped directly
    // rather than decomposed into the `ProvedQueryStatement` tree.
    if matches!(envelope.proof, RecursiveRelationProof::CrossRootHashJoin { .. }) {
        return ProvedBgp {
            result_commitments: envelope.result_commitments.clone(),
            result_columns: envelope.result_columns.clone(),
            result_log_domain: envelope.result_log_domain,
            result_len: envelope.result_len,
            statement: ProvedQueryStatement::CrossRootHashJoin { envelope: Box::new(envelope) },
        };
    }
    // The ordered-list walk is a store-proven leaf whose composite proof is store-private (it cannot
    // be decomposed into the `ProvedQueryStatement` tree the way a closure decomposes into
    // edge/seed/rounds/fixpoint). So — exactly like the cross-root hash join — it is carried
    // WHOLESALE and later verified by re-running the store envelope verify, not by reconstruction.
    if matches!(envelope.proof, RecursiveRelationProof::OrderedListWalk(_)) {
        return ProvedBgp {
            result_commitments: envelope.result_commitments.clone(),
            result_columns: envelope.result_columns.clone(),
            result_log_domain: envelope.result_log_domain,
            result_len: envelope.result_len,
            statement: ProvedQueryStatement::OrderedListWalk { envelope: Box::new(envelope) },
        };
    }
    // The ordered-array walk is likewise a store-proven leaf whose composite proof is store-private
    // (the cell-set BGP, functional binds, type filter, dictionary lookup, and density permutation
    // cannot be decomposed into the `ProvedQueryStatement` tree), so — exactly like the ordered-list
    // walk — it is carried WHOLESALE and later verified by re-running the store envelope verify.
    if matches!(envelope.proof, RecursiveRelationProof::OrderedArrayWalk(_)) {
        return ProvedBgp {
            result_commitments: envelope.result_commitments.clone(),
            result_columns: envelope.result_columns.clone(),
            result_log_domain: envelope.result_log_domain,
            result_len: envelope.result_len,
            statement: ProvedQueryStatement::OrderedArrayWalk { envelope: Box::new(envelope) },
        };
    }
    let statement = match envelope.proof {
        RecursiveRelationProof::LayerTripleCount => ProvedQueryStatement::LayerTripleCount,
        RecursiveRelationProof::GeneratedU64Filter { input } => {
            ProvedQueryStatement::GeneratedU64Filter {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::GeneratedI64Filter { input } => {
            ProvedQueryStatement::GeneratedI64Filter {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::Bgp(leaf) => ProvedQueryStatement::Single {
            proof: Box::new(leaf.proof),
            predicate_ids: leaf.predicate_ids,
        },
        RecursiveRelationProof::BagUnion { branches, proof } => ProvedQueryStatement::BagUnion {
            branches: branches.into_iter().map(proved_from_recursive).collect(),
            proof,
        },
        RecursiveRelationProof::BagProjection { input } => {
            ProvedQueryStatement::BagProjection {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::Distinct { input, proof } => ProvedQueryStatement::Distinct {
            input: Box::new(proved_from_recursive(*input)),
            proof,
        },
        RecursiveRelationProof::LeftAnti { left, right, proof } => ProvedQueryStatement::LeftAnti {
            left: Box::new(proved_from_recursive(*left)),
            right: Box::new(proved_from_recursive(*right)),
            proof,
        },
        RecursiveRelationProof::InnerJoin { left, right, proof } => ProvedQueryStatement::InnerJoin {
            left: Box::new(proved_from_recursive(*left)),
            right: Box::new(proved_from_recursive(*right)),
            proof,
        },
        RecursiveRelationProof::LeftOuter { left, right, proof } => {
            ProvedQueryStatement::LeftOuter {
                left: Box::new(proved_from_recursive(*left)),
                right: Box::new(proved_from_recursive(*right)),
                proof,
            }
        }
        RecursiveRelationProof::Equality { input, proof } => ProvedQueryStatement::Equality {
            input: Box::new(proved_from_recursive(*input)),
            proof,
        },
        RecursiveRelationProof::TupleDisequality { input, proof } => {
            ProvedQueryStatement::TupleDisequality {
                input: Box::new(proved_from_recursive(*input)),
                proof,
            }
        }
        RecursiveRelationProof::ExactScalarOrder { input, proof } => {
            ProvedQueryStatement::ExactScalarOrder {
                input: Box::new(proved_from_recursive(*input)),
                proof,
            }
        }
        RecursiveRelationProof::ExactMultiOrder { input, proof } => ProvedQueryStatement::ExactMultiOrder {
            input: Box::new(proved_from_recursive(*input)), proof,
        },
        RecursiveRelationProof::ExactOrderedSlice { input } => {
            ProvedQueryStatement::ExactOrderedSlice {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::GroupExtrema { input } => {
            ProvedQueryStatement::GroupExtrema {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::GroupSum { input } => ProvedQueryStatement::GroupSum {
            input: Box::new(proved_from_recursive(*input)),
        },
        RecursiveRelationProof::TransitiveClosure {
            edge,
            seed,
            rounds,
            fixpoint,
        } => ProvedQueryStatement::TransitiveClosure {
            edge: Box::new(proved_from_recursive(*edge)),
            seed,
            rounds,
            fixpoint: Box::new(fixpoint),
        },
        RecursiveRelationProof::GroupCount { input } => ProvedQueryStatement::GroupCount {
            input: Box::new(proved_from_recursive(*input)),
        },
        RecursiveRelationProof::CollectionCount { input } => {
            ProvedQueryStatement::CollectionCount {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::CollectionSum { input } => ProvedQueryStatement::CollectionSum {
            input: Box::new(proved_from_recursive(*input)),
        },
        RecursiveRelationProof::CollectionExtrema { input } => {
            ProvedQueryStatement::CollectionExtrema {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        // The two-root cross-dictionary hash join is carried WHOLESALE by the early return at the top of
        // this function (it engages two roots and is verified by `verify_cross_root`, not the
        // single-graph mirror), so it never reaches this decomposition match.
        RecursiveRelationProof::CrossRootHashJoin { .. } => unreachable!(
            "cross-root envelopes are wrapped wholesale by the early return, not decomposed here"
        ),
        // Wrapped wholesale by the early return above (its store proof is opaque to this layer), so it
        // never reaches this decomposition match.
        RecursiveRelationProof::OrderedListWalk(_) => unreachable!(
            "ordered-list-walk envelopes are wrapped wholesale by the early return, not decomposed here"
        ),
        // Wrapped wholesale by the early return above (its store proof is opaque to this layer), so it
        // never reaches this decomposition match.
        RecursiveRelationProof::OrderedArrayWalk(_) => unreachable!(
            "ordered-array-walk envelopes are wrapped wholesale by the early return, not decomposed here"
        ),
    };
    ProvedBgp {
        statement,
        result_commitments: envelope.result_commitments,
        result_columns: envelope.result_columns,
        result_log_domain: envelope.result_log_domain,
        result_len: envelope.result_len,
    }
}

/// Errors at the explicit compile/load/prove boundary.
#[derive(Debug, thiserror::Error)]
pub enum QueryProofError {
    #[error(transparent)]
    Plan(#[from] BgpPlanError),
    #[error(transparent)]
    Store(#[from] std::io::Error),
}


pub(crate) fn store_id_for_field(
    field: ArkFr,
    namespace: ResultIdNamespace,
    layer: &impl VerifierLayer,
) -> std::io::Result<u64> {
    let view = layer.verifier_view();
    match namespace {
        ResultIdNamespace::Predicate => view
            .predicate_catalog
            .iter()
            .find(|r| ArkFr::from_u64(r.predicate_id) == field)
            .map(|r| r.predicate_id),
        ResultIdNamespace::NodeValue => view
            .node_value_catalog
            .iter()
            .find(|r| ArkFr::from_u64(r.object_id) == field)
            .map(|r| r.object_id),
        ResultIdNamespace::GeneratedDecimalU64
        | ResultIdNamespace::GeneratedDecimalI64 => None,
    }
    .ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-extrema Store id absent from planned namespace",
        )
    })
}

pub(crate) fn group_output_rows(
    columns: &[Vec<ArkFr>],
    n: usize,
    relation: &CompiledRelation,
    layer: &impl VerifierLayer,
) -> std::io::Result<Vec<(u64, u64)>> {
    if columns.len() != 2 || columns.iter().any(|c| c.len() < n) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-extrema output shape mismatch",
        ));
    }
    let namespace = relation_result_descriptors(relation)?[0].namespace;
    (0..n)
        .map(|i| {
            Ok((
                store_id_for_field(columns[0][i], namespace, layer)?,
                store_id_for_field(columns[1][i], ResultIdNamespace::NodeValue, layer)?,
            ))
        })
        .collect()
}

pub(crate) fn distinct_output_rows(
    columns: &[Vec<ArkFr>],
    n: usize,
    relation: &CompiledRelation,
    layer: &impl VerifierLayer,
) -> std::io::Result<Vec<Vec<u64>>> {
    let descriptors = relation_result_descriptors(relation)?;
    if columns.len() != descriptors.len() || columns.iter().any(|column| column.len() < n) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "distinct output shape mismatch",
        ));
    }
    (0..n)
        .map(|row| {
            columns
                .iter()
                .zip(&descriptors)
                .map(|(column, descriptor)| {
                    store_id_for_field(column[row], descriptor.namespace, layer)
                })
                .collect()
        })
        .collect()
}


impl CompiledBgp {
    /// Generate a proof against an already loaded authenticated commitment.
    pub fn prove(&self, layer: &LayerCommitment) -> std::io::Result<ProvedBgp> {
        self.prove_with_context(&StoreProverContext::cpu(), layer)
    }

    /// Generate the complete recursive relation proof using caller-selected arithmetic.
    pub fn prove_with_context(
        &self,
        context: &StoreProverContext<'_>,
        layer: &LayerCommitment,
    ) -> std::io::Result<ProvedBgp> {
        // WOQL `Eval` is a woql2-side verifier-recompute layered over a Store-proven child.
        // Prove the operand-binding child through the Store, then APPEND the derived integer
        // column using the same recompute the verifier will re-run — no new Store gadget and
        // no change to the existing committed columns.
        if let CompiledRelationStatement::Eval { input, expr } = &self.relation.statement {
            return prove_eval_relation(context, layer, input, expr);
        }
        // WOQL `Typecast` is likewise a woql2-side verifier-recompute over a Store-proven child.
        if let CompiledRelationStatement::Typecast {
            input,
            source_column,
            source_datatype,
            target_datatype,
        } = &self.relation.statement
        {
            return prove_typecast_relation(
                context,
                layer,
                input,
                *source_column,
                *source_datatype,
                *target_datatype,
            );
        }
        // WOQL `Concatenate` is likewise a woql2-side verifier-recompute over a Store-proven
        // child: prove the operand-binding child through the Store, then DELIVER the per-row
        // concatenation recomputed by the same function the verifier re-runs — no new Store
        // gadget, no committed result column, no change to the existing committed columns.
        if let CompiledRelationStatement::Concat { input, elements, .. } = &self.relation.statement {
            return prove_concat_relation(context, layer, input, elements);
        }
        // WOQL `Join` — Concat with the separator interposed; same woql2-side recompute discipline.
        if let CompiledRelationStatement::Join {
            input,
            elements,
            separator,
            ..
        } = &self.relation.statement
        {
            return prove_join_relation(context, layer, input, elements, separator);
        }
        // WOQL `Substring` — a per-row char-slice of a Store-proven authenticated string column.
        if let CompiledRelationStatement::Substring {
            input,
            string_column,
            before,
            length,
            ..
        } = &self.relation.statement
        {
            return prove_substring_relation(context, layer, input, *string_column, *before, *length);
        }
        // WOQL `Pad` — a per-row left-pad of a Store-proven authenticated string column.
        if let CompiledRelationStatement::Pad {
            input,
            string_column,
            pad_char,
            target_len,
            ..
        } = &self.relation.statement
        {
            return prove_pad_relation(context, layer, input, *string_column, pad_char, *target_len);
        }
        // WOQL `HashKey`/`hash` — a per-row `idgen_hash` over Store-proven authenticated operands.
        if let CompiledRelationStatement::HashKey {
            input, base, args, ..
        } = &self.relation.statement
        {
            return prove_hashkey_relation(context, layer, input, base, args);
        }
        // WOQL `floor(x)` — a per-row exact-rational floor over a Store-proven authenticated child.
        if let CompiledRelationStatement::FloorEval { input, expr } = &self.relation.statement {
            return prove_floor_eval_relation(context, layer, input, expr);
        }
        // WOQL exact-rational arithmetic (`Divide`/decimal operands) — a per-row terminal
        // xsd:decimal delivery over a Store-proven authenticated child.
        if let CompiledRelationStatement::RationalArith { input, expr, .. } =
            &self.relation.statement
        {
            return prove_rational_arith_relation(context, layer, input, expr);
        }
        // WOQL NUMBER→STRING `Typecast` — a per-row terminal xsd:string delivery over a Store-proven
        // authenticated numeric child.
        if let CompiledRelationStatement::TypecastNumberToString {
            input,
            source_column,
            ..
        } = &self.relation.statement
        {
            return prove_typecast_to_string_relation(context, layer, input, *source_column);
        }
        // WOQL STRING→NUMBER `Typecast` — a per-row terminal numeric delivery over a Store-proven
        // authenticated xsd:string child.
        if let CompiledRelationStatement::TypecastStringToNumber {
            input,
            source_column,
            target,
            ..
        } = &self.relation.statement
        {
            return prove_typecast_from_string_relation(
                context,
                layer,
                input,
                *source_column,
                *target,
            );
        }
        // WOQL INTEGER-result calendar ops — a per-row map over a Store-proven authenticated date
        // column, appending the derived integer column(s) via the same recompute verify re-runs.
        if let CompiledRelationStatement::CalendarInt {
            input,
            date_column,
            outputs,
        } = &self.relation.statement
        {
            return prove_calendar_int_relation(context, layer, input, *date_column, outputs);
        }
        // WOQL DATE-result calendar ops — a per-row terminal xsd:date delivery over a Store-proven
        // authenticated operand column (like Concat/RationalArith).
        if let CompiledRelationStatement::CalendarDate {
            input,
            op,
            input_column,
            ..
        } = &self.relation.statement
        {
            return prove_calendar_date_relation(context, layer, input, *op, *input_column);
        }
        let digest = compiled_query_digest(self)?;
        let plan = relation_proof_plan(&self.relation, digest)?;
        let envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &plan)?;
        Ok(proved_from_recursive(envelope))
    }

    /// Explicitly load the commitment from a synchronous Store layer and prove. Calling
    /// this method is the lifecycle decision; no worker or background job is started.
    ///
    /// INSTANCE-ONLY: this entry loads only the instance-graph commitment (byte-identical to before
    /// the schema-graph foundation). A query that reads the SCHEMA graph (Subsumption) rejects closed
    /// here — use [`CompiledBgp::prove_on_layers`], which also loads the schema commitment.
    pub fn prove_on_layer(&self, layer: &SyncStoreLayer) -> Result<ProvedBgp, QueryProofError> {
        self.prove_on_layer_with_context(&StoreProverContext::cpu(), layer)
    }

    /// Load the commitment from a synchronous Store layer and prove with the caller's
    /// context. No runtime initialization or scheduling occurs here.
    pub fn prove_on_layer_with_context(
        &self,
        context: &StoreProverContext<'_>,
        layer: &SyncStoreLayer,
    ) -> Result<ProvedBgp, QueryProofError> {
        // A schema-reading query cannot be proven with only the instance layer — reject closed rather
        // than silently anchor a schema proof to the instance root.
        if relation_graph(&self.relation)? != ProofGraph::Instance {
            return Err(QueryProofError::Store(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "query reads the schema graph; prove with `prove_on_layers` (instance + schema layers)",
            )));
        }
        let commitment = layer.proof_commitment()?;
        Ok(self.prove_with_context(context, &commitment)?)
    }

    /// SCHEMA-GRAPH FOUNDATION — prove over BOTH committed graphs (instance + schema).
    ///
    /// Selects, from the compiled plan's single [`relation_graph`], which layer's `LayerCommitment`
    /// the whole proof is built over: an instance query proves over the instance commitment exactly as
    /// [`prove_on_layer`] (byte-identical), a Subsumption query proves over the schema commitment. The
    /// audited recursion is handed that ONE commitment and stays graph-blind. The trust anchor a
    /// verifier reconstructs is the combined [`terminus_store::proof::commitment::database_root`] of
    /// the two per-graph roots (see [`CompiledBgp::prove_on_layers_root`]).
    pub fn prove_on_layers(
        &self,
        instance: &SyncStoreLayer,
        schema: &SyncStoreLayer,
    ) -> Result<ProvedBgp, QueryProofError> {
        self.prove_on_layers_with_context(&StoreProverContext::cpu(), instance, schema)
    }

    /// [`prove_on_layers`] with a caller-owned prover context.
    pub fn prove_on_layers_with_context(
        &self,
        context: &StoreProverContext<'_>,
        instance: &SyncStoreLayer,
        schema: &SyncStoreLayer,
    ) -> Result<ProvedBgp, QueryProofError> {
        // The cross-root hash join (IsA) reads BOTH graphs: prove it through the two-layer store entry
        // (left=schema, right=instance), binding both roots to the trusted `database_root`.
        if matches!(self.relation.statement, CompiledRelationStatement::CrossRootHashJoin { .. }) {
            let instance_commit = instance.proof_commitment()?;
            let schema_commit = schema.proof_commitment()?;
            let digest = compiled_query_digest(self)?;
            let plan = relation_proof_plan(&self.relation, digest)?;
            let envelope = RecursiveRelationEnvelope::prove_cross_root_with_context(
                context, &instance_commit, &schema_commit, &plan,
            )?;
            return Ok(proved_from_recursive(envelope));
        }
        let commitment = match relation_graph(&self.relation)? {
            ProofGraph::Instance => instance.proof_commitment()?,
            ProofGraph::Schema => schema.proof_commitment()?,
        };
        Ok(self.prove_with_context(context, &commitment)?)
    }

    /// Prove a top-level cross-root (IsA) query directly over two already-committed layers (the
    /// instance and schema `LayerCommitment`s). Mirrors [`prove_on_layers`] for callers that hold
    /// commitments rather than `SyncStoreLayer`s.
    pub fn prove_cross_root_layers(
        &self,
        instance: &LayerCommitment,
        schema: &LayerCommitment,
    ) -> Result<ProvedBgp, QueryProofError> {
        self.prove_cross_root_layers_with_context(&StoreProverContext::cpu(), instance, schema)
    }

    pub fn prove_cross_root_layers_with_context(
        &self,
        context: &StoreProverContext<'_>,
        instance: &LayerCommitment,
        schema: &LayerCommitment,
    ) -> Result<ProvedBgp, QueryProofError> {
        if !matches!(self.relation.statement, CompiledRelationStatement::CrossRootHashJoin { .. }) {
            return Err(QueryProofError::Store(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "prove_cross_root_layers requires a cross-root (IsA) query",
            )));
        }
        let digest = compiled_query_digest(self)?;
        let plan = relation_proof_plan(&self.relation, digest)?;
        let envelope = RecursiveRelationEnvelope::prove_cross_root_with_context(
            context, instance, schema, &plan,
        )?;
        Ok(proved_from_recursive(envelope))
    }

    /// The externally-trusted combined `database_root` for a two-graph database: binds the instance
    /// and schema per-graph commitment roots. A caller publishes this alongside a proof so a verifier
    /// can validate both commitments against it (see
    /// [`terminus_store::proof::commitment::database_root`]).
    ///
    /// TRUST ANCHOR: a verifier must obtain this `database_root` from a trusted OUT-OF-BAND pin (commit
    /// log / signature over the head) — NEVER by recomputing it from a prover-supplied proof bundle or
    /// schema view. A prover controls the schema layer it commits, so a `database_root` recomputed from
    /// prover input would happily bind a FABRICATED schema (with an injected `sys:inherits` superclass)
    /// and let a forged subsumption verify. See [`ProvedBgp::verify_over_graphs`].
    pub fn prove_on_layers_root(
        instance: &SyncStoreLayer,
        schema: &SyncStoreLayer,
    ) -> Result<ProofHash, QueryProofError> {
        let instance_root = instance.proof_commitment()?.state.commitment_root;
        let schema_root = schema.proof_commitment()?.state.commitment_root;
        Ok(terminus_store::proof::commitment::database_root(
            &instance_root,
            &schema_root,
        ))
    }
}

/// Convenience for app/domain callers that want one explicit compile-and-prove call.
pub fn compile_and_prove(
    query: &Query,
    layer: &SyncStoreLayer,
) -> Result<(CompiledBgp, ProvedBgp), QueryProofError> {
    compile_and_prove_with_context(&StoreProverContext::cpu(), query, layer)
}

/// Compile and prove with an explicitly caller-owned Store prover context.
pub fn compile_and_prove_with_context(
    context: &StoreProverContext<'_>,
    query: &Query,
    layer: &SyncStoreLayer,
) -> Result<(CompiledBgp, ProvedBgp), QueryProofError> {
    let compiled = plan_bgp(query)?;
    let proof = compiled.prove_on_layer_with_context(context, layer)?;
    Ok((compiled, proof))
}

