//! Compile a WOQL conjunctive BGP AST into a safe, variable-blind [`LayerBgpPlan`].
//!
//! See the crate docs for why the PLAN is the sole safety line. This module accepts
//! only the fragment the authenticated left-deep join chain can encode faithfully and
//! rejects everything else closed:
//!
//! * zero or more nested outer [`Select`]s (projection is read-side only — the proof
//!   still proves the FULL BGP), wrapping
//! * an [`And`] (nested `And`s flattened) of `k >= 1` [`Triple`]s, each a
//!   `<subj> <predicate> <obj>` atom where the predicate is either a constant IRI or a
//!   query variable retained from the authenticated P column, and each of subject/object
//!   is EITHER a query VARIABLE (a kept output column) OR an
//!   aligned CONSTANT — a subject/object node IRI ([`AtomTerm::NodeConstant`]) or an
//!   object typed literal of a round-trip-aligned xsd datatype ([`AtomTerm::ValueConstant`]:
//!   all scalar `XSDAnySimpleType` variants, routed through the Store's
//!   typed-value construction). Each atom keeps one output column per variable (S,P,O order);
//!   the atoms join into a left-deep chain. Shared variables use ordinary natural joins;
//!   disconnected components use the Store's verifier-known UNIT-key Cartesian product.
//!   Two-variable graph keys use the Store's injective two-u64 packing and remain bound
//!   to the original columns.
//!
//! Fully-ground atoms are authenticated zero-arity existence relations and gate the
//! relational result. Every emitted constant is one the store
//! can authenticate against its committed node/value dictionary, canonicalized IDENTICALLY
//! to how the store stored it (node IRIs by their IRI bytes; `xsd:string` literals by
//! routing through the store's own value construction).

use chrono::Datelike;
use dory_pcs::backends::arkworks::{ArkFr, ArkGT};
use dory_pcs::primitives::arithmetic::Field;
use dory_pcs::primitives::transcript::Transcript;
use sha2::{Digest, Sha256};
use tdb_succinct::{AnyURI, Date, HexBinary, TdbDataType, TypedDictEntry};
use terminus_store::proof::anti::{
    prove_global_not, prove_left_anti_join_with_keys, prove_left_outer_join_with_keys,
    verify_global_not, verify_left_anti_join_with_keys, verify_left_outer_join_with_keys,
    LeftAntiJoinProof, LeftOuterJoinProof,
};
use terminus_store::proof::argument::Transcript254;
use terminus_store::proof::bgp::{JoinStep, RelationCommit, RelationWitness};
use terminus_store::proof::commitment::LayerCommitment;
use terminus_store::proof::envelope::{
    plan_digest, BagUnionEnvelope, LayerBgpEnvelope, LeftAntiJoinEnvelope,
    LeftOuterJoinEnvelope,
};
use terminus_store::proof::query::{
    prove_layer_bgp_with_result, resolve_result_values, resolve_typed_result_objects,
    result_id_namespaces, verify_layer_bgp, verify_layer_bgp_result_log_domain,
    verify_result_columns, AtomTerm, CanonicalResultValue, LayerBgpAtom, LayerBgpPlan,
    LayerBgpProof, ObjectRangeBound, ObjectRangeSpec, ResultColumnDescriptor,
    ResultIdNamespace,
};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::union::{prove_bag_union, verify_bag_union, BagUnionProof};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject, ProofHash};
use terminus_store::store::sync::SyncStoreLayer;
use terminus_store::ValueTriple;

use crate::misc::InRange;
use crate::query::Query;
use crate::triple::{Data, Link, Triple};
use crate::value::{DataValue, NodeValue, Value};
use terminusdb_schema::{GraphType, XSDAnySimpleType};

/// A compiled, variable-blind BGP plan plus the metadata a caller needs to read the
/// proof's result columns.
#[derive(Clone, Debug)]
pub struct CompiledBgp {
    /// The variable-blind plan handed to `prove_layer_bgp` / `verify_layer_bgp`.
    pub plan: LayerBgpPlan,
    /// Ordered branch plans for an outer WOQL `Or`. Empty denotes the ordinary
    /// single-plan path; otherwise `plan` is the first branch for compatibility.
    pub union_plans: Vec<LayerBgpPlan>,
    /// For each union branch, public-schema column -> branch-result column. This aligns
    /// compatible variable sets whose internal left-deep plans produce different order.
    pub union_schema_mappings: Vec<Vec<usize>>,
    /// A correlated `Not` compiled as a scalar-key left anti-join. The ordinary
    /// `plan` is the positive outer relation; this metadata names the independently
    /// authenticated negated relation and the one shared variable column.
    pub anti_join: Option<CompiledAntiJoin>,
    pub optional_join: Option<CompiledOptionalJoin>,
    /// Result column index -> BGP variable name, in the proof's output order
    /// (`[r_other.., key, s_other..]` folded over the join steps).
    pub schema: Vec<String>,
    /// If a `Select` wrapped the BGP, the selected variables' column indices into
    /// `schema`. The proof still proves the FULL BGP; projection is which output
    /// columns the caller reads (sound — a projection of the true answer).
    pub projection: Option<Vec<usize>>,
    /// Query-routing metadata retained from outer WOQL wrappers. The cryptographic
    /// statement is anchored to the layer root supplied by the caller; integrations
    /// must use this collection when resolving that layer.
    pub collection: Option<String>,
    /// An outer WOQL `Count` binding. The proven BGP result length is the count, so no
    /// additional cryptographic argument is required.
    pub count_variable: Option<String>,
    /// True when a WOQL `Distinct` was proven redundant because it names the complete
    /// BGP schema. Full natural-join assignments over set-valued triple relations are
    /// already unique; subset/projection distinctness still requires a dedup argument.
    pub full_row_distinct: bool,
}

fn compiled_result_namespaces(compiled: &CompiledBgp) -> std::io::Result<Vec<ResultIdNamespace>> {
    if let Some(optional) = &compiled.optional_join {
        let mut namespaces = result_id_namespaces(&compiled.plan)?;
        let right = result_id_namespaces(&optional.right_plan)?;
        let shared_right: Vec<usize> = optional.key_pairs.iter().map(|pair| pair.1).collect();
        namespaces.extend(
            right
                .into_iter()
                .enumerate()
                .filter_map(|(index, namespace)| (!shared_right.contains(&index)).then_some(namespace)),
        );
        return Ok(namespaces);
    }
    if compiled.union_plans.is_empty() {
        result_id_namespaces(&compiled.plan)
    } else {
        let mut expected: Option<Vec<ResultIdNamespace>> = None;
        for (plan, mapping) in compiled
            .union_plans
            .iter()
            .zip(&compiled.union_schema_mappings)
        {
            let branch = result_id_namespaces(plan)?;
            let mapped: Vec<_> = mapping.iter().map(|index| branch[*index]).collect();
            if expected.as_ref().is_some_and(|kinds| kinds != &mapped) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL union branches expose incompatible Store ID namespaces",
                ));
            }
            expected = Some(mapped);
        }
        Ok(expected.unwrap_or_default())
    }
}

fn compiled_result_descriptors(
    compiled: &CompiledBgp,
) -> std::io::Result<Vec<ResultColumnDescriptor>> {
    let namespaces = compiled_result_namespaces(compiled)?;
    let left_arity = result_id_namespaces(&compiled.plan)?.len();
    Ok(namespaces
        .into_iter()
        .enumerate()
        .map(|(index, namespace)| ResultColumnDescriptor {
            namespace,
            nullable: compiled.optional_join.is_some() && index >= left_arity,
        })
        .collect())
}

#[derive(Clone, Debug)]
pub struct CompiledAntiJoin {
    pub right_plan: LayerBgpPlan,
    /// Negated branch schema, bound into the public query digest even though local
    /// variables are not exposed by Prolog negation.
    pub right_schema: Vec<String>,
    /// Ordered `(outer, negated)` schema-column pairs. Two pairs use the Store's
    /// injective two-u64 composite key in this exact order.
    pub key_pairs: Vec<(usize, usize)>,
}

#[derive(Clone, Debug)]
pub struct CompiledOptionalJoin {
    pub right_plan: LayerBgpPlan,
    pub right_schema: Vec<String>,
    pub key_pairs: Vec<(usize, usize)>,
}

/// An in-memory proof result produced only when a caller explicitly invokes
/// [`CompiledBgp::prove`] or [`CompiledBgp::prove_on_layer`]. It intentionally contains
/// no scheduling or persistence policy; applications own when and where proving runs.
#[derive(Debug)]
pub struct ProvedBgp {
    pub statement: ProvedQueryStatement,
    pub result_commitments: Vec<ArkGT>,
    /// Real, unpadded Store ID columns in the compiled schema order.
    pub result_columns: Vec<Vec<ArkFr>>,
    pub result_log_domain: usize,
    pub result_len: usize,
}

#[derive(Debug)]
pub enum ProvedQueryStatement {
    Single {
        proof: LayerBgpProof,
        predicate_ids: Vec<u64>,
    },
    BagUnion(ProvedBagUnion),
    LeftAntiJoin(ProvedLeftAntiJoin),
    LeftOuterJoin(ProvedLeftOuterJoin),
}

#[derive(Debug)]
pub struct ProvedLeftOuterJoin {
    pub left: ProvedBagUnionBranch,
    pub right: ProvedBagUnionBranch,
    pub proof: LeftOuterJoinProof,
}

#[derive(Debug)]
pub struct ProvedLeftAntiJoin {
    pub left: ProvedBagUnionBranch,
    pub right: ProvedBagUnionBranch,
    /// Present for correlated anti-joins. Global Not is pinned entirely by the
    /// independently authenticated right length and carries no extra witness.
    pub proof: Option<LeftAntiJoinProof>,
}

#[derive(Debug)]
pub struct ProvedBagUnionBranch {
    pub proof: LayerBgpProof,
    pub result_commitments: Vec<ArkGT>,
    pub result_columns: Vec<Vec<ArkFr>>,
    pub result_log_domain: usize,
    pub result_len: usize,
    pub predicate_ids: Vec<u64>,
}

#[derive(Debug)]
pub struct ProvedBagUnion {
    pub branches: Vec<ProvedBagUnionBranch>,
    pub proof: BagUnionProof,
}

/// A decoded envelope that has already been checked against the caller's query, trusted
/// root and layer. Canonical results remain alongside the reconstructed proof result.
#[derive(Debug)]
pub struct VerifiedBgpEnvelope {
    pub proved: ProvedBgp,
    pub result_objects: Vec<Vec<CanonicalObject>>,
    pub result_values: Option<Vec<Vec<CanonicalResultValue>>>,
}

fn digest_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Stable identity of all public WOQL interpretation metadata layered over the Store
/// plan. This prevents reusing one envelope under a different schema, projection,
/// collection, count binding, or `Distinct` interpretation.
pub fn compiled_query_digest(compiled: &CompiledBgp) -> std::io::Result<ProofHash> {
    let mut hasher = Sha256::new();
    hasher.update(b"terminusdb-woql2/query-proof-envelope/v1");
    hasher.update(plan_digest(&compiled.plan)?);
    if !compiled.union_plans.is_empty() {
        hasher.update(b"/bag-union/v1");
        hasher.update((compiled.union_plans.len() as u64).to_le_bytes());
        for (plan, mapping) in compiled
            .union_plans
            .iter()
            .zip(&compiled.union_schema_mappings)
        {
            hasher.update(plan_digest(plan)?);
            hasher.update((mapping.len() as u64).to_le_bytes());
            for index in mapping {
                hasher.update((*index as u64).to_le_bytes());
            }
        }
    }
    if let Some(anti) = &compiled.anti_join {
        hasher.update(b"/left-anti-join/v0");
        hasher.update(plan_digest(&anti.right_plan)?);
        hasher.update((anti.right_schema.len() as u64).to_le_bytes());
        for name in &anti.right_schema {
            digest_bytes(&mut hasher, name.as_bytes());
        }
        hasher.update((anti.key_pairs.len() as u64).to_le_bytes());
        for (left, right) in &anti.key_pairs {
            hasher.update((*left as u64).to_le_bytes());
            hasher.update((*right as u64).to_le_bytes());
        }
    }
    if let Some(optional) = &compiled.optional_join {
        hasher.update(b"/left-outer-join/v0");
        hasher.update(plan_digest(&optional.right_plan)?);
        hasher.update((optional.right_schema.len() as u64).to_le_bytes());
        for name in &optional.right_schema {
            digest_bytes(&mut hasher, name.as_bytes());
        }
        hasher.update((optional.key_pairs.len() as u64).to_le_bytes());
        for (left, right) in &optional.key_pairs {
            hasher.update((*left as u64).to_le_bytes());
            hasher.update((*right as u64).to_le_bytes());
        }
    }
    hasher.update((compiled.schema.len() as u64).to_le_bytes());
    for name in &compiled.schema {
        digest_bytes(&mut hasher, name.as_bytes());
    }
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

impl ProvedBgp {
    pub fn as_single(&self) -> Option<(&LayerBgpProof, &[u64])> {
        match &self.statement {
            ProvedQueryStatement::Single {
                proof,
                predicate_ids,
            } => Some((proof, predicate_ids)),
            ProvedQueryStatement::BagUnion(_) => None,
            ProvedQueryStatement::LeftAntiJoin(_) => None,
            ProvedQueryStatement::LeftOuterJoin(_) => None,
        }
    }

    pub fn as_bag_union(&self) -> Option<&ProvedBagUnion> {
        match &self.statement {
            ProvedQueryStatement::Single { .. } => None,
            ProvedQueryStatement::BagUnion(union) => Some(union),
            ProvedQueryStatement::LeftAntiJoin(_) => None,
            ProvedQueryStatement::LeftOuterJoin(_) => None,
        }
    }

    /// Verify against a caller-selected, independently trusted root.
    pub fn verify(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
    ) -> std::io::Result<()> {
        if let ProvedQueryStatement::LeftOuterJoin(proved) = &self.statement {
            let optional = compiled.optional_join.as_ref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "left outer proof supplied for a non-Optional query",
                )
            })?;
            for (branch, plan) in [
                (&proved.left, &compiled.plan),
                (&proved.right, &optional.right_plan),
            ] {
                verify_layer_bgp(
                    layer,
                    expected_root,
                    plan,
                    &branch.predicate_ids,
                    &branch.result_commitments,
                    branch.result_len,
                    &branch.proof,
                )?;
                verify_layer_bgp_result_log_domain(
                    layer,
                    plan,
                    &branch.proof,
                    branch.result_log_domain,
                )?;
                verify_result_columns(
                    &branch.result_commitments,
                    &branch.result_columns,
                    branch.result_len,
                    branch.result_log_domain,
                )?;
            }
            let digest = compiled_query_digest(compiled)?;
            let mut transcript = Transcript254::new(b"parture-left-outer-envelope");
            transcript.append_bytes(b"pow/outer-envelope/root", &expected_root);
            transcript.append_bytes(b"pow/outer-envelope/query", &digest);
            verify_left_outer_join_with_keys(
                &RelationCommit {
                    commitments: proved.left.result_commitments.clone(),
                    log_domain: proved.left.result_log_domain,
                    n: proved.left.result_len,
                },
                &RelationCommit {
                    commitments: proved.right.result_commitments.clone(),
                    log_domain: proved.right.result_log_domain,
                    n: proved.right.result_len,
                },
                &optional.key_pairs,
                &self.result_commitments,
                self.result_len,
                self.result_log_domain,
                &proved.proof,
                &mut transcript,
            )?;
            return self.verify_columns(&self.result_columns);
        }
        if let ProvedQueryStatement::LeftAntiJoin(proved) = &self.statement {
            let anti = compiled.anti_join.as_ref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "left anti-join proof supplied for a non-Not query",
                )
            })?;
            for (branch, plan) in [
                (&proved.left, &compiled.plan),
                (&proved.right, &anti.right_plan),
            ] {
                verify_layer_bgp(
                    layer,
                    expected_root,
                    plan,
                    &branch.predicate_ids,
                    &branch.result_commitments,
                    branch.result_len,
                    &branch.proof,
                )?;
                verify_layer_bgp_result_log_domain(
                    layer,
                    plan,
                    &branch.proof,
                    branch.result_log_domain,
                )?;
                verify_result_columns(
                    &branch.result_commitments,
                    &branch.result_columns,
                    branch.result_len,
                    branch.result_log_domain,
                )?;
            }
            let digest = compiled_query_digest(compiled)?;
            let mut transcript = Transcript254::new(b"parture-left-anti-envelope");
            transcript.append_bytes(b"pow/anti-envelope/root", &expected_root);
            transcript.append_bytes(b"pow/anti-envelope/query", &digest);
            let left_commit = RelationCommit {
                    commitments: proved.left.result_commitments.clone(),
                    log_domain: proved.left.result_log_domain,
                    n: proved.left.result_len,
                };
            let right_commit = RelationCommit {
                    commitments: proved.right.result_commitments.clone(),
                    log_domain: proved.right.result_log_domain,
                    n: proved.right.result_len,
                };
            match (anti.key_pairs.is_empty(), proved.proof.as_ref()) {
                (true, None) => verify_global_not(
                    &left_commit, &right_commit, &self.result_commitments, self.result_len,
                    self.result_log_domain,
                )?,
                (false, Some(proof)) => verify_left_anti_join_with_keys(
                    &left_commit, &right_commit, &anti.key_pairs, &self.result_commitments,
                    self.result_len, self.result_log_domain, proof, &mut transcript,
                )?,
                _ => return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Not key/proof variant mismatch",
                )),
            }
            return self.verify_columns(&self.result_columns);
        }
        if let ProvedQueryStatement::BagUnion(union) = &self.statement {
            if compiled.union_plans.len() != union.branches.len()
                || compiled.union_schema_mappings.len() != union.branches.len()
                || union.branches.len() < 2
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL bag-union branch shape mismatch",
                ));
            }
            let mut inputs = Vec::with_capacity(union.branches.len());
            for ((branch, plan), mapping) in union
                .branches
                .iter()
                .zip(&compiled.union_plans)
                .zip(&compiled.union_schema_mappings)
            {
                verify_layer_bgp(
                    layer,
                    expected_root,
                    plan,
                    &branch.predicate_ids,
                    &branch.result_commitments,
                    branch.result_len,
                    &branch.proof,
                )?;
                verify_layer_bgp_result_log_domain(
                    layer,
                    plan,
                    &branch.proof,
                    branch.result_log_domain,
                )?;
                verify_result_columns(
                    &branch.result_commitments,
                    &branch.result_columns,
                    branch.result_len,
                    branch.result_log_domain,
                )?;
                let union_arity = compiled.schema.len().max(1);
                if mapping.len() != union_arity
                    || mapping
                        .iter()
                        .any(|index| *index >= branch.result_columns.len())
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL bag-union schema mapping mismatch",
                    ));
                }
                inputs.push(RelationCommit {
                    commitments: mapping
                        .iter()
                        .map(|index| branch.result_commitments[*index])
                        .collect(),
                    log_domain: branch.result_log_domain,
                    n: branch.result_len,
                });
            }
            let digest = compiled_query_digest(compiled)?;
            let mut transcript = Transcript254::new(b"parture-union-envelope");
            transcript.append_bytes(b"pow/union-envelope/root", &expected_root);
            transcript.append_bytes(b"pow/union-envelope/query", &digest);
            verify_bag_union(
                &inputs,
                &self.result_commitments,
                self.result_len,
                self.result_log_domain,
                &union.proof,
                &mut transcript,
            )?;
            return self.verify_columns(&self.result_columns);
        }
        let ProvedQueryStatement::Single {
            proof,
            predicate_ids,
        } = &self.statement
        else {
            unreachable!("bag-union statement returned from its verification branch")
        };
        if !compiled.union_plans.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "single BGP proof cannot verify a compiled WOQL bag union",
            ));
        }
        if compiled.anti_join.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "single BGP proof cannot verify a compiled WOQL Not",
            ));
        }
        verify_layer_bgp(
            layer,
            expected_root,
            &compiled.plan,
            predicate_ids,
            &self.result_commitments,
            self.result_len,
            proof,
        )?;
        verify_layer_bgp_result_log_domain(layer, &compiled.plan, proof, self.result_log_domain)?;
        self.verify_columns(&self.result_columns)
    }

    /// Verify arbitrary caller-delivered Store ID columns against the result commitments
    /// authenticated by this proof. A changed, reordered, inserted, or omitted row fails.
    pub fn verify_columns(&self, columns: &[Vec<ArkFr>]) -> std::io::Result<()> {
        verify_result_columns(
            &self.result_commitments,
            columns,
            self.result_len,
            self.result_log_domain,
        )
    }

    /// Verify the query proof and resolve every proven Store ID to the exact canonical
    /// node IRI or typed literal in the root-authenticated node/value dictionary.
    pub fn verify_and_resolve(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalObject>>> {
        if compiled.optional_join.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "nullable Optional results require verify_and_resolve_values",
            ));
        }
        self.verify(compiled, layer, expected_root)?;
        if compiled.schema.is_empty() {
            Ok(Vec::new())
        } else {
            resolve_typed_result_objects(
                layer,
                expected_root,
                &self.result_columns,
                &compiled_result_namespaces(compiled)?,
            )
        }
    }

    pub fn verify_and_resolve_values(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalResultValue>>> {
        self.verify(compiled, layer, expected_root)?;
        if compiled.schema.is_empty() {
            Ok(Vec::new())
        } else {
            resolve_result_values(
                layer,
                expected_root,
                &self.result_columns,
                &compiled_result_descriptors(compiled)?,
            )
        }
    }

    /// Verify caller-delivered canonical node IRIs/typed literals against the query
    /// proof, its committed result IDs, and the root-authenticated Store dictionary.
    pub fn verify_objects(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
        objects: &[Vec<CanonicalObject>],
    ) -> std::io::Result<()> {
        self.verify(compiled, layer, expected_root)?;
        if compiled.schema.is_empty() {
            if objects.is_empty() {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ground result exposes no dictionary-object columns",
                ))
            }
        } else {
            let resolved = resolve_typed_result_objects(
                layer,
                expected_root,
                &self.result_columns,
                &compiled_result_namespaces(compiled)?,
            )?;
            if resolved == objects {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "delivered result objects do not match typed authenticated Store IDs",
                ))
            }
        }
    }

    /// Verify rows produced by the ordinary WOQL executor against this proof. The
    /// executor supplies Store dictionary IDs in its public variable order. WOQL BGP
    /// results are unordered, so comparison is an exact row-multiset comparison after
    /// both sides resolve through the authenticated dictionary.
    pub fn verify_executed_rows(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
        variables: &[String],
        rows: &[Vec<u64>],
    ) -> std::io::Result<()> {
        if let Some(count_variable) = compiled.count_variable.as_deref() {
            self.verify(compiled, layer, expected_root)?;
            if variables != [count_variable] {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "executed Count variable does not match the compiled count binding",
                ));
            }
            let count = u64::try_from(self.result_len).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "proven Count result does not fit the execution boundary",
                )
            })?;
            if rows != [vec![count]] {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary WOQL Count binding does not equal the proven result cardinality",
                ));
            }
            return Ok(());
        }
        if compiled.schema.is_empty() {
            self.verify(compiled, layer, expected_root)?;
            if !variables.is_empty()
                || rows.len() != self.result_len
                || rows.iter().any(|row| !row.is_empty())
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary ground-query rows do not match the proven existence relation",
                ));
            }
            return Ok(());
        }
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..compiled.schema.len()).collect());
        let expected_variables: Vec<&str> = projection
            .iter()
            .map(|index| compiled.schema[*index].as_str())
            .collect();
        if variables.iter().map(String::as_str).collect::<Vec<_>>() != expected_variables {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "executed result variables do not match the compiled projection",
            ));
        }
        if rows.iter().any(|row| row.len() != projection.len()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "executed result row width does not match the compiled projection",
            ));
        }

        let proven_objects = self.verify_and_resolve(compiled, layer, expected_root)?;
        let mut expected_rows: Vec<Vec<CanonicalObject>> = (0..self.result_len)
            .map(|row| {
                projection
                    .iter()
                    .map(|column| proven_objects[*column][row].clone())
                    .collect()
            })
            .collect();

        let mut delivered_columns = vec![Vec::with_capacity(rows.len()); projection.len()];
        for row in rows {
            for (column, id) in row.iter().enumerate() {
                delivered_columns[column].push(ArkFr::from_u64(*id));
            }
        }
        let namespaces = compiled_result_namespaces(compiled)?;
        let projected_namespaces: Vec<_> = projection.iter().map(|index| namespaces[*index]).collect();
        let delivered_objects = resolve_typed_result_objects(
            layer,
            expected_root,
            &delivered_columns,
            &projected_namespaces,
        )?;
        let mut delivered_rows: Vec<Vec<CanonicalObject>> = (0..rows.len())
            .map(|row| {
                (0..projection.len())
                    .map(|column| delivered_objects[column][row].clone())
                    .collect()
            })
            .collect();

        expected_rows.sort();
        delivered_rows.sort();
        if expected_rows != delivered_rows {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ordinary WOQL result rows do not match the proven result multiset",
            ));
        }
        Ok(())
    }

    /// Encode a self-contained result envelope only after checking the proof, result IDs,
    /// and canonical lexical values against the caller-selected root.
    pub fn encode_envelope(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<u8>> {
        if let ProvedQueryStatement::LeftOuterJoin(proved) = &self.statement {
            let optional = compiled.optional_join.as_ref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "left outer proof supplied for a non-Optional query",
                )
            })?;
            let digest = compiled_query_digest(compiled)?;
            let branch_envelope = |branch: &ProvedBagUnionBranch,
                                   plan: &LayerBgpPlan|
             -> std::io::Result<LayerBgpEnvelope> {
                LayerBgpEnvelope::new_verified(
                    layer,
                    expected_root,
                    plan,
                    digest,
                    branch.proof.clone(),
                    branch.result_commitments.clone(),
                    branch.result_columns.clone(),
                    branch.result_log_domain,
                    branch.result_len,
                    branch.predicate_ids.clone(),
                )
            };
            return LeftOuterJoinEnvelope::new_verified(
                layer,
                expected_root,
                &compiled.plan,
                &optional.right_plan,
                digest,
                branch_envelope(&proved.left, &compiled.plan)?,
                branch_envelope(&proved.right, &optional.right_plan)?,
                optional.key_pairs.clone(),
                proved.proof.clone(),
                self.result_commitments.clone(),
                self.result_columns.clone(),
                self.result_log_domain,
                self.result_len,
            )?
            .encode();
        }
        if let ProvedQueryStatement::LeftAntiJoin(proved) = &self.statement {
            let anti = compiled.anti_join.as_ref().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "left anti-join proof supplied for a non-Not query",
                )
            })?;
            let digest = compiled_query_digest(compiled)?;
            let branch_envelope = |branch: &ProvedBagUnionBranch,
                                   plan: &LayerBgpPlan|
             -> std::io::Result<LayerBgpEnvelope> {
                LayerBgpEnvelope::new_verified(
                    layer,
                    expected_root,
                    plan,
                    digest,
                    branch.proof.clone(),
                    branch.result_commitments.clone(),
                    branch.result_columns.clone(),
                    branch.result_log_domain,
                    branch.result_len,
                    branch.predicate_ids.clone(),
                )
            };
            return LeftAntiJoinEnvelope::new_verified(
                layer,
                expected_root,
                &compiled.plan,
                &anti.right_plan,
                digest,
                branch_envelope(&proved.left, &compiled.plan)?,
                branch_envelope(&proved.right, &anti.right_plan)?,
                anti.key_pairs.clone(),
                proved.proof.clone(),
                self.result_commitments.clone(),
                self.result_columns.clone(),
                self.result_log_domain,
                self.result_len,
                compiled.schema.is_empty(),
            )?
            .encode();
        }
        if let ProvedQueryStatement::BagUnion(union) = &self.statement {
            let digest = compiled_query_digest(compiled)?;
            if union.branches.len() != compiled.union_plans.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL bag-union branch count mismatch",
                ));
            }
            let branches: Vec<LayerBgpEnvelope> = union
                .branches
                .iter()
                .zip(&compiled.union_plans)
                .map(|(branch, plan)| {
                    LayerBgpEnvelope::new_verified(
                        layer,
                        expected_root,
                        plan,
                        digest,
                        branch.proof.clone(),
                        branch.result_commitments.clone(),
                        branch.result_columns.clone(),
                        branch.result_log_domain,
                        branch.result_len,
                        branch.predicate_ids.clone(),
                    )
                })
                .collect::<std::io::Result<_>>()?;
            return BagUnionEnvelope::new_verified(
                layer,
                expected_root,
                &compiled.union_plans,
                digest,
                branches,
                union.proof.clone(),
                self.result_commitments.clone(),
                self.result_columns.clone(),
                self.result_log_domain,
                self.result_len,
                compiled.schema.is_empty(),
            )?
            .encode();
        }
        let ProvedQueryStatement::Single {
            proof,
            predicate_ids,
        } = &self.statement
        else {
            unreachable!("bag-union statement returned from its encoding branch")
        };
        if !compiled.union_plans.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "single BGP proof cannot encode as a WOQL bag union",
            ));
        }
        LayerBgpEnvelope::new_verified(
            layer,
            expected_root,
            &compiled.plan,
            compiled_query_digest(compiled)?,
            proof.clone(),
            self.result_commitments.clone(),
            self.result_columns.clone(),
            self.result_log_domain,
            self.result_len,
            predicate_ids.clone(),
        )?
        .encode()
    }

    /// Commitments selected by an outer WOQL `Select`, or every result commitment when
    /// the query has no projection.
    pub fn projected_commitments<'a>(
        &'a self,
        compiled: &'a CompiledBgp,
    ) -> impl Iterator<Item = &'a ArkGT> + 'a {
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..self.result_commitments.len()).collect());
        projection
            .into_iter()
            .map(|index| &self.result_commitments[index])
    }

    /// Result ID columns selected by an outer WOQL `Select`, or every column when there
    /// is no projection. `Count` exposes no underlying columns.
    pub fn projected_columns<'a>(
        &'a self,
        compiled: &'a CompiledBgp,
    ) -> impl Iterator<Item = &'a [ArkFr]> + 'a {
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..self.result_columns.len()).collect());
        projection
            .into_iter()
            .map(|index| self.result_columns[index].as_slice())
    }

    /// Return the variable/value pair bound by an outer WOQL `Count`, if present.
    pub fn count_binding<'a>(&self, compiled: &'a CompiledBgp) -> Option<(&'a str, usize)> {
        compiled
            .count_variable
            .as_deref()
            .map(|variable| (variable, self.result_len))
    }
}

/// Decode and verify a portable envelope against an independently compiled query and
/// caller-selected trusted root. Parsing alone never confers verification status.
pub fn decode_and_verify_envelope(
    bytes: &[u8],
    compiled: &CompiledBgp,
    layer: &LayerCommitment,
    expected_root: ProofHash,
) -> std::io::Result<VerifiedBgpEnvelope> {
    if let Some(optional) = &compiled.optional_join {
        let envelope = LeftOuterJoinEnvelope::decode(bytes)?;
        let digest = compiled_query_digest(compiled)?;
        envelope.verify(
            layer,
            expected_root,
            &compiled.plan,
            &optional.right_plan,
            digest,
            &optional.key_pairs,
        )?;
        let branch = |envelope: LayerBgpEnvelope| ProvedBagUnionBranch {
            proof: envelope.proof,
            result_commitments: envelope.result_commitments,
            result_columns: envelope.result_columns,
            result_log_domain: envelope.result_log_domain,
            result_len: envelope.result_len,
            predicate_ids: envelope.predicate_ids,
        };
        return Ok(VerifiedBgpEnvelope {
            proved: ProvedBgp {
                statement: ProvedQueryStatement::LeftOuterJoin(ProvedLeftOuterJoin {
                    left: branch(envelope.left),
                    right: branch(envelope.right),
                    proof: envelope.outer_proof,
                }),
                result_commitments: envelope.result_commitments,
                result_columns: envelope.result_columns,
                result_log_domain: envelope.result_log_domain,
                result_len: envelope.result_len,
            },
            result_objects: Vec::new(),
            result_values: Some(envelope.result_values),
        });
    }
    if let Some(anti) = &compiled.anti_join {
        let envelope = LeftAntiJoinEnvelope::decode(bytes)?;
        let digest = compiled_query_digest(compiled)?;
        envelope.verify(
            layer,
            expected_root,
            &compiled.plan,
            &anti.right_plan,
            digest,
            &anti.key_pairs,
            compiled.schema.is_empty(),
        )?;
        let branch = |envelope: LayerBgpEnvelope| ProvedBagUnionBranch {
            proof: envelope.proof,
            result_commitments: envelope.result_commitments,
            result_columns: envelope.result_columns,
            result_log_domain: envelope.result_log_domain,
            result_len: envelope.result_len,
            predicate_ids: envelope.predicate_ids,
        };
        return Ok(VerifiedBgpEnvelope {
            proved: ProvedBgp {
                statement: ProvedQueryStatement::LeftAntiJoin(ProvedLeftAntiJoin {
                    left: branch(envelope.left),
                    right: branch(envelope.right),
                    proof: envelope.anti_proof,
                }),
                result_commitments: envelope.result_commitments,
                result_columns: envelope.result_columns,
                result_log_domain: envelope.result_log_domain,
                result_len: envelope.result_len,
            },
            result_objects: envelope.result_objects,
            result_values: None,
        });
    }
    if !compiled.union_plans.is_empty() {
        let envelope = BagUnionEnvelope::decode(bytes)?;
        let digest = compiled_query_digest(compiled)?;
        envelope.verify(
            layer,
            expected_root,
            &compiled.union_plans,
            digest,
            compiled.schema.is_empty(),
        )?;
        let branches: Vec<ProvedBagUnionBranch> = envelope
            .branches
            .into_iter()
            .map(|branch| ProvedBagUnionBranch {
                proof: branch.proof,
                result_commitments: branch.result_commitments,
                result_columns: branch.result_columns,
                result_log_domain: branch.result_log_domain,
                result_len: branch.result_len,
                predicate_ids: branch.predicate_ids,
            })
            .collect();
        branches.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "empty bag-union envelope")
        })?;
        return Ok(VerifiedBgpEnvelope {
            proved: ProvedBgp {
                statement: ProvedQueryStatement::BagUnion(ProvedBagUnion {
                    branches,
                    proof: envelope.union_proof,
                }),
                result_commitments: envelope.result_commitments,
                result_columns: envelope.result_columns,
                result_log_domain: envelope.result_log_domain,
                result_len: envelope.result_len,
            },
            result_objects: envelope.result_objects,
            result_values: None,
        });
    }
    let envelope = LayerBgpEnvelope::decode(bytes)?;
    envelope.verify(
        layer,
        expected_root,
        &compiled.plan,
        compiled_query_digest(compiled)?,
    )?;
    Ok(VerifiedBgpEnvelope {
        proved: ProvedBgp {
            statement: ProvedQueryStatement::Single {
                proof: envelope.proof,
                predicate_ids: envelope.predicate_ids,
            },
            result_commitments: envelope.result_commitments,
            result_columns: envelope.result_columns,
            result_log_domain: envelope.result_log_domain,
            result_len: envelope.result_len,
        },
        result_objects: envelope.result_objects,
        result_values: None,
    })
}

/// Errors at the explicit compile/load/prove boundary.
#[derive(Debug, thiserror::Error)]
pub enum QueryProofError {
    #[error(transparent)]
    Plan(#[from] BgpPlanError),
    #[error(transparent)]
    Store(#[from] std::io::Error),
}

impl CompiledBgp {
    /// Generate a proof against an already loaded authenticated commitment.
    pub fn prove(&self, layer: &LayerCommitment) -> std::io::Result<ProvedBgp> {
        if let Some(optional) = &self.optional_join {
            if !self.union_plans.is_empty() || self.anti_join.is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Optional cannot also carry union or Not plans",
                ));
            }
            let left = prove_layer_bgp_with_result(layer, &self.plan)?;
            let right = prove_layer_bgp_with_result(layer, &optional.right_plan)?;
            let left_witness = RelationWitness {
                columns: left.result_columns.clone(),
                commitments: left.result_commitments.clone(),
                log_domain: left.result_log_domain,
                n: left.result_len,
            };
            let right_witness = RelationWitness {
                columns: right.result_columns.clone(),
                commitments: right.result_commitments.clone(),
                log_domain: right.result_log_domain,
                n: right.result_len,
            };
            let digest = compiled_query_digest(self)?;
            let mut transcript = Transcript254::new(b"parture-left-outer-envelope");
            transcript.append_bytes(b"pow/outer-envelope/root", &layer.state.commitment_root);
            transcript.append_bytes(b"pow/outer-envelope/query", &digest);
            let result = prove_left_outer_join_with_keys(
                &left_witness,
                &right_witness,
                &optional.key_pairs,
                &mut transcript,
            )?;
            let branch = |result: terminus_store::proof::query::LayerBgpResult| {
                let real_columns = result.real_columns();
                ProvedBagUnionBranch {
                    proof: result.proof,
                    result_commitments: result.result_commitments,
                    result_columns: real_columns,
                    result_log_domain: result.result_log_domain,
                    result_len: result.result_len,
                    predicate_ids: result.predicate_ids,
                }
            };
            return Ok(ProvedBgp {
                statement: ProvedQueryStatement::LeftOuterJoin(ProvedLeftOuterJoin {
                    left: branch(left),
                    right: branch(right),
                    proof: result.proof,
                }),
                result_commitments: result.commitments,
                result_columns: result
                    .columns
                    .iter()
                    .map(|column| column[..result.n].to_vec())
                    .collect(),
                result_log_domain: result.log_domain,
                result_len: result.n,
            });
        }
        if let Some(anti) = &self.anti_join {
            if !self.union_plans.is_empty() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Not cannot also carry bag-union plans",
                ));
            }
            let left = prove_layer_bgp_with_result(layer, &self.plan)?;
            let right = prove_layer_bgp_with_result(layer, &anti.right_plan)?;
            let left_witness = RelationWitness {
                columns: left.result_columns.clone(),
                commitments: left.result_commitments.clone(),
                log_domain: left.result_log_domain,
                n: left.result_len,
            };
            let right_witness = RelationWitness {
                columns: right.result_columns.clone(),
                commitments: right.result_commitments.clone(),
                log_domain: right.result_log_domain,
                n: right.result_len,
            };
            let digest = compiled_query_digest(self)?;
            let mut transcript = Transcript254::new(b"parture-left-anti-envelope");
            transcript.append_bytes(b"pow/anti-envelope/root", &layer.state.commitment_root);
            transcript.append_bytes(b"pow/anti-envelope/query", &digest);
            let (operator_proof, commitments, columns, result_log_domain, result_len) =
                if anti.key_pairs.is_empty() {
                    let (commitments, columns, log_domain, n) =
                        prove_global_not(&left_witness, &right_witness)?;
                    (None, commitments, columns, log_domain, n)
                } else {
                    let result = prove_left_anti_join_with_keys(
                        &left_witness,
                        &right_witness,
                        &anti.key_pairs,
                        &mut transcript,
                    )?;
                    (
                        Some(result.proof),
                        result.commitments,
                        result.columns,
                        result.log_domain,
                        result.n,
                    )
                };
            let branch = |result: terminus_store::proof::query::LayerBgpResult| {
                let real_columns = result.real_columns();
                ProvedBagUnionBranch {
                    proof: result.proof,
                    result_commitments: result.result_commitments,
                    result_columns: real_columns,
                    result_log_domain: result.result_log_domain,
                    result_len: result.result_len,
                    predicate_ids: result.predicate_ids,
                }
            };
            return Ok(ProvedBgp {
                statement: ProvedQueryStatement::LeftAntiJoin(ProvedLeftAntiJoin {
                    left: branch(left),
                    right: branch(right),
                    proof: operator_proof,
                }),
                result_commitments: commitments,
                result_columns: columns
                    .iter()
                    .map(|column| column[..result_len].to_vec())
                    .collect(),
                result_log_domain,
                result_len,
            });
        }
        if !self.union_plans.is_empty() {
            if self.union_plans.len() != self.union_schema_mappings.len()
                || self.union_plans.len() < 2
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "compiled WOQL bag-union branch shape mismatch",
                ));
            }
            let mut branches = Vec::with_capacity(self.union_plans.len());
            let mut inputs = Vec::with_capacity(self.union_plans.len());
            for (plan, mapping) in self.union_plans.iter().zip(&self.union_schema_mappings) {
                let result = prove_layer_bgp_with_result(layer, plan)?;
                let mapped_columns: Vec<Vec<ArkFr>> = mapping
                    .iter()
                    .map(|index| result.result_columns[*index].clone())
                    .collect();
                let mapped_commitments: Vec<ArkGT> = mapping
                    .iter()
                    .map(|index| result.result_commitments[*index])
                    .collect();
                inputs.push(RelationWitness {
                    columns: mapped_columns,
                    commitments: mapped_commitments,
                    log_domain: result.result_log_domain,
                    n: result.result_len,
                });
                let real_columns = result.real_columns();
                branches.push(ProvedBagUnionBranch {
                    proof: result.proof,
                    result_commitments: result.result_commitments,
                    result_columns: real_columns,
                    result_log_domain: result.result_log_domain,
                    result_len: result.result_len,
                    predicate_ids: result.predicate_ids,
                });
            }
            let digest = compiled_query_digest(self)?;
            let mut transcript = Transcript254::new(b"parture-union-envelope");
            transcript.append_bytes(b"pow/union-envelope/root", &layer.state.commitment_root);
            transcript.append_bytes(b"pow/union-envelope/query", &digest);
            let union_result = prove_bag_union(&inputs, &mut transcript)?;
            let result_columns: Vec<Vec<ArkFr>> = union_result
                .columns
                .iter()
                .map(|column| column[..union_result.n].to_vec())
                .collect();
            return Ok(ProvedBgp {
                statement: ProvedQueryStatement::BagUnion(ProvedBagUnion {
                    branches,
                    proof: union_result.proof,
                }),
                result_commitments: union_result.commitments,
                result_columns,
                result_log_domain: union_result.log_domain,
                result_len: union_result.n,
            });
        }
        let result = prove_layer_bgp_with_result(layer, &self.plan)?;
        let result_columns = result.real_columns();
        Ok(ProvedBgp {
            statement: ProvedQueryStatement::Single {
                proof: result.proof,
                predicate_ids: result.predicate_ids,
            },
            result_commitments: result.result_commitments,
            result_columns,
            result_log_domain: result.result_log_domain,
            result_len: result.result_len,
        })
    }

    /// Explicitly load the commitment from a synchronous Store layer and prove. Calling
    /// this method is the lifecycle decision; no worker or background job is started.
    pub fn prove_on_layer(&self, layer: &SyncStoreLayer) -> Result<ProvedBgp, QueryProofError> {
        let commitment = layer.proof_commitment()?;
        Ok(self.prove(&commitment)?)
    }
}

/// Convenience for app/domain callers that want one explicit compile-and-prove call.
pub fn compile_and_prove(
    query: &Query,
    layer: &SyncStoreLayer,
) -> Result<(CompiledBgp, ProvedBgp), QueryProofError> {
    let compiled = plan_bgp(query)?;
    let proof = compiled.prove_on_layer(layer)?;
    Ok((compiled, proof))
}

/// Every reason [`plan_bgp`] refuses a query. One variant per rejection reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BgpPlanError {
    /// A construct that is not a supported proof wrapper, triple pattern, conjunction,
    /// or comparison appeared in the query body.
    #[error(
        "unsupported construct `{construct}`: only a conjunction (And) of triples is supported"
    )]
    UnsupportedConstruct { construct: &'static str },

    /// A conjunct inside the `And` was neither a `Triple` (or nested `And`) nor a numeric
    /// comparison (`Greater`/`Less`/`Gte`/`Lte`, routed to object-value range filters).
    #[error("unsupported conjunct `{construct}`: every conjunct must be a Triple or a numeric comparison")]
    ConjunctNotTriple { construct: &'static str },

    /// A comparison conjunct had a VARIABLE on both sides (`?a op ?b`): a range filter
    /// needs exactly one variable and one numeric constant.
    #[error("range comparison has a variable on both sides: exactly one side must be a numeric constant")]
    RangeBothVariables,

    /// A comparison conjunct had a CONSTANT on both sides (no variable to range over).
    #[error("range comparison has no variable: exactly one side must be the compared variable")]
    RangeBothConstants,

    /// A comparison's constant is not a round-trip-aligned order-preserving numeric literal.
    /// Supported: `xsd:boolean` (`Boolean`), `xsd:unsignedInt` (`UInt32`), `xsd:integer`
    /// (`Int64`), `xsd:float`/`double` (`Float64`). Strings, decimals, other datatypes, and
    /// list operands reject closed — a range over a non-order-preserving encoding would
    /// compare a sentinel and is unsound at the query level.
    #[error(
        "unsupported range constant `{datatype}`: no round-trip-aligned order-preserving encoding"
    )]
    UnsupportedRangeDatatype { datatype: &'static str },

    /// A ranged variable is not the object of any atom, so there is no object-binding atom
    /// to attach the range to.
    #[error("range variable `?{var}` is not the object of any atom")]
    RangeVariableNotObject { var: String },

    /// A ranged variable also occupies a SUBJECT position. v1 attaches a range only to a
    /// variable that is purely an atom's object (never a join key on the subject side).
    #[error(
        "range variable `?{var}` appears as a subject: only a pure object variable can be ranged"
    )]
    RangeVariableIsSubject { var: String },

    /// A ranged variable is the object of MORE THAN ONE atom. v1 requires a unique
    /// object-binding atom to attach the range to.
    #[error("range variable `?{var}` is the object of more than one atom")]
    RangeVariableObjectOfMultiple { var: String },

    /// More than two range bounds target the same variable.
    #[error("more than two range bounds on variable `?{var}`")]
    DuplicateRangeOnVariable { var: String },

    #[error("InRange value must be a variable")]
    InRangeValueNotVariable,

    #[error("InRange bounds must be scalar constants")]
    InRangeBoundNotConstant,

    #[error("range bounds for `?{var}` use different datatypes")]
    RangeDatatypeMismatch { var: String },

    /// No triple atoms remain after normalization.
    #[error("BGP needs at least one atom, found {count}")]
    TooFewAtoms { count: usize },

    #[error("variable `?{var}` occurs in both predicate and node/value positions")]
    VariableNamespaceConflict { var: String },

    /// `Link`/`Data` declare an object-kind restriction which a generic variable loses
    /// when normalized to a triple. Constants carry their kind intrinsically; variables
    /// need an authenticated type-selection proof.
    #[error("{construct} with a variable object requires authenticated object-kind filtering")]
    TypedObjectVariable { construct: &'static str },

    /// The object position held a compound value the store's node/value dictionary does
    /// not model as a single constant (a `Dictionary` or `List`). Node IRIs and aligned
    /// typed literals are accepted; these are not.
    #[error(
        "unsupported object value `{kind}`: only node IRIs and aligned typed literals are allowed"
    )]
    UnsupportedObjectValue { kind: &'static str },

    /// The object was a typed literal whose xsd datatype is not one this compiler can
    /// round-trip-align with the store's value canonicalization (only the verified subset
    /// is emitted; every other datatype rejects closed rather than authenticate an
    /// unproven constant). See CP3.
    #[error("unsupported value datatype `{datatype}`: no round-trip-aligned encoding")]
    UnsupportedValueDatatype { datatype: &'static str },

    /// Reserved for malformed externally-built plans. The compiler now emits sound
    /// mixed-arity gating plans directly.
    #[error("invalid ground/relational atom composition")]
    GroundAtom,

    /// The triple targeted the schema graph.
    #[error("schema-graph triples unsupported: only the instance graph is queried")]
    SchemaGraphUnsupported,

    /// Reserved for malformed externally-built plans. The compiler now composes
    /// disconnected components through authenticated Cartesian products.
    #[error("disconnected BGP")]
    DisconnectedBgp,

    #[error("Or needs at least two branches, found {count}")]
    OrTooFewBranches { count: usize },

    #[error("Or branches must expose exactly the same variable schema and order")]
    OrIncompatibleSchema,

    #[error("Or branches expose incompatible Store ID namespaces")]
    OrIncompatibleNamespaces,

    #[error("Select, Count, or nested Or inside an Or branch is unsupported")]
    OrUnsupportedBranchWrapper,

    #[error("Or branches resolve to conflicting collections")]
    OrRoutingConflict,

    #[error("correlated Not needs exactly one negated conjunct, found {count}")]
    NotConjunctCount { count: usize },

    #[error("correlated Not needs a non-empty positive outer BGP")]
    NotMissingOuter,

    #[error("Not supports at most two shared variables, found {count}")]
    NotSharedVariableCount { count: usize },

    #[error("Select, Count, Distinct, Or, or nested Not inside a Not subplan is unsupported")]
    NotUnsupportedSubplan,

    #[error("Not subplans resolve to conflicting collections")]
    NotRoutingConflict,

    #[error("Optional needs exactly one optional conjunct, found {count}")]
    OptionalConjunctCount { count: usize },

    #[error("Optional needs a non-empty positive outer BGP")]
    OptionalMissingOuter,

    #[error("Optional initially requires one or two shared variables, found {count}")]
    OptionalSharedVariableCount { count: usize },

    #[error("Optional subplans may not contain Select, Count, Distinct, Or, Not, or Optional")]
    OptionalUnsupportedSubplan,

    #[error("Optional subplans resolve to conflicting collections")]
    OptionalRoutingConflict,

    /// A `Select`ed variable does not occur in the BGP body.
    #[error("selected variable `?{var}` does not occur in the BGP")]
    SelectedVariableNotInBgp { var: String },

    /// Count output must be a variable; constant/list outputs need unification semantics
    /// that are not represented by the current result metadata.
    #[error("Count output must be a variable")]
    CountOutputNotVariable,

    #[error("Distinct is currently supported only without Select or Count wrappers")]
    DistinctWithProjectionOrCount,

    #[error("Distinct variables must name the complete BGP schema")]
    DistinctNotFullSchema,
}

/// The name of a `Query` variant, for descriptive rejection messages. Kept tiny and
/// covering the constructs a caller is most likely to hit.
fn query_variant_name(q: &Query) -> &'static str {
    match q {
        Query::And(_) => "And",
        Query::Or(_) => "Or",
        Query::Not(_) => "Not",
        Query::True(_) => "True",
        Query::Triple(_) => "Triple",
        Query::AddTriple(_) => "AddTriple",
        Query::DeleteTriple(_) => "DeleteTriple",
        Query::Link(_) => "Link",
        Query::Data(_) => "Data",
        Query::Eval(_) => "Eval",
        Query::Path(_) => "Path",
        Query::Equals(_) => "Equals",
        Query::Less(_) => "Less",
        Query::Greater(_) => "Greater",
        Query::Gte(_) => "Gte",
        Query::Lte(_) => "Lte",
        Query::Select(_) => "Select",
        Query::Distinct(_) => "Distinct",
        Query::If(_) => "If",
        Query::WoqlOptional(_) => "Optional",
        Query::Once(_) => "Once",
        Query::Immediately(_) => "Immediately",
        Query::OrderBy(_) => "OrderBy",
        Query::GroupBy(_) => "GroupBy",
        Query::Limit(_) => "Limit",
        Query::Start(_) => "Start",
        Query::Count(_) => "Count",
        Query::InRange(_) => "InRange",
        Query::TripleSlice(_) => "TripleSlice",
        Query::TripleSliceRev(_) => "TripleSliceRev",
        Query::Sum(_) => "Sum",
        Query::Member(_) => "Member",
        Query::Get(_) => "Get",
        _ => "unsupported",
    }
}

/// A per-atom relation, ready for the variable-arity planner. `layer_atom` is the
/// variable-blind atom emitted into the plan (subject/predicate/object terms). `kept`
/// is the atom's VARIABLE names in S,P,O order — the relation's output columns, one per
/// variable. An all-variable atom keeps two columns `[subj_var, obj_var]`; a
/// subject/object-constant atom keeps one. The planner joins on shared `kept` names and
/// `rel_key` indexes into `kept`.
struct Atom {
    layer_atom: LayerBgpAtom,
    kept: Vec<String>,
    /// The subject variable name, if the subject is a variable (else the subject is a
    /// constant). Used to route comparison conjuncts to the right object-binding atom.
    subj_var: Option<String>,
    /// Predicate variables use the independent predicate dictionary ID namespace.
    pred_var: Option<String>,
    /// The object variable name, if the object is a variable (else a constant). A numeric
    /// range attaches to the unique atom whose `obj_var` is the compared variable.
    obj_var: Option<String>,
}

/// Compile a woql2 conjunctive BGP query into a safe [`CompiledBgp`].
///
/// Accepts nested outer [`Select`]s and supported routing/result wrappers around an
/// `And` of `>= 1` triples of
/// the form `?subj <constant IRI> ?obj`, whose atoms connect into a
/// left-deep join tree (each step shares one or two variables). Everything
/// else is rejected closed with a descriptive [`BgpPlanError`].
pub fn plan_bgp(query: &Query) -> Result<CompiledBgp, BgpPlanError> {
    // (1) Unwrap proof-transparent outer controls. `Using` is not discarded: it is
    // retained as routing metadata so an integration can resolve the intended layer.
    // `From(instance)` is safe for the instance-layer proof path; schema or wildcard
    // reads reject closed. `Pin` affects optimization order, not query semantics.
    // Traversal is outer-to-inner. The bool records whether the Select is outside a
    // Count (and therefore selects the Count binding) or inside it (and therefore
    // projects the counted relation before its cardinality is taken).
    let mut selections: Vec<(&[String], bool)> = Vec::new();
    let mut collection: Option<String> = None;
    let mut count_variable: Option<String> = None;
    let mut distincts: Vec<&[String]> = Vec::new();
    let mut body = query;
    loop {
        body = match body {
            Query::Select(select) => {
                selections.push((select.variables.as_slice(), count_variable.is_none()));
                select.query.as_ref()
            }
            Query::Using(using) => {
                // Nested Using scopes replace the active collection for their inner query;
                // the innermost scope is therefore the layer-routing metadata we retain.
                collection = Some(using.collection.clone());
                using.query.as_ref()
            }
            Query::From(from) if from.graph.eq_ignore_ascii_case("instance") => from.query.as_ref(),
            Query::From(_) => return Err(BgpPlanError::SchemaGraphUnsupported),
            Query::Pin(pin) => pin.query.as_ref(),
            Query::Immediately(immediately) => immediately.query.as_ref(),
            Query::Distinct(distinct) => {
                distincts.push(distinct.variables.as_slice());
                distinct.query.as_ref()
            }
            Query::Count(count) if count_variable.is_none() => {
                count_variable = match &count.count {
                    DataValue::Variable(variable) => Some(variable.clone()),
                    _ => return Err(BgpPlanError::CountOutputNotVariable),
                };
                count.query.as_ref()
            }
            _ => break,
        };
    }

    if let Query::Or(disjunction) = body {
        if disjunction.or.len() < 2 {
            return Err(BgpPlanError::OrTooFewBranches {
                count: disjunction.or.len(),
            });
        }
        // Distinct over an Or is not transparent: overlapping branches deliberately
        // preserve duplicate multiplicity and need a separate authenticated dedup.
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        let mut branches = Vec::with_capacity(disjunction.or.len());
        for branch_query in &disjunction.or {
            let branch = plan_bgp(branch_query)?;
            let branch_has_projection = branch.projection.is_some() && !branch.schema.is_empty();
            if branch_has_projection
                || branch.count_variable.is_some()
                || !branch.union_plans.is_empty()
                || branch.anti_join.is_some()
                || branch.optional_join.is_some()
            {
                return Err(BgpPlanError::OrUnsupportedBranchWrapper);
            }
            branches.push(branch);
        }
        let schema = branches[0].schema.clone();
        if branches.iter().any(|branch| branch.schema != schema) {
            return Err(BgpPlanError::OrIncompatibleSchema);
        }
        let first_namespaces = result_id_namespaces(&branches[0].plan)
            .map_err(|_| BgpPlanError::OrIncompatibleNamespaces)?;
        if branches.iter().skip(1).any(|branch| {
            result_id_namespaces(&branch.plan)
                .map(|namespaces| namespaces != first_namespaces)
                .unwrap_or(true)
        }) {
            return Err(BgpPlanError::OrIncompatibleNamespaces);
        }

        let mut effective_collection = collection.clone();
        for branch in &branches {
            match (&effective_collection, &branch.collection) {
                (Some(expected), Some(actual)) if expected != actual => {
                    return Err(BgpPlanError::OrRoutingConflict)
                }
                (None, Some(actual)) => effective_collection = Some(actual.clone()),
                _ => {}
            }
        }
        if branches.iter().any(|branch| {
            branch.collection.as_ref().or(collection.as_ref()) != effective_collection.as_ref()
        }) {
            return Err(BgpPlanError::OrRoutingConflict);
        }

        let projection = compile_projection(&selections, count_variable.as_deref(), &schema)?;
        let union_plans: Vec<LayerBgpPlan> =
            branches.iter().map(|branch| branch.plan.clone()).collect();
        let union_arity = schema.len().max(1);
        let union_schema_mappings = (0..branches.len())
            .map(|_| (0..union_arity).collect())
            .collect();
        return Ok(CompiledBgp {
            plan: union_plans[0].clone(),
            union_plans,
            union_schema_mappings,
            anti_join: None,
            optional_join: None,
            schema,
            projection,
            collection: effective_collection,
            count_variable,
            full_row_distinct: false,
        });
    }

    let mut optional_positive = Vec::new();
    let mut optionals = Vec::new();
    split_top_level_optional(body, &mut optional_positive, &mut optionals);
    if !optionals.is_empty() {
        if optionals.len() != 1 {
            return Err(BgpPlanError::OptionalConjunctCount {
                count: optionals.len(),
            });
        }
        if optional_positive.is_empty() {
            return Err(BgpPlanError::OptionalMissingOuter);
        }
        let left_query = if optional_positive.len() == 1 {
            optional_positive.pop().unwrap()
        } else {
            Query::And(crate::query::And { and: optional_positive })
        };
        let left = plan_bgp(&left_query)?;
        let right = plan_bgp(&optionals.pop().unwrap())?;
        if left.projection.is_some()
            || left.count_variable.is_some()
            || left.full_row_distinct
            || !left.union_plans.is_empty()
            || left.anti_join.is_some()
            || left.optional_join.is_some()
            || right.projection.is_some()
            || right.count_variable.is_some()
            || right.full_row_distinct
            || !right.union_plans.is_empty()
            || right.anti_join.is_some()
            || right.optional_join.is_some()
        {
            return Err(BgpPlanError::OptionalUnsupportedSubplan);
        }
        let shared: Vec<(&String, usize, usize)> = left
            .schema
            .iter()
            .enumerate()
            .filter_map(|(left_index, name)| {
                right
                    .schema
                    .iter()
                    .position(|candidate| candidate == name)
                    .map(|right_index| (name, left_index, right_index))
            })
            .collect();
        if !(1..=2).contains(&shared.len()) {
            return Err(BgpPlanError::OptionalSharedVariableCount {
                count: shared.len(),
            });
        }
        let left_namespaces = result_id_namespaces(&left.plan)
            .map_err(|_| BgpPlanError::OptionalUnsupportedSubplan)?;
        let right_namespaces = result_id_namespaces(&right.plan)
            .map_err(|_| BgpPlanError::OptionalUnsupportedSubplan)?;
        if let Some((name, _, _)) = shared.iter().find(|(_, left, right)| {
            left_namespaces[*left] != right_namespaces[*right]
        }) {
            return Err(BgpPlanError::VariableNamespaceConflict {
                var: (*name).clone(),
            });
        }
        let effective_collection = [
            collection.as_ref(),
            left.collection.as_ref(),
            right.collection.as_ref(),
        ]
        .into_iter()
        .flatten()
        .next()
        .cloned();
        if [collection.as_ref(), left.collection.as_ref(), right.collection.as_ref()]
            .into_iter()
            .flatten()
            .any(|candidate| Some(candidate) != effective_collection.as_ref())
        {
            return Err(BgpPlanError::OptionalRoutingConflict);
        }
        let mut schema = left.schema.clone();
        schema.extend(
            right
                .schema
                .iter()
                .filter(|name| !left.schema.contains(name))
                .cloned(),
        );
        if !distincts.is_empty() {
            if !selections.is_empty() || count_variable.is_some() {
                return Err(BgpPlanError::DistinctWithProjectionOrCount);
            }
            for variables in &distincts {
                let covers = variables.len() == schema.len()
                    && schema.iter().all(|name| {
                        variables.iter().filter(|candidate| *candidate == name).count() == 1
                    });
                if !covers {
                    return Err(BgpPlanError::DistinctNotFullSchema);
                }
            }
        }
        let projection = compile_projection(&selections, count_variable.as_deref(), &schema)?;
        return Ok(CompiledBgp {
            plan: left.plan,
            union_plans: Vec::new(),
            union_schema_mappings: Vec::new(),
            anti_join: None,
            optional_join: Some(CompiledOptionalJoin {
                right_plan: right.plan,
                right_schema: right.schema,
                key_pairs: shared
                    .iter()
                    .map(|(_, left, right)| (*left, *right))
                    .collect(),
            }),
            schema,
            projection,
            collection: effective_collection,
            count_variable,
            full_row_distinct: !distincts.is_empty(),
        });
    }

    // Correlated Prolog negation is a left anti-join: evaluate the positive BGP,
    // evaluate the negated BGP, and retain exactly the outer rows whose shared key has
    // no right match. This first sound slice deliberately accepts one shared variable;
    // two-variable correlation needs the composite-key anti-join extension.
    let mut positive = Vec::new();
    let mut negatives = Vec::new();
    split_top_level_not(body, &mut positive, &mut negatives);
    if !negatives.is_empty() {
        if negatives.len() != 1 {
            return Err(BgpPlanError::NotConjunctCount {
                count: negatives.len(),
            });
        }
        if positive.is_empty() {
            return Err(BgpPlanError::NotMissingOuter);
        }
        let positive_query = if positive.len() == 1 {
            positive.pop().unwrap()
        } else {
            Query::And(crate::query::And { and: positive })
        };
        let left = plan_bgp(&positive_query)?;
        let right = plan_bgp(&negatives.pop().unwrap())?;
        if left.projection.is_some()
            || left.count_variable.is_some()
            || left.full_row_distinct
            || !left.union_plans.is_empty()
            || left.anti_join.is_some()
            || left.optional_join.is_some()
            || right.projection.is_some()
            || right.count_variable.is_some()
            || right.full_row_distinct
            || !right.union_plans.is_empty()
            || right.anti_join.is_some()
            || right.optional_join.is_some()
        {
            return Err(BgpPlanError::NotUnsupportedSubplan);
        }
        let shared: Vec<(&String, usize, usize)> = left
            .schema
            .iter()
            .enumerate()
            .filter_map(|(left_index, name)| {
                right
                    .schema
                    .iter()
                    .position(|candidate| candidate == name)
                    .map(|right_index| (name, left_index, right_index))
            })
            .collect();
        let left_namespaces = result_id_namespaces(&left.plan)
            .map_err(|_| BgpPlanError::VariableNamespaceConflict { var: "unknown".into() })?;
        let right_namespaces = result_id_namespaces(&right.plan)
            .map_err(|_| BgpPlanError::VariableNamespaceConflict { var: "unknown".into() })?;
        if let Some((name, _, _)) = shared.iter().find(|(_, left, right)| {
            left_namespaces[*left] != right_namespaces[*right]
        }) {
            return Err(BgpPlanError::VariableNamespaceConflict {
                var: (*name).clone(),
            });
        }
        if shared.len() > 2 {
            return Err(BgpPlanError::NotSharedVariableCount {
                count: shared.len(),
            });
        }
        let effective_collection = [
            collection.as_ref(),
            left.collection.as_ref(),
            right.collection.as_ref(),
        ]
        .into_iter()
        .flatten()
        .next()
        .cloned();
        if [
            collection.as_ref(),
            left.collection.as_ref(),
            right.collection.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|candidate| Some(candidate) != effective_collection.as_ref())
        {
            return Err(BgpPlanError::NotRoutingConflict);
        }
        if !distincts.is_empty() {
            if !selections.is_empty() || count_variable.is_some() {
                return Err(BgpPlanError::DistinctWithProjectionOrCount);
            }
            for variables in &distincts {
                let covers = variables.len() == left.schema.len()
                    && left
                        .schema
                        .iter()
                        .all(|name| variables.iter().filter(|v| *v == name).count() == 1);
                if !covers {
                    return Err(BgpPlanError::DistinctNotFullSchema);
                }
            }
        }
        let projection = compile_projection(&selections, count_variable.as_deref(), &left.schema)?;
        return Ok(CompiledBgp {
            plan: left.plan,
            union_plans: Vec::new(),
            union_schema_mappings: Vec::new(),
            anti_join: Some(CompiledAntiJoin {
                right_plan: right.plan,
                key_pairs: shared
                    .iter()
                    .map(|(_, left, right)| (*left, *right))
                    .collect(),
                right_schema: right.schema,
            }),
            optional_join: None,
            schema: left.schema,
            projection,
            collection: effective_collection,
            count_variable,
            full_row_distinct: !distincts.is_empty(),
        });
    }

    if !matches!(
        body,
        Query::And(_) | Query::Triple(_) | Query::Link(_) | Query::Data(_)
    ) {
        return Err(BgpPlanError::UnsupportedConstruct {
            construct: query_variant_name(body),
        });
    }

    // (1b) Flatten nested Ands. Sound constant-object Link/Data forms share one
    // canonical Triple representation here; variable-object forms reject because their
    // declared node/literal kind is not yet authenticated. Comparisons become
    // object-value range filters. True and Comment contribute no bindings and are
    // proof-transparent conjunction leaves.
    let mut triples: Vec<Triple> = Vec::new();
    let mut comparisons: Vec<NormalizedRange> = Vec::new();
    collect_conjunct(body, &mut triples, &mut comparisons)?;

    // (2) Lift each triple to an Atom, with all per-atom safety checks.
    let mut atoms: Vec<Atom> = triples
        .iter()
        .map(atom_from_triple)
        .collect::<Result<_, _>>()?;

    let k = atoms.len();
    if k == 0 {
        return Err(BgpPlanError::TooFewAtoms { count: k });
    }
    validate_variable_namespaces(&atoms)?;

    // (2b) Phase 7.4: route each comparison conjunct to a numeric RANGE filter on the
    // object of the (unique) atom that binds the compared variable. Consumes the
    // comparisons; the ranged variable stays a kept output column.
    attach_ranges(&mut atoms, &comparisons)?;

    // The Store's mixed-arity plan is a relational prefix followed by zero-arity
    // existence bits. Stable ordering preserves query order within both groups.
    atoms.sort_by_key(|atom| atom.kept.is_empty());

    // (3) Shape guard + left-deep planner: the safety-critical core.
    let (plan, schema) = plan_left_deep(&atoms)?;

    // A full BGP binding is already unique: for a fixed assignment every triple atom
    // identifies at most one row in the Store's set-valued triple relation. Therefore
    // Distinct over exactly the full schema is proof-transparent. Distinct over a subset
    // (or around projection/count) needs an authenticated deduplication operator.
    if !distincts.is_empty() {
        if !selections.is_empty() || count_variable.is_some() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        for variables in &distincts {
            let covers_schema = variables.len() == schema.len()
                && schema.iter().all(|schema_var| {
                    variables
                        .iter()
                        .filter(|candidate| *candidate == schema_var)
                        .count()
                        == 1
                });
            if !covers_schema {
                return Err(BgpPlanError::DistinctNotFullSchema);
            }
        }
    }

    // (4) Validate every nested projection against what its inner query exposes. The
    // proof still authenticates the complete BGP; only the outermost visible Select
    // determines which committed columns the caller reads.
    let projection = compile_projection(&selections, count_variable.as_deref(), &schema)?;

    Ok(CompiledBgp {
        plan,
        union_plans: Vec::new(),
        union_schema_mappings: Vec::new(),
        anti_join: None,
        optional_join: None,
        schema,
        projection,
        collection,
        count_variable,
        full_row_distinct: !distincts.is_empty(),
    })
}

fn split_top_level_not(query: &Query, positive: &mut Vec<Query>, negative: &mut Vec<Query>) {
    match query {
        Query::And(and) => {
            for child in &and.and {
                split_top_level_not(child, positive, negative);
            }
        }
        Query::Not(not) => negative.push((*not.query).clone()),
        other => positive.push(other.clone()),
    }
}

fn split_top_level_optional(query: &Query, positive: &mut Vec<Query>, optional: &mut Vec<Query>) {
    match query {
        Query::And(and) => {
            for child in &and.and {
                split_top_level_optional(child, positive, optional);
            }
        }
        Query::WoqlOptional(inner) => optional.push((*inner.query).clone()),
        other => positive.push(other.clone()),
    }
}

fn compile_projection(
    selections: &[(&[String], bool)],
    count_variable: Option<&str>,
    schema: &[String],
) -> Result<Option<Vec<usize>>, BgpPlanError> {
    if let Some(count) = count_variable {
        let outside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| outside.then_some(*vars))
            .collect();
        let inside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| (!outside).then_some(*vars))
            .collect();
        validate_selection_chain(&inside, schema)?;
        validate_selection_chain(&outside, &[count.to_owned()])?;
        Ok(Some(Vec::new()))
    } else if selections.is_empty() {
        // Ground relations use one authenticated internal UNIT column that remains
        // hidden from the public WOQL schema, including a bag union of ground branches.
        Ok(schema.is_empty().then(Vec::new))
    } else {
        let selections: Vec<&[String]> = selections.iter().map(|(vars, _)| *vars).collect();
        validate_selection_chain(&selections, schema)?;
        Ok(Some(
            selections[0]
                .iter()
                .map(|var| {
                    schema
                        .iter()
                        .position(|candidate| candidate == var)
                        .expect("validated outer projection must occur in the BGP schema")
                })
                .collect(),
        ))
    }
}

/// Validate nested Selects listed outer-to-inner. Each Select may only name variables
/// exposed by its directly nested query; the innermost Select reads from `base`.
fn validate_selection_chain(selections: &[&[String]], base: &[String]) -> Result<(), BgpPlanError> {
    let mut available = base;
    for selected in selections.iter().rev() {
        for var in *selected {
            if !available.iter().any(|candidate| candidate == var) {
                return Err(BgpPlanError::SelectedVariableNotInBgp { var: var.clone() });
            }
        }
        available = selected;
    }
    Ok(())
}

/// Flatten a query body recursively, partitioning leaves into canonical triple patterns
/// and comparisons. Any other construct rejects closed.
fn collect_conjunct<'a>(
    query: &'a Query,
    triples: &mut Vec<Triple>,
    comparisons: &mut Vec<NormalizedRange>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::Triple(triple) => triples.push(triple.clone()),
        Query::Link(link) => triples.push(triple_from_link(link)?),
        Query::Data(data) => triples.push(triple_from_data(data)?),
        Query::And(inner) => {
            for query in &inner.and {
                collect_conjunct(query, triples, comparisons)?;
            }
        }
        Query::Greater(_) | Query::Less(_) | Query::Gte(_) | Query::Lte(_) => {
            comparisons.push(normalize_comparison(query)?)
        }
        Query::InRange(range) => comparisons.push(normalize_in_range(range)?),
        Query::Pin(pin) => collect_conjunct(pin.query.as_ref(), triples, comparisons)?,
        Query::Immediately(immediately) => {
            collect_conjunct(immediately.query.as_ref(), triples, comparisons)?
        }
        Query::From(from) if from.graph.eq_ignore_ascii_case("instance") => {
            collect_conjunct(from.query.as_ref(), triples, comparisons)?
        }
        Query::From(_) => return Err(BgpPlanError::SchemaGraphUnsupported),
        Query::True(_) | Query::Comment(_) => {}
        other => {
            return Err(BgpPlanError::ConjunctNotTriple {
                construct: query_variant_name(other),
            })
        }
    }
    Ok(())
}

fn triple_from_link(link: &Link) -> Result<Triple, BgpPlanError> {
    let object = match &link.object {
        NodeValue::Node(node) => Value::Node(node.clone()),
        NodeValue::Variable(_) => {
            return Err(BgpPlanError::TypedObjectVariable { construct: "Link" })
        }
    };
    Ok(Triple {
        subject: link.subject.clone(),
        predicate: link.predicate.clone(),
        object,
        graph: Some(link.graph.clone()),
    })
}

fn triple_from_data(data: &Data) -> Result<Triple, BgpPlanError> {
    let object = match &data.object {
        DataValue::Data(value) => Value::Data(value.clone()),
        DataValue::List(values) => Value::List(values.iter().map(value_from_data_value).collect()),
        DataValue::Variable(_) => {
            return Err(BgpPlanError::TypedObjectVariable { construct: "Data" })
        }
    };
    Ok(Triple {
        subject: data.subject.clone(),
        predicate: data.predicate.clone(),
        object,
        graph: Some(data.graph.clone()),
    })
}

fn value_from_data_value(value: &DataValue) -> Value {
    match value {
        DataValue::Data(value) => Value::Data(value.clone()),
        DataValue::Variable(var) => Value::Variable(var.clone()),
        DataValue::List(values) => Value::List(values.iter().map(value_from_data_value).collect()),
    }
}

/// Lift one `Triple` to an [`Atom`]: the predicate must be a constant IRI; the subject
/// and object are each either a query VARIABLE (a kept output column) or an aligned
/// CONSTANT (emitted as an [`AtomTerm`] the store authenticates against its node/value
/// dictionary); and the graph must be the instance graph.
///
/// Accepted constants:
/// * subject/object node IRIs → [`AtomTerm::NodeConstant`] (CP2);
/// * object typed literals of a round-trip-aligned xsd datatype → [`AtomTerm::ValueConstant`]
///   (CP3, via [`value_constant_term`]).
///
/// Rejected closed: cross-namespace reuse of one variable in predicate and S/O positions,
/// schema-graph triples, dictionary/list object values, and unaligned typed-literal datatypes. A
/// value-literal SUBJECT is unrepresentable — woql2's
/// subject is a [`NodeValue`] (node/variable only) — so the store's `validate_atom`
/// value-subject guard is unreachable through this compiler by construction.
fn atom_from_triple(t: &Triple) -> Result<Atom, BgpPlanError> {
    // Graph: None (defaults to instance) or explicit Instance only.
    match &t.graph {
        None | Some(GraphType::Instance) => {}
        Some(GraphType::Schema) => return Err(BgpPlanError::SchemaGraphUnsupported),
    }

    let (predicate_term, pred_var) = match &t.predicate {
        NodeValue::Node(iri) => (iri.clone().into_bytes(), None),
        NodeValue::Variable(name) => (Vec::new(), Some(name.clone())),
    };

    // Subject: a variable (kept column) or a node-IRI constant.
    let (subject_term, subj_var) = match &t.subject {
        NodeValue::Variable(name) => (AtomTerm::Variable, Some(name.clone())),
        NodeValue::Node(iri) => (AtomTerm::NodeConstant(iri.clone().into_bytes()), None),
    };

    // Object: a variable (kept column), a node-IRI constant, or an aligned typed literal.
    let (object_term, obj_var) = match &t.object {
        Value::Variable(name) => (AtomTerm::Variable, Some(name.clone())),
        Value::Node(iri) => (AtomTerm::NodeConstant(iri.clone().into_bytes()), None),
        Value::Data(xsd) => (value_constant_term(xsd)?, None),
        Value::Dictionary(_) => {
            return Err(BgpPlanError::UnsupportedObjectValue { kind: "dictionary" })
        }
        Value::List(_) => return Err(BgpPlanError::UnsupportedObjectValue { kind: "list" }),
    };

    let subject_object_equal = matches!((&subj_var, &obj_var), (Some(s), Some(o)) if s == o);

    // Kept columns are materialized in S,P,O order. Predicate IDs occupy an independent
    // Store namespace, so one WOQL variable may not alias P with S or O.
    let mut kept = Vec::with_capacity(3);
    if let Some(s) = &subj_var {
        kept.push(s.clone());
    }
    if let Some(p) = &pred_var {
        if kept.contains(p) {
            return Err(BgpPlanError::VariableNamespaceConflict { var: p.clone() });
        }
        kept.push(p.clone());
    }
    if let Some(o) = &obj_var {
        if pred_var.as_ref() == Some(o) {
            return Err(BgpPlanError::VariableNamespaceConflict { var: o.clone() });
        }
        if !kept.contains(o) {
            kept.push(o.clone());
        }
    }
    Ok(Atom {
        layer_atom: LayerBgpAtom {
            subject: subject_term,
            predicate_term,
            predicate_variable: pred_var.is_some(),
            object: object_term,
            // CP-D Step 1 (migration): every atom starts with no object range. Step 2
            // fills this in for atoms whose object variable a comparison conjunct ranges.
            object_range: None,
            subject_object_equal,
        },
        kept,
        subj_var,
        pred_var,
        obj_var,
    })
}

fn validate_variable_namespaces(atoms: &[Atom]) -> Result<(), BgpPlanError> {
    for atom in atoms {
        if let Some(predicate) = &atom.pred_var {
            if atoms.iter().any(|candidate| {
                candidate.subj_var.as_ref() == Some(predicate)
                    || candidate.obj_var.as_ref() == Some(predicate)
            }) {
                return Err(BgpPlanError::VariableNamespaceConflict {
                    var: predicate.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Route each numeric comparison conjunct to an object-value RANGE filter on the atom
/// that binds the compared variable. For each comparison:
///
/// 1. [`normalize_comparison`] turns it into `(var, op, datatype, c)`, orienting the
///    operator so `c` is always on the right (`k > ?a` ⇒ `?a < k`), rejecting closed a
///    var-vs-var / const-vs-const comparison or an unsupported constant datatype.
/// 2. v1 constraints (all reject closed): `var` must be the object of EXACTLY ONE atom,
///    must NOT appear as any atom's subject, and each `var` may carry at most two bounds.
/// 3. The [`ObjectRangeSpec`] attaches to that atom's object (its object stays a kept
///    variable — the range is a filter, not a projection change).
fn attach_ranges(atoms: &mut [Atom], comparisons: &[NormalizedRange]) -> Result<(), BgpPlanError> {
    for comparison in comparisons {
        let var = comparison.var.clone();
        // The ranged variable may not occupy a subject position anywhere.
        if atoms
            .iter()
            .any(|a| a.subj_var.as_deref() == Some(var.as_str()))
        {
            return Err(BgpPlanError::RangeVariableIsSubject { var });
        }
        // It must be the object variable of exactly one atom.
        let mut object_of: Vec<usize> = Vec::new();
        for (i, a) in atoms.iter().enumerate() {
            if a.obj_var.as_deref() == Some(var.as_str()) {
                object_of.push(i);
            }
        }
        let idx = match object_of.as_slice() {
            [] => return Err(BgpPlanError::RangeVariableNotObject { var }),
            [i] => *i,
            _ => return Err(BgpPlanError::RangeVariableObjectOfMultiple { var }),
        };

        // The matched atom's object is a Variable by construction (obj_var is Some only
        // for a variable object), so a range never lands on a constant object here.
        debug_assert!(matches!(atoms[idx].layer_atom.object, AtomTerm::Variable));
        for bound in &comparison.bounds {
            match &mut atoms[idx].layer_atom.object_range {
                None => {
                    atoms[idx].layer_atom.object_range = Some(ObjectRangeSpec {
                        op: bound.op,
                        datatype: comparison.datatype,
                        c: bound.c,
                        b_bits: OBJECT_RANGE_B_BITS,
                        second: None,
                    });
                }
                Some(existing) => {
                    if existing.datatype != comparison.datatype {
                        return Err(BgpPlanError::RangeDatatypeMismatch { var });
                    }
                    if existing.second.is_some() {
                        return Err(BgpPlanError::DuplicateRangeOnVariable { var });
                    }
                    existing.second = Some(ObjectRangeBound {
                        op: bound.op,
                        c: bound.c,
                    });
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
struct NormalizedBound {
    op: RangeOp,
    c: ArkFr,
}

#[derive(Clone)]
struct NormalizedRange {
    var: String,
    datatype: ArkFr,
    bounds: Vec<NormalizedBound>,
}

/// Normalize a numeric comparison conjunct into `(var, op, datatype, c)`, where `op` is
/// oriented so the constant is on the RIGHT (`Greater(k, ?a)` ⇒ `?a < k`), and
/// `(datatype, c)` is the store-aligned encoding of the constant (see
/// [`encode_range_constant`]).
///
/// Rejects closed: both sides variables ([`BgpPlanError::RangeBothVariables`]), neither
/// side a variable ([`BgpPlanError::RangeBothConstants`]), or a constant that is not a
/// supported order-preserving numeric literal ([`BgpPlanError::UnsupportedRangeDatatype`]).
fn normalize_comparison(q: &Query) -> Result<NormalizedRange, BgpPlanError> {
    // `op_var_left` is the operator when the VARIABLE is the left operand; `op_var_right`
    // is its flip (variable on the right) — the same relation with the sides swapped.
    let (left, right, op_var_left, op_var_right) = match q {
        Query::Greater(g) => (&g.left, &g.right, RangeOp::Gt, RangeOp::Lt),
        Query::Less(l) => (&l.left, &l.right, RangeOp::Lt, RangeOp::Gt),
        Query::Gte(g) => (&g.left, &g.right, RangeOp::Ge, RangeOp::Le),
        Query::Lte(l) => (&l.left, &l.right, RangeOp::Le, RangeOp::Ge),
        // Only comparison variants are collected into `comparisons`.
        _ => unreachable!("normalize_comparison only sees Greater/Less/Gte/Lte"),
    };

    let (var, op, konst): (String, RangeOp, &DataValue) = match (left, right) {
        (DataValue::Variable(_), DataValue::Variable(_)) => {
            return Err(BgpPlanError::RangeBothVariables)
        }
        (DataValue::Variable(v), other) => (v.clone(), op_var_left, other),
        (other, DataValue::Variable(v)) => (v.clone(), op_var_right, other),
        // Neither side is a variable.
        _ => return Err(BgpPlanError::RangeBothConstants),
    };

    let xsd = match konst {
        DataValue::Data(k) => k,
        DataValue::List(_) => {
            return Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "list" })
        }
        // Excluded above (the variable was the other operand).
        DataValue::Variable(_) => unreachable!(),
    };
    let (datatype, c) = encode_range_constant(xsd)?;
    Ok(NormalizedRange {
        var,
        datatype,
        bounds: vec![NormalizedBound { op, c }],
    })
}

fn normalize_in_range(range: &InRange) -> Result<NormalizedRange, BgpPlanError> {
    let var = match &range.value {
        DataValue::Variable(var) => var.clone(),
        _ => return Err(BgpPlanError::InRangeValueNotVariable),
    };
    let start = match &range.start {
        DataValue::Data(value) => value,
        _ => return Err(BgpPlanError::InRangeBoundNotConstant),
    };
    let end = match &range.end {
        DataValue::Data(value) => value,
        _ => return Err(BgpPlanError::InRangeBoundNotConstant),
    };
    let (start_datatype, start_c) = encode_range_constant(start)?;
    let (end_datatype, end_c) = encode_range_constant(end)?;
    if start_datatype != end_datatype {
        return Err(BgpPlanError::RangeDatatypeMismatch { var });
    }
    Ok(NormalizedRange {
        var,
        datatype: start_datatype,
        bounds: vec![
            NormalizedBound {
                op: RangeOp::Ge,
                c: start_c,
            },
            NormalizedBound {
                op: RangeOp::Lt,
                c: end_c,
            },
        ],
    })
}

/// Build the store's typed dict entry for a woql2 SCALAR literal, routing each supported
/// xsd datatype through its `terminus_store`/`tdb-succinct` typed-value constructor (the
/// [`TdbDataType::make_entry`] family) so the resulting canonicalization is byte-identical
/// to a stored value of the same literal (no hand-rolled byte layouts — the 7.5c alignment
/// rule). Returns the entry, or the xsd datatype name for a literal this compiler does not
/// support (the caller wraps that name into the right rejection error).
///
/// `xsd:string` is handled separately by the caller via
/// [`ValueTriple::new_string_value`]. Invalid scalar lexical forms reject closed.
fn xsd_scalar_entry(xsd: &XSDAnySimpleType) -> Result<TypedDictEntry, &'static str> {
    Ok(match xsd {
        XSDAnySimpleType::Boolean(b) => bool::make_entry(b),
        // xsd:unsignedInt is the 32-bit unsigned type → store `UInt32`.
        XSDAnySimpleType::UnsignedInt(u) => {
            let v = u32::try_from(*u).map_err(|_| "unsignedInt (exceeds u32)")?;
            u32::make_entry(&v)
        }
        // The woql2 `Integer` variant carries an `i64` → store `Int64` (fixed-width,
        // order-preserving via offset-binary — so it also supports range).
        XSDAnySimpleType::Integer(i) => i64::make_entry(i),
        // The woql2 `Float` variant carries an `f64` → store `Float64` (8-byte IEEE
        // total-order OP encoding).
        XSDAnySimpleType::Float(f) => f64::make_entry(f),
        XSDAnySimpleType::Decimal(decimal) => {
            let value = tdb_succinct::Decimal::new(decimal.to_string()).map_err(|_| "decimal")?;
            tdb_succinct::Decimal::make_entry(&value)
        }
        XSDAnySimpleType::URI(uri) => AnyURI::make_entry(uri),
        // The WOQL AST carries a UTC DateTime; the Store's xsd:dateTime entry is its
        // timezone-free UTC representation.
        XSDAnySimpleType::DateTime(datetime) => {
            chrono::NaiveDateTime::make_entry(&datetime.naive_utc())
        }
        XSDAnySimpleType::Date(date) => Date::make_entry(&Date {
            year: i64::from(date.year()),
            month: date.month() as u8,
            day: date.day() as u8,
            offset: 0,
        }),
        XSDAnySimpleType::Time(time) => chrono::NaiveTime::make_entry(time),
        XSDAnySimpleType::HexBinary(value) => {
            let bytes = hex::decode(value).map_err(|_| "hexBinary (invalid lexical value)")?;
            HexBinary::make_entry(&HexBinary(bytes))
        }
        other => return Err(xsd_datatype_name(other)),
    })
}

/// Encode a numeric range constant to `(datatype_tag, c)` matching the store's committed
/// node/value dictionary encoding EXACTLY, by routing through the store's OWN value
/// canonicalization (7.5c alignment trick): build the typed value via [`xsd_scalar_entry`]
/// + [`ValueTriple::new_value`], read back its [`CanonicalObject::Value`] via
/// [`CanonicalObject::from_store_object`], take its [`CanonicalDatatype`] discriminant as the
/// datatype tag, and compute `c` as the big-endian unsigned integer of its order-preserving
/// `to_lexical` bytes — the SAME conversion `terminus_store`'s (private) `node_value_op_value`
/// applies to the committed dict value column.
///
/// Supported (round-trip-verified, all order-preserving fixed-width): `xsd:boolean`
/// (`Boolean`), `xsd:unsignedInt` (`UInt32`), `xsd:integer` (`Int64`), `xsd:float`/`double`
/// (`Float64`). Every other `XSDAnySimpleType` rejects closed: a range over a
/// non-order-preserving or variable-width datatype (string/decimal/…) would compare a
/// `0` sentinel in the store, which is unsound at the query level. Adding a datatype
/// requires a store-routed construction in [`xsd_scalar_entry`], an
/// [`op_preserving_width`] entry, PLUS a round-trip alignment test.
fn encode_range_constant(xsd: &XSDAnySimpleType) -> Result<(ArkFr, ArkFr), BgpPlanError> {
    if !matches!(
        xsd,
        XSDAnySimpleType::Boolean(_)
            | XSDAnySimpleType::UnsignedInt(_)
            | XSDAnySimpleType::Integer(_)
            | XSDAnySimpleType::Float(_)
    ) {
        return Err(BgpPlanError::UnsupportedRangeDatatype {
            datatype: xsd_datatype_name(xsd),
        });
    }
    let entry = xsd_scalar_entry(xsd)
        .map_err(|datatype| BgpPlanError::UnsupportedRangeDatatype { datatype })?;
    encode_via_store(entry)
}

/// Canonicalize a `TypedDictEntry` through the store's own value path and derive the
/// `(datatype_tag, c)` pair. Fails closed if the resulting datatype is not one of the
/// round-trip-verified order-preserving fixed-width numerics (see [`op_preserving_width`]).
fn encode_via_store(entry: TypedDictEntry) -> Result<(ArkFr, ArkFr), BgpPlanError> {
    // Subject/predicate are irrelevant to the object canonicalization.
    let triple = ValueTriple::new_value("", "", entry);
    let (datatype, lexical) = match CanonicalObject::from_store_object(&triple.object) {
        CanonicalObject::Value { datatype, lexical } => (datatype, lexical),
        // `new_value` always yields a Value object; fail closed rather than emit an
        // unaligned constant.
        CanonicalObject::Node(_) => {
            return Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "node" })
        }
    };
    // Guard: only order-preserving fixed-width numerics are sound to range over.
    let width = op_preserving_width(datatype).ok_or(BgpPlanError::UnsupportedRangeDatatype {
        datatype: "non-order-preserving datatype",
    })?;
    if lexical.len() != width {
        return Err(BgpPlanError::UnsupportedRangeDatatype {
            datatype: "unexpected lexical width",
        });
    }
    // c = big-endian unsigned integer of the fixed-width OP bytes (matches the store's
    // `node_value_op_value`). Widths are ≤ 8, so it fits a u64.
    let mut buf = [0u8; 8];
    buf[8 - width..].copy_from_slice(&lexical);
    let c = ArkFr::from_u64(u64::from_be_bytes(buf));
    let datatype_tag = ArkFr::from_u64(datatype as u64);
    Ok((datatype_tag, c))
}

/// Fixed byte width of a SUPPORTED order-preserving numeric datatype, or `None` for any
/// datatype whose lexical is variable-width or not order-preserving. Mirrors
/// `terminus_store`'s private `order_preserving_numeric_width` EXACTLY: for each of these
/// the store's `to_lexical` writes exactly `width` bytes whose big-endian unsigned read is
/// monotone in value (`Boolean` 0/1; `(U)Int8/16/32/64` raw-BE / offset-binary;
/// `Float32/64` IEEE total-order), so `node_value_op_value` and this compiler compute the
/// SAME `c`. A range over any other datatype rejects closed (see [`encode_range_constant`]).
fn op_preserving_width(datatype: CanonicalDatatype) -> Option<usize> {
    use CanonicalDatatype::*;
    Some(match datatype {
        Boolean => 1,
        UInt8 | Int8 => 1,
        UInt16 | Int16 => 2,
        UInt32 | Int32 | Float32 => 4,
        UInt64 | Int64 | Float64 => 8,
        _ => return None,
    })
}

/// The xsd datatype name of a woql2 typed literal, for descriptive rejection messages.
fn xsd_datatype_name(xsd: &XSDAnySimpleType) -> &'static str {
    match xsd {
        XSDAnySimpleType::String(_) => "string",
        XSDAnySimpleType::Decimal(_) => "decimal",
        XSDAnySimpleType::Float(_) => "float",
        XSDAnySimpleType::Boolean(_) => "boolean",
        XSDAnySimpleType::HexBinary(_) => "hexBinary",
        XSDAnySimpleType::URI(_) => "anyURI",
        XSDAnySimpleType::DateTime(_) => "dateTime",
        XSDAnySimpleType::Date(_) => "date",
        XSDAnySimpleType::Time(_) => "time",
        XSDAnySimpleType::UnsignedInt(_) => "unsignedInt",
        XSDAnySimpleType::Integer(_) => "integer",
    }
}

/// Emit an [`AtomTerm::ValueConstant`] for an object typed literal — but ONLY for xsd
/// datatypes whose `(CanonicalDatatype, lexical bytes)` this compiler can produce so they
/// canonicalize IDENTICALLY to the store's own encoding of the same literal. Any other
/// datatype rejects closed rather than authenticate a constant whose alignment is unproven
/// (a misaligned `(datatype, lexical)` would `hash_object` to the wrong dictionary row, so
/// membership would fail — or, worse, authenticate the wrong constant).
///
/// Equality does NOT require order-preservation (it is id-based via dictionary membership),
/// so ANY datatype the store can canonicalize is sound here. Supported (round-trip-verified,
/// each routed through the store's OWN value construction so the bytes match BY CONSTRUCTION,
/// no re-implementation of the tdb-succinct encoding):
/// * `xsd:string` via [`ValueTriple::new_string_value`] (the exact path the store uses to
///   store a string value);
/// * every other supported WOQL scalar via [`xsd_scalar_entry`] →
///   [`ValueTriple::new_value`] using its matching `tdb-succinct` typed entry.
///
/// See the `object_*_literal_*` round-trip tests. Invalid scalar lexical forms reject closed.
fn value_constant_term(xsd: &XSDAnySimpleType) -> Result<AtomTerm, BgpPlanError> {
    // Build the store object via the store's OWN construction path for each datatype. String
    // uses the dedicated string-value constructor; every other supported scalar goes through
    // its typed dict entry. Subject/predicate are irrelevant to the object canonicalization,
    // so empty placeholders are fine.
    let object = match xsd {
        XSDAnySimpleType::String(s) => ValueTriple::new_string_value("", "", s).object,
        _ => {
            let entry = xsd_scalar_entry(xsd)
                .map_err(|datatype| BgpPlanError::UnsupportedValueDatatype { datatype })?;
            ValueTriple::new_value("", "", entry).object
        }
    };
    match CanonicalObject::from_store_object(&object) {
        CanonicalObject::Value { datatype, lexical } => {
            Ok(AtomTerm::ValueConstant(datatype, lexical))
        }
        // Unreachable: a value construction always yields a `Value` object. Fail closed
        // rather than emit an unaligned constant.
        CanonicalObject::Node(_) => Err(BgpPlanError::UnsupportedValueDatatype {
            datatype: xsd_datatype_name(xsd),
        }),
    }
}

/// The safety-critical left-deep planner. Tracks the accumulator schema as an ordered
/// list of distinct variable names and emits atoms (in placement order) + `k-1` join
/// steps. Each step joins the accumulator with an unplaced atom on the SINGLE shared
/// variable; the accumulator schema evolves as `[r_other.., key, s_other..]`, matching
/// `terminus_store`'s `bgp` output ordering exactly. If no remaining atom shares a
/// variable, the next input-order component starts with a verifier-known UNIT-key
/// Cartesian step whose output ordering is `[acc.., relation..]`.
///
/// Each atom relation carries ONE column per VARIABLE, in S,P,O order (`atom.kept`); a
/// constant removes its position from `kept`. `rel_key` is the shared variable's index within
/// the atom's KEPT columns, and `s_other` is the atom's remaining kept columns (possibly
/// empty for a 1-variable atom).
fn plan_left_deep(atoms: &[Atom]) -> Result<(LayerBgpPlan, Vec<String>), BgpPlanError> {
    let k = atoms.len();

    let ground_count = atoms.iter().filter(|atom| atom.kept.is_empty()).count();
    if ground_count == k {
        return Ok((
            LayerBgpPlan {
                atoms: atoms.iter().map(|atom| atom.layer_atom.clone()).collect(),
                joins: Vec::new(),
            },
            Vec::new(),
        ));
    }
    let relational_count = k - ground_count;

    // Deterministic start atom: input order 0. Its kept variables seed the accumulator.
    let mut acc_vars: Vec<String> = atoms[0].kept.clone();
    let mut placed = vec![false; k];
    placed[0] = true;

    let mut ordered_atoms: Vec<LayerBgpAtom> = vec![atoms[0].layer_atom.clone()];
    let mut joins: Vec<JoinStep> = Vec::with_capacity(relational_count - 1);

    let mut placed_count = 1usize;
    while placed_count < relational_count {
        // Find an unplaced atom sharing at least one variable with the accumulator.
        let mut chosen: Option<usize> = None;
        for (i, atom) in atoms.iter().take(relational_count).enumerate() {
            if placed[i] {
                continue;
            }
            if shared_count(&acc_vars, &atom.kept) >= 1 {
                chosen = Some(i);
                break;
            }
        }

        let i = chosen.unwrap_or_else(|| {
            placed
                .iter()
                .take(relational_count)
                .position(|is_placed| !is_placed)
                .expect("an unplaced relational atom must remain")
        });

        // Join on every shared variable. The first pair remains the scalar-key fields;
        // further pairs activate the Store chain's authenticated composite-key mode.
        let atom = &atoms[i];
        let shared: Vec<(usize, usize)> = atom
            .kept
            .iter()
            .enumerate()
            .filter_map(|(rel, variable)| {
                acc_vars
                    .iter()
                    .position(|acc| acc == variable)
                    .map(|acc| (acc, rel))
            })
            .collect();
        if shared.is_empty() {
            joins.push(JoinStep::cross_product());
            acc_vars.extend(atom.kept.iter().cloned());
            ordered_atoms.push(atom.layer_atom.clone());
            placed[i] = true;
            placed_count += 1;
            continue;
        }
        let (acc_key, rel_key) = shared[0];
        let v = atom.kept[rel_key].clone();
        let additional_keys = shared[1..].to_vec();

        if additional_keys.is_empty() {
            // Scalar path preserves the audited binary-join output ordering.
            let mut r_other = acc_vars.clone();
            r_other.remove(acc_key);
            let mut s_other = atom.kept.clone();
            s_other.remove(rel_key);
            r_other.push(v);
            r_other.extend(s_other);
            acc_vars = r_other;
        } else {
            // Composite path retains the accumulator in place and appends only variables
            // not already represented by one of the shared key columns.
            acc_vars.extend(
                atom.kept
                    .iter()
                    .filter(|variable| !acc_vars.contains(variable))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
        }

        joins.push(JoinStep {
            acc_key,
            rel_key,
            additional_keys,
        });
        ordered_atoms.push(atom.layer_atom.clone());
        placed[i] = true;
        placed_count += 1;
    }

    ordered_atoms.extend(
        atoms[relational_count..]
            .iter()
            .map(|atom| atom.layer_atom.clone()),
    );

    Ok((
        LayerBgpPlan {
            atoms: ordered_atoms,
            joins,
        },
        acc_vars,
    ))
}

/// How many of an atom's KEPT (variable) columns occur in the accumulator schema. Because
/// repeated variables within an atom are represented once after their same-row equality
/// constraint is recorded, and `acc_vars` also holds distinct names, so this is an exact
/// shared-variable count in `0..=kept.len()`.
fn shared_count(acc_vars: &[String], kept: &[String]) -> usize {
    kept.iter()
        .filter(|kv| acc_vars.iter().any(|a| a == *kv))
        .count()
}
