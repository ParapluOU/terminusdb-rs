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
    verify_global_left_outer_join, verify_global_not, verify_left_anti_join_with_keys,
    verify_left_outer_join_with_keys, LeftAntiJoinProof, LeftOuterJoinProof,
};
use terminus_store::proof::argument::{StoreProverContext, Transcript254};
use terminus_store::proof::bgp::{JoinStep, RelationCommit};
use terminus_store::proof::commitment::{
    LayerCommitment, VerifierCommitment, VerifierLayer,
};
use terminus_store::proof::envelope::{
    plan_digest, resolve_relation_equality_constraint, EqualityValueDomain, LayerBgpEnvelope,
    RecursiveRelationEnvelope, RecursiveRelationProof, RelationEqualityPlan, RelationProofPlan,
    RelationProofPlanKind, MAX_RELATION_PROOF_DEPTH, MAX_RELATION_PROOF_NODES,
};
use terminus_store::proof::equality::{
    verify_relation_equality, RelationEqualityProof,
};
use terminus_store::proof::group::{
    verify_group_extrema, GroupExtremaKind, GroupExtremaProof,
};
use terminus_store::proof::query::{
    resolve_result_values, resolve_typed_result_objects, result_id_namespaces, verify_layer_bgp,
    verify_layer_bgp_result_log_domain,
    verify_result_columns, AtomTerm, CanonicalResultValue, LayerBgpAtom, LayerBgpPlan,
    LayerBgpProof, ObjectRangeBound, ObjectRangeSpec, ResultColumnDescriptor, ResultIdNamespace,
};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::union::{verify_bag_union, BagUnionProof};
use terminus_store::proof::{CanonicalDatatype, CanonicalObject, ProofHash};
use terminus_store::store::sync::SyncStoreLayer;
use terminus_store::ValueTriple;

use crate::misc::InRange;
use crate::query::Query;
use crate::triple::Triple;
use crate::value::{DataValue, NodeValue, Value};
use terminusdb_schema::{GraphType, XSDAnySimpleType};

/// A compiled, variable-blind BGP plan plus the metadata a caller needs to read the
/// proof's result columns.
#[derive(Clone, Debug)]
pub struct CompiledBgp {
    /// The one authoritative recursive authenticated relation.
    pub relation: CompiledRelation,
    /// If a `Select` wrapped the BGP, the selected variables' column indices into
    /// `relation.schema`. The proof still proves the full relation; projection is which output
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

#[derive(Clone, Debug)]
pub struct CompiledRelation {
    pub schema: Vec<String>,
    /// Exact executor value domains known for each schema column. `None` means the
    /// node/value dictionary column may contain both nodes and differently typed values.
    pub domains: Vec<Option<EqualityValueDomain>>,
    pub statement: CompiledRelationStatement,
}

#[derive(Clone, Debug)]
pub enum CompiledRelationStatement {
    Bgp(LayerBgpPlan),
    BagUnion {
        branches: Vec<CompiledRelation>,
        /// Public output column -> child relation column, per ordered branch.
        mappings: Vec<Vec<usize>>,
    },
    LeftAnti {
        left: Box<CompiledRelation>,
        right: Box<CompiledRelation>,
        key_pairs: Vec<(usize, usize)>,
    },
    LeftOuter {
        left: Box<CompiledRelation>,
        right: Box<CompiledRelation>,
        key_pairs: Vec<(usize, usize)>,
    },
    Equality {
        input: Box<CompiledRelation>,
        constraint: RelationEqualityPlan,
    },
    GroupExtrema {
        input: Box<CompiledRelation>,
        key_column: usize,
        value_column: usize,
        datatype: CanonicalDatatype,
        kind: GroupExtremaKind,
    },
}

impl CompiledRelation {
    pub fn as_bgp(&self) -> Option<&LayerBgpPlan> {
        match &self.statement {
            CompiledRelationStatement::Bgp(plan) => Some(plan),
            _ => None,
        }
    }

    pub fn as_bag_union(&self) -> Option<(&[CompiledRelation], &[Vec<usize>])> {
        match &self.statement {
            CompiledRelationStatement::BagUnion { branches, mappings } => {
                Some((branches, mappings))
            }
            _ => None,
        }
    }

    pub fn as_left_anti(
        &self,
    ) -> Option<(&CompiledRelation, &CompiledRelation, &[(usize, usize)])> {
        match &self.statement {
            CompiledRelationStatement::LeftAnti {
                left,
                right,
                key_pairs,
            } => Some((left, right, key_pairs)),
            _ => None,
        }
    }

    pub fn as_left_anti_mut(&mut self) -> Option<&mut Vec<(usize, usize)>> {
        match &mut self.statement {
            CompiledRelationStatement::LeftAnti { key_pairs, .. } => Some(key_pairs),
            _ => None,
        }
    }

    pub fn as_left_outer(
        &self,
    ) -> Option<(&CompiledRelation, &CompiledRelation, &[(usize, usize)])> {
        match &self.statement {
            CompiledRelationStatement::LeftOuter {
                left,
                right,
                key_pairs,
            } => Some((left, right, key_pairs)),
            _ => None,
        }
    }

    pub fn as_left_outer_mut(&mut self) -> Option<&mut Vec<(usize, usize)>> {
        match &mut self.statement {
            CompiledRelationStatement::LeftOuter { key_pairs, .. } => Some(key_pairs),
            _ => None,
        }
    }

    pub fn as_equality(&self) -> Option<(&CompiledRelation, &RelationEqualityPlan)> {
        match &self.statement {
            CompiledRelationStatement::Equality { input, constraint } => Some((input, constraint)),
            _ => None,
        }
    }
}

impl CompiledBgp {
    /// Return the Store plan only when the authoritative relation is a leaf BGP.
    pub fn bgp_plan(&self) -> &LayerBgpPlan {
        self.relation
            .as_bgp()
            .expect("compiled relation is not a leaf BGP")
    }
}

fn compiled_result_descriptors(
    compiled: &CompiledBgp,
) -> std::io::Result<Vec<ResultColumnDescriptor>> {
    relation_result_descriptors(&compiled.relation)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutedResultValue {
    Id(u64),
    Null,
}

#[derive(Debug)]
pub enum ProvedQueryStatement {
    Single {
        proof: Box<LayerBgpProof>,
        predicate_ids: Vec<u64>,
    },
    BagUnion {
        branches: Vec<ProvedBgp>,
        proof: Box<BagUnionProof>,
    },
    LeftAnti {
        left: Box<ProvedBgp>,
        right: Box<ProvedBgp>,
        /// Correlated anti joins carry a proof; global Not is length-pinned.
        proof: Option<Box<LeftAntiJoinProof>>,
    },
    LeftOuter {
        left: Box<ProvedBgp>,
        right: Box<ProvedBgp>,
        proof: Box<LeftOuterJoinProof>,
    },
    Equality {
        input: Box<ProvedBgp>,
        proof: Box<RelationEqualityProof>,
    },
    GroupExtrema {
        input: Box<ProvedBgp>,
        proof: Box<GroupExtremaProof>,
    },
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

fn relation_query_digest(relation: &CompiledRelation) -> std::io::Result<ProofHash> {
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
        }]);
        hasher.update([descriptor.nullable as u8]);
    }
    match &relation.statement {
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
        CompiledRelationStatement::GroupExtrema {
            input,
            key_column,
            value_column,
            datatype,
            kind,
        } => {
            hasher.update(b"/group-extrema");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*key_column as u64).to_le_bytes());
            hasher.update((*value_column as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
            hasher.update([match kind {
                GroupExtremaKind::Min => 0,
                GroupExtremaKind::Max => 1,
            }]);
        }
    }
    Ok(hasher.finalize().into())
}

fn digest_equality_domain(hasher: &mut Sha256, domain: Option<EqualityValueDomain>) {
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

fn digest_canonical_object(hasher: &mut Sha256, value: &CanonicalObject) {
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

fn digest_equality_plan(hasher: &mut Sha256, plan: &RelationEqualityPlan) {
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

fn digest_key_pairs(hasher: &mut Sha256, key_pairs: &[(usize, usize)]) {
    hasher.update((key_pairs.len() as u64).to_le_bytes());
    for (left, right) in key_pairs {
        hasher.update((*left as u64).to_le_bytes());
        hasher.update((*right as u64).to_le_bytes());
    }
}

fn relation_result_descriptors(
    relation: &CompiledRelation,
) -> std::io::Result<Vec<ResultColumnDescriptor>> {
    match &relation.statement {
        CompiledRelationStatement::Bgp(plan) => {
            terminus_store::proof::query::result_column_descriptors(plan)
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
        CompiledRelationStatement::LeftAnti { left, .. } => relation_result_descriptors(left),
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
        CompiledRelationStatement::GroupExtrema {
            input, key_column, ..
        } => {
            let child = relation_result_descriptors(input)?;
            let key = *child.get(*key_column).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-extrema key column out of range",
                )
            })?;
            Ok(vec![
                key,
                ResultColumnDescriptor {
                    namespace: ResultIdNamespace::NodeValue,
                    nullable: false,
                },
            ])
        }
    }
}

fn relation_proof_plan(
    relation: &CompiledRelation,
    query_digest: ProofHash,
) -> std::io::Result<RelationProofPlan> {
    let child = |relation: &CompiledRelation| {
        let digest = relation_query_digest(relation)?;
        relation_proof_plan(relation, digest)
    };
    let kind = match &relation.statement {
        CompiledRelationStatement::Bgp(plan) => RelationProofPlanKind::Bgp(plan.clone()),
        CompiledRelationStatement::BagUnion { branches, mappings } => {
            RelationProofPlanKind::BagUnion {
                branches: branches.iter().map(child).collect::<std::io::Result<_>>()?,
                mappings: mappings.clone(),
            }
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
        CompiledRelationStatement::Equality { input, constraint } => {
            RelationProofPlanKind::Equality {
                input: Box::new(child(input)?),
                constraint: constraint.clone(),
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
            key_column,
            value_column,
            datatype,
            kind,
        } => RelationProofPlanKind::GroupExtrema {
            input: Box::new(child(input)?),
            key_column: *key_column,
            value_column: *value_column,
            datatype: *datatype,
            kind: *kind,
        },
    };
    Ok(RelationProofPlan { query_digest, kind })
}

impl ProvedBgp {
    pub fn as_single(&self) -> Option<(&LayerBgpProof, &[u64])> {
        match &self.statement {
            ProvedQueryStatement::Single {
                proof,
                predicate_ids,
            } => Some((proof, predicate_ids)),
            _ => None,
        }
    }

    pub fn as_bag_union(&self) -> Option<(&[ProvedBgp], &BagUnionProof)> {
        match &self.statement {
            ProvedQueryStatement::BagUnion { branches, proof } => Some((branches, proof)),
            _ => None,
        }
    }

    /// Verify against a caller-selected, independently trusted root.
    pub fn verify(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
    ) -> std::io::Result<()> {
        self.verify_relation(
            &compiled.relation,
            layer,
            expected_root,
            compiled_query_digest(compiled)?,
        )
    }

    fn relation_commit(&self, mapping: Option<&[usize]>) -> std::io::Result<RelationCommit> {
        let commitments = match mapping {
            Some(mapping) => mapping
                .iter()
                .map(|index| {
                    self.result_commitments.get(*index).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL recursive relation mapping out of range",
                        )
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?,
            None => self.result_commitments.clone(),
        };
        Ok(RelationCommit {
            commitments,
            log_domain: self.result_log_domain,
            n: self.result_len,
        })
    }

    fn verify_relation(
        &self,
        compiled: &CompiledRelation,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        query_digest: ProofHash,
    ) -> std::io::Result<()> {
        match (&self.statement, &compiled.statement) {
            (
                ProvedQueryStatement::Single {
                    proof,
                    predicate_ids,
                },
                CompiledRelationStatement::Bgp(plan),
            ) => {
                verify_layer_bgp(
                    layer,
                    expected_root,
                    plan,
                    predicate_ids,
                    &self.result_commitments,
                    self.result_len,
                    proof,
                )?;
                verify_layer_bgp_result_log_domain(layer, plan, proof, self.result_log_domain)?;
            }
            (
                ProvedQueryStatement::BagUnion { branches, proof },
                CompiledRelationStatement::BagUnion {
                    branches: branch_plans,
                    mappings,
                },
            ) => {
                if branches.len() < 2
                    || branches.len() != branch_plans.len()
                    || branches.len() != mappings.len()
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL recursive union branch mismatch",
                    ));
                }
                let mut inputs = Vec::with_capacity(branches.len());
                for ((branch, branch_plan), mapping) in
                    branches.iter().zip(branch_plans).zip(mappings)
                {
                    branch.verify_relation(
                        branch_plan,
                        layer,
                        expected_root,
                        relation_query_digest(branch_plan)?,
                    )?;
                    inputs.push(if compiled.schema.is_empty() {
                        branch.relation_commit(None)?
                    } else {
                        branch.relation_commit(Some(mapping))?
                    });
                }
                let mut transcript = Transcript254::new(b"parture-union-envelope");
                transcript.append_bytes(b"pow/union-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/union-envelope/query", &query_digest);
                verify_bag_union(
                    &inputs,
                    &self.result_commitments,
                    self.result_len,
                    self.result_log_domain,
                    proof,
                    &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::LeftAnti { left, right, proof },
                CompiledRelationStatement::LeftAnti {
                    left: left_plan,
                    right: right_plan,
                    key_pairs,
                },
            ) => {
                left.verify_relation(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?;
                right.verify_relation(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?;
                let left_commit = left.relation_commit(None)?;
                let right_commit = right.relation_commit(None)?;
                let mut transcript = Transcript254::new(b"parture-left-anti-envelope");
                transcript.append_bytes(b"pow/anti-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/anti-envelope/query", &query_digest);
                match (key_pairs.is_empty(), proof.as_ref()) {
                    (true, None) => verify_global_not(
                        &left_commit,
                        &right_commit,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                    )?,
                    (false, Some(proof)) => verify_left_anti_join_with_keys(
                        &left_commit,
                        &right_commit,
                        key_pairs,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                        proof,
                        &mut transcript,
                    )?,
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL recursive anti proof variant mismatch",
                        ))
                    }
                }
            }
            (
                ProvedQueryStatement::LeftOuter { left, right, proof },
                CompiledRelationStatement::LeftOuter {
                    left: left_plan,
                    right: right_plan,
                    key_pairs,
                },
            ) => {
                left.verify_relation(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?;
                right.verify_relation(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?;
                let left_commit = left.relation_commit(None)?;
                let right_commit = right.relation_commit(None)?;
                let mut transcript = Transcript254::new(b"parture-left-outer-envelope");
                transcript.append_bytes(b"pow/outer-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/outer-envelope/query", &query_digest);
                if key_pairs.is_empty() {
                    verify_global_left_outer_join(
                        &left_commit,
                        &right_commit,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                        proof,
                        &mut transcript,
                    )?;
                } else {
                    verify_left_outer_join_with_keys(
                        &left_commit,
                        &right_commit,
                        key_pairs,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                        proof,
                        &mut transcript,
                    )?;
                }
            }
            (
                ProvedQueryStatement::Equality { input, proof },
                CompiledRelationStatement::Equality {
                    input: input_plan,
                    constraint,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let resolved = resolve_relation_equality_constraint(layer, constraint)?;
                let mut transcript = Transcript254::new(b"parture-equality-envelope");
                transcript.append_bytes(b"pow/equality-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/equality-envelope/query", &query_digest);
                verify_relation_equality(
                    &input.relation_commit(None)?,
                    &resolved,
                    &self.result_commitments,
                    self.result_len,
                    self.result_log_domain,
                    proof,
                    &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::GroupExtrema { input, proof },
                CompiledRelationStatement::GroupExtrema {
                    input: input_plan,
                    key_column,
                    value_column,
                    datatype,
                    kind,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let rows =
                    group_output_rows(&self.result_columns, self.result_len, compiled, layer)?;
                let mut transcript = Transcript254::new(b"parture-group-extrema-envelope");
                transcript.append_bytes(b"pow/group-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/group-envelope/query", &query_digest);
                verify_group_extrema(
                    &input.relation_commit(None)?,
                    *key_column,
                    *value_column,
                    *datatype,
                    *kind,
                    &rows,
                    layer,
                    proof,
                    &mut transcript,
                )?;
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL recursive proof/plan variant mismatch",
                ))
            }
        }
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
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalObject>>> {
        let descriptors = compiled_result_descriptors(compiled)?;
        if descriptors.iter().any(|descriptor| descriptor.nullable) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "nullable Optional results require verify_and_resolve_values",
            ));
        }
        self.verify(compiled, layer, expected_root)?;
        if compiled.relation.schema.is_empty() {
            Ok(Vec::new())
        } else {
            let namespaces = descriptors
                .into_iter()
                .map(|descriptor| descriptor.namespace)
                .collect::<Vec<_>>();
            resolve_typed_result_objects(layer, expected_root, &self.result_columns, &namespaces)
        }
    }

    pub fn verify_and_resolve_values(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalResultValue>>> {
        self.verify(compiled, layer, expected_root)?;
        if compiled.relation.schema.is_empty() {
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
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        objects: &[Vec<CanonicalObject>],
    ) -> std::io::Result<()> {
        self.verify(compiled, layer, expected_root)?;
        if compiled.relation.schema.is_empty() {
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
                &compiled_result_descriptors(compiled)?
                    .into_iter()
                    .map(|descriptor| descriptor.namespace)
                    .collect::<Vec<_>>(),
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
        layer: &impl VerifierLayer,
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
        if compiled_result_descriptors(compiled)?
            .iter()
            .any(|descriptor| descriptor.nullable)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "nullable Optional rows require verify_executed_values",
            ));
        }
        if compiled.relation.schema.is_empty() {
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
            .unwrap_or_else(|| (0..compiled.relation.schema.len()).collect());
        let expected_variables: Vec<&str> = projection
            .iter()
            .map(|index| compiled.relation.schema[*index].as_str())
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
        let namespaces: Vec<_> = compiled_result_descriptors(compiled)?
            .into_iter()
            .map(|descriptor| descriptor.namespace)
            .collect();
        let projected_namespaces: Vec<_> =
            projection.iter().map(|index| namespaces[*index]).collect();
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

    pub fn verify_executed_values(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        variables: &[String],
        rows: &[Vec<ExecutedResultValue>],
    ) -> std::io::Result<()> {
        if compiled.count_variable.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Count bindings use the scalar execution boundary, not Store result values",
            ));
        }
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..compiled.relation.schema.len()).collect());
        let expected_variables: Vec<&str> = projection
            .iter()
            .map(|index| compiled.relation.schema[*index].as_str())
            .collect();
        if variables.iter().map(String::as_str).collect::<Vec<_>>() != expected_variables
            || rows.iter().any(|row| row.len() != projection.len())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "executed nullable result shape does not match compiled projection",
            ));
        }
        let proven = self.verify_and_resolve_values(compiled, layer, expected_root)?;
        let mut expected_rows: Vec<Vec<CanonicalResultValue>> = (0..self.result_len)
            .map(|row| {
                projection
                    .iter()
                    .map(|column| proven[*column][row].clone())
                    .collect()
            })
            .collect();
        let descriptors = compiled_result_descriptors(compiled)?;
        let projected_descriptors: Vec<_> =
            projection.iter().map(|index| descriptors[*index]).collect();
        let mut delivered_columns = vec![Vec::with_capacity(rows.len()); projection.len()];
        for row in rows {
            for (column, value) in row.iter().enumerate() {
                delivered_columns[column].push(match value {
                    ExecutedResultValue::Id(id) if *id != 0 => ArkFr::from_u64(*id),
                    ExecutedResultValue::Null => ArkFr::zero(),
                    ExecutedResultValue::Id(_) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "executed Store ID zero must be represented as typed Null",
                        ))
                    }
                });
            }
        }
        let delivered = resolve_result_values(
            layer,
            expected_root,
            &delivered_columns,
            &projected_descriptors,
        )?;
        let mut delivered_rows: Vec<Vec<CanonicalResultValue>> = (0..rows.len())
            .map(|row| {
                (0..projection.len())
                    .map(|column| delivered[column][row].clone())
                    .collect()
            })
            .collect();
        expected_rows.sort();
        delivered_rows.sort();
        if expected_rows != delivered_rows {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ordinary WOQL nullable rows do not match the proven result multiset",
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
        let digest = compiled_query_digest(compiled)?;
        // `verify_relation` recursively checks the complete proof tree.  Keep that single
        // public trust boundary, then only materialize the already-checked tree below;
        // verifying every child again (and the finished envelope once more) made checked
        // encoding quadratic in the depth/number of recursive operators.
        self.verify_relation(&compiled.relation, layer, expected_root, digest)?;
        let envelope = self.recursive_envelope_after_verification(
            &compiled.relation,
            layer,
            expected_root,
            digest,
        )?;
        envelope.encode()
    }

    /// Materialize an envelope after the caller has verified the complete root relation.
    /// This must remain private: decoded/external envelopes still go through the full
    /// recursive `RecursiveRelationEnvelope::verify` path.
    fn recursive_envelope_after_verification(
        &self,
        compiled: &CompiledRelation,
        layer: &LayerCommitment,
        expected_root: ProofHash,
        query_digest: ProofHash,
    ) -> std::io::Result<RecursiveRelationEnvelope> {
        let proof = match (&self.statement, &compiled.statement) {
            (
                ProvedQueryStatement::Single {
                    proof,
                    predicate_ids,
                },
                CompiledRelationStatement::Bgp(plan),
            ) => {
                let result_objects = if compiled.schema.is_empty() {
                    Vec::new()
                } else {
                    resolve_typed_result_objects(
                        layer,
                        expected_root,
                        &self.result_columns,
                        &result_id_namespaces(plan)?,
                    )?
                };
                RecursiveRelationProof::Bgp(Box::new(LayerBgpEnvelope {
                    layer_root: expected_root,
                    plan_digest: plan_digest(plan)?,
                    query_digest,
                    proof: *proof.clone(),
                    result_commitments: self.result_commitments.clone(),
                    result_columns: self.result_columns.clone(),
                    result_log_domain: self.result_log_domain,
                    result_len: self.result_len,
                    predicate_ids: predicate_ids.clone(),
                    result_objects,
                }))
            }
            (
                ProvedQueryStatement::BagUnion { branches, proof },
                CompiledRelationStatement::BagUnion {
                    branches: branch_plans,
                    ..
                },
            ) => RecursiveRelationProof::BagUnion {
                branches: branches
                    .iter()
                    .zip(branch_plans)
                    .map(|(branch, plan)| {
                        branch.recursive_envelope_after_verification(
                            plan,
                            layer,
                            expected_root,
                            relation_query_digest(plan)?,
                        )
                    })
                    .collect::<std::io::Result<_>>()?,
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::Equality { input, proof },
                CompiledRelationStatement::Equality {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::Equality {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::GroupExtrema { input, proof },
                CompiledRelationStatement::GroupExtrema {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::GroupExtrema {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::LeftAnti { left, right, proof },
                CompiledRelationStatement::LeftAnti {
                    left: left_plan,
                    right: right_plan,
                    ..
                },
            ) => RecursiveRelationProof::LeftAnti {
                left: Box::new(left.recursive_envelope_after_verification(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?),
                right: Box::new(right.recursive_envelope_after_verification(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::LeftOuter { left, right, proof },
                CompiledRelationStatement::LeftOuter {
                    left: left_plan,
                    right: right_plan,
                    ..
                },
            ) => RecursiveRelationProof::LeftOuter {
                left: Box::new(left.recursive_envelope_after_verification(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?),
                right: Box::new(right.recursive_envelope_after_verification(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?),
                proof: proof.clone(),
            },
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL recursive envelope proof/plan mismatch",
                ))
            }
        };
        let result_descriptors = relation_result_descriptors(compiled)?;
        let ground_result = compiled.schema.is_empty();
        let result_values = if ground_result {
            Vec::new()
        } else {
            resolve_result_values(
                layer,
                expected_root,
                &self.result_columns,
                &result_descriptors,
            )?
        };
        Ok(RecursiveRelationEnvelope {
            layer_root: expected_root,
            query_digest,
            proof,
            result_commitments: self.result_commitments.clone(),
            result_columns: self.result_columns.clone(),
            result_log_domain: self.result_log_domain,
            result_len: self.result_len,
            ground_result,
            result_descriptors,
            result_values,
        })
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
    let result_objects = if nullable {
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
                    })
                    .collect()
            })
            .collect::<std::io::Result<_>>()?
    };
    let result_values = nullable.then(|| envelope.result_values.clone());
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

fn proved_from_recursive(envelope: RecursiveRelationEnvelope) -> ProvedBgp {
    let statement = match envelope.proof {
        RecursiveRelationProof::Bgp(leaf) => ProvedQueryStatement::Single {
            proof: Box::new(leaf.proof),
            predicate_ids: leaf.predicate_ids,
        },
        RecursiveRelationProof::BagUnion { branches, proof } => ProvedQueryStatement::BagUnion {
            branches: branches.into_iter().map(proved_from_recursive).collect(),
            proof,
        },
        RecursiveRelationProof::LeftAnti { left, right, proof } => ProvedQueryStatement::LeftAnti {
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
        RecursiveRelationProof::GroupExtrema { input, proof } => {
            ProvedQueryStatement::GroupExtrema {
                input: Box::new(proved_from_recursive(*input)),
                proof,
            }
        }
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


fn store_id_for_field(
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
    }
    .ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-extrema Store id absent from planned namespace",
        )
    })
}

fn group_output_rows(
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
        let digest = compiled_query_digest(self)?;
        let plan = relation_proof_plan(&self.relation, digest)?;
        let envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &plan)?;
        Ok(proved_from_recursive(envelope))
    }

    /// Explicitly load the commitment from a synchronous Store layer and prove. Calling
    /// this method is the lifecycle decision; no worker or background job is started.
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
        let commitment = layer.proof_commitment()?;
        Ok(self.prove_with_context(context, &commitment)?)
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

    #[error("Or branches must expose the same variable schema")]
    OrIncompatibleSchema,

    #[error("Or branches expose incompatible Store ID namespaces")]
    OrIncompatibleNamespaces,

    #[error("Select, Count, or Distinct inside an Or branch is unsupported")]
    OrUnsupportedBranchWrapper,

    #[error("Or branches resolve to conflicting collections")]
    OrRoutingConflict,

    #[error("correlated Not needs a non-empty positive outer BGP")]
    NotMissingOuter,

    #[error("Not supports at most two shared variables, found {count}")]
    NotSharedVariableCount { count: usize },

    #[error("Not subplans may not contain result wrappers or correlate through nullable columns")]
    NotUnsupportedSubplan,

    #[error("Not subplans resolve to conflicting collections")]
    NotRoutingConflict,

    #[error("Optional needs a non-empty positive outer BGP")]
    OptionalMissingOuter,

    #[error("Optional supports at most two shared variables, found {count}")]
    OptionalSharedVariableCount { count: usize },

    #[error(
        "Optional subplans may not contain result wrappers or correlate through nullable columns"
    )]
    OptionalUnsupportedSubplan,

    #[error("recursive proof relation exceeds depth or node limits")]
    RecursiveStructureLimit,

    #[error("Equals needs an already-authenticated input relation")]
    EqualsMissingInput,

    #[error("Equals operand is a list or dictionary")]
    EqualsUnsupportedOperand,

    #[error("Equals variable `?{var}` is not already bound by the preceding relation")]
    EqualsUnboundVariable { var: String },

    #[error("Equals variable `?{var}` is nullable")]
    EqualsNullableVariable { var: String },

    #[error("Equals variable `?{var}` has no exact node/predicate/datatype domain proof")]
    EqualsUnknownDomain { var: String },

    #[error("Equals operands have incompatible exact value domains")]
    EqualsDomainMismatch,

    #[error("cross-datatype numeric Equals uses coercive executor semantics")]
    EqualsCoerciveNumeric,

    #[error("GroupBy proof coverage requires exactly one scalar key and an immediately consuming RangeMin/RangeMax over the hidden grouped list")]
    GroupExtremaUnsupportedShape,

    #[error("GroupBy extrema template must be a nonnullable variable with an exact fixed-width order-preserving datatype")]
    GroupExtremaUnsupportedValue,

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
        Query::If(_) => "When",
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

#[derive(Clone)]
struct TypedTriple {
    triple: Triple,
}

fn plan_group_extrema_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() == 2 => &and.and,
        _ => return Ok(None),
    };
    let (group, range, kind) = match (&conjuncts[0], &conjuncts[1]) {
        (Query::GroupBy(group), Query::RangeMin(range)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Min)
        }
        (Query::GroupBy(group), Query::RangeMax(range)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Max)
        }
        (Query::RangeMin(range), Query::GroupBy(group)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Min)
        }
        (Query::RangeMax(range), Query::GroupBy(group)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Max)
        }
        _ => return Ok(None),
    };
    if group.group_by.len() != 1 {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let template = match &group.template {
        Value::Variable(v) => v.clone(),
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedValue),
    };
    let hidden = match &group.grouped_value {
        Value::Variable(v) => v,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    let list = match range.0 {
        DataValue::Variable(v) => v,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    let result = match range.1 {
        DataValue::Variable(v) => v.clone(),
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    if hidden != list
        || hidden == &result
        || group.group_by[0] == result
        || group.group_by[0] == *hidden
    {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let child = plan_bgp(&group.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let key_column = child
        .relation
        .schema
        .iter()
        .position(|v| v == &group.group_by[0])
        .ok_or(BgpPlanError::GroupExtremaUnsupportedShape)?;
    let value_column = child
        .relation
        .schema
        .iter()
        .position(|v| v == &template)
        .ok_or(BgpPlanError::GroupExtremaUnsupportedValue)?;
    if key_column == value_column {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let datatype = match child.relation.domains.get(value_column).and_then(|d| *d) {
        Some(EqualityValueDomain::Datatype(dt))
            if matches!(
                dt,
                CanonicalDatatype::UInt8
                    | CanonicalDatatype::Int8
                    | CanonicalDatatype::UInt16
                    | CanonicalDatatype::Int16
                    | CanonicalDatatype::UInt32
                    | CanonicalDatatype::Int32
                    | CanonicalDatatype::Float32
                    | CanonicalDatatype::UInt64
                    | CanonicalDatatype::Int64
                    | CanonicalDatatype::Float64
            ) =>
        {
            dt
        }
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedValue),
    };
    let key_domain = child.relation.domains[key_column];
    let collection = child.collection;
    Ok(Some(CompiledBgp {
        relation: CompiledRelation {
            schema: vec![group.group_by[0].clone(), result],
            domains: vec![key_domain, Some(EqualityValueDomain::Datatype(datatype))],
            statement: CompiledRelationStatement::GroupExtrema {
                input: Box::new(child.relation),
                key_column,
                value_column,
                datatype,
                kind,
            },
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
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
        return plan_or_relation(
            disjunction,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }

    if let Some(mut aggregate) = plan_group_extrema_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if has_top_level_read_operator(body) {
        return plan_sequential_read_operators(
            body,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }
    if !matches!(
        body,
        Query::And(_) | Query::Triple(_)
    ) {
        return Err(BgpPlanError::UnsupportedConstruct {
            construct: query_variant_name(body),
        });
    }

    // (1b) Flatten nested Ands. Prolog's authoritative JSON compiler exposes graph
    // patterns only as Triple; Rust-only Link/Data forms reject closed because the
    // executor cannot enforce their declared node/literal distinction. Comparisons
    // become object-value range filters. True and Comment contribute no bindings and
    // are proof-transparent conjunction leaves.
    let mut triples: Vec<TypedTriple> = Vec::new();
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
    let domains = infer_schema_domains(&schema, &atoms, &comparisons);

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
        relation: CompiledRelation {
            schema: schema.clone(),
            domains,
            statement: CompiledRelationStatement::Bgp(plan),
        },
        projection,
        collection,
        count_variable,
        full_row_distinct: !distincts.is_empty(),
    })
}

fn infer_schema_domains(
    schema: &[String],
    atoms: &[Atom],
    comparisons: &[NormalizedRange],
) -> Vec<Option<EqualityValueDomain>> {
    schema
        .iter()
        .map(|variable| {
            if atoms
                .iter()
                .any(|atom| atom.pred_var.as_ref() == Some(variable))
            {
                Some(EqualityValueDomain::Predicate)
            } else if atoms
                .iter()
                .any(|atom| atom.subj_var.as_ref() == Some(variable))
            {
                Some(EqualityValueDomain::Node)
            } else {
                comparisons
                    .iter()
                    .find(|comparison| &comparison.var == variable)
                    .map(|comparison| EqualityValueDomain::Datatype(comparison.canonical_datatype))
            }
        })
        .collect()
}

fn plan_or_relation(
    disjunction: &crate::query::Or,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    if disjunction.or.len() < 2 {
        return Err(BgpPlanError::OrTooFewBranches {
            count: disjunction.or.len(),
        });
    }
    if !distincts.is_empty() {
        return Err(BgpPlanError::DistinctWithProjectionOrCount);
    }
    let mut compiled = Vec::with_capacity(disjunction.or.len());
    for query in &disjunction.or {
        let branch = plan_bgp(query)?;
        let internal_ground_projection = branch.relation.schema.is_empty()
            && branch.projection.as_deref() == Some(&[]);
        if (branch.projection.is_some() && !internal_ground_projection)
            || branch.count_variable.is_some()
            || branch.full_row_distinct
        {
            return Err(BgpPlanError::OrUnsupportedBranchWrapper);
        }
        compiled.push(branch);
    }
    let schema = compiled[0].relation.schema.clone();
    let mut effective_collection = collection;
    for branch in &compiled {
        merge_collection(
            &mut effective_collection,
            branch.collection.as_ref(),
            BgpPlanError::OrRoutingConflict,
        )?;
    }

    let mut branches = Vec::new();
    let mut mappings = Vec::new();
    let mut expected_descriptors = None;
    let mut merged_domains: Option<Vec<Option<EqualityValueDomain>>> = None;
    for branch in compiled {
        let mapping = schema_mapping(&schema, &branch.relation.schema)
            .ok_or(BgpPlanError::OrIncompatibleSchema)?;
        let descriptors = relation_result_descriptors(&branch.relation)
            .map_err(|_| BgpPlanError::OrIncompatibleNamespaces)?;
        let mapped: Vec<_> = mapping.iter().map(|index| descriptors[*index]).collect();
        if expected_descriptors
            .as_ref()
            .is_some_and(|expected| expected != &mapped)
        {
            return Err(BgpPlanError::OrIncompatibleNamespaces);
        }
        expected_descriptors = Some(mapped);
        let mapped_domains = mapping
            .iter()
            .map(|index| branch.relation.domains[*index])
            .collect::<Vec<_>>();
        if let Some(prior) = &mut merged_domains {
            for (prior, current) in prior.iter_mut().zip(&mapped_domains) {
                if *prior != *current {
                    *prior = None;
                }
            }
        } else {
            merged_domains = Some(mapped_domains);
        }
        match branch.relation.statement {
            CompiledRelationStatement::BagUnion {
                branches: nested,
                mappings: nested_mappings,
            } => {
                for (nested, nested_mapping) in nested.into_iter().zip(nested_mappings) {
                    mappings.push(mapping.iter().map(|index| nested_mapping[*index]).collect());
                    branches.push(nested);
                }
            }
            statement => {
                mappings.push(if schema.is_empty() {
                    Vec::new()
                } else {
                    mapping
                });
                branches.push(CompiledRelation {
                    schema: branch.relation.schema,
                    domains: branch.relation.domains,
                    statement,
                });
            }
        }
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &schema)?;
    let compiled = CompiledBgp {
        relation: CompiledRelation {
            schema,
            domains: merged_domains.unwrap_or_default(),
            statement: CompiledRelationStatement::BagUnion { branches, mappings },
        },
        projection,
        collection: effective_collection,
        count_variable,
        full_row_distinct: false,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

enum ReadOperator {
    Optional(Query),
    Not(Query),
    Equals(crate::compare::Equals),
}

fn has_top_level_read_operator(query: &Query) -> bool {
    match query {
        Query::And(and) => and.and.iter().any(has_top_level_read_operator),
        Query::WoqlOptional(_) | Query::Not(_) | Query::Equals(_) => true,
        _ => false,
    }
}

fn collect_read_operator_sequence(
    query: &Query,
    positive: &mut Vec<Query>,
    operators: &mut Vec<ReadOperator>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::And(and) => {
            for child in &and.and {
                collect_read_operator_sequence(child, positive, operators)?;
            }
        }
        Query::WoqlOptional(optional) => {
            operators.push(ReadOperator::Optional((*optional.query).clone()))
        }
        Query::Not(not) => operators.push(ReadOperator::Not((*not.query).clone())),
        Query::Equals(equals) => operators.push(ReadOperator::Equals(equals.clone())),
        other if operators.is_empty() => positive.push(other.clone()),
        other => {
            return Err(BgpPlanError::ConjunctNotTriple {
                construct: query_variant_name(other),
            })
        }
    }
    Ok(())
}

#[derive(Clone)]
enum EqualityOperand {
    Variable(String),
    Constant(CanonicalObject),
}

fn equality_operand(value: &crate::value::WoqlValue) -> Result<EqualityOperand, BgpPlanError> {
    match value {
        crate::value::WoqlValue::Variable(variable) => {
            Ok(EqualityOperand::Variable(variable.clone()))
        }
        crate::value::WoqlValue::Node(iri) => Ok(EqualityOperand::Constant(CanonicalObject::Node(
            iri.clone().into_bytes(),
        ))),
        crate::value::WoqlValue::Data(value) => {
            let term = value_constant_term(value)?;
            match term {
                AtomTerm::ValueConstant(datatype, lexical) => {
                    Ok(EqualityOperand::Constant(CanonicalObject::Value {
                        datatype,
                        lexical,
                    }))
                }
                _ => unreachable!("value_constant_term returned a non-value"),
            }
        }
        crate::value::WoqlValue::Dictionary(_) | crate::value::WoqlValue::List(_) => {
            Err(BgpPlanError::EqualsUnsupportedOperand)
        }
    }
}

fn numeric_equality_datatype(datatype: CanonicalDatatype) -> bool {
    use CanonicalDatatype::*;
    matches!(
        datatype,
        UInt8
            | Int8
            | UInt16
            | Int16
            | UInt32
            | Int32
            | Float32
            | UInt64
            | Int64
            | Float64
            | Decimal
            | BigInt
            | PositiveInteger
            | NonNegativeInteger
            | NonPositiveInteger
            | NegativeInteger
    )
}

fn bound_equality_column(
    relation: &CompiledRelation,
    variable: &str,
) -> Result<(usize, EqualityValueDomain), BgpPlanError> {
    let column = relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| BgpPlanError::EqualsUnboundVariable {
            var: variable.to_string(),
        })?;
    let descriptors =
        relation_result_descriptors(relation).map_err(|_| BgpPlanError::EqualsDomainMismatch)?;
    if descriptors[column].nullable {
        return Err(BgpPlanError::EqualsNullableVariable {
            var: variable.to_string(),
        });
    }
    let domain = relation.domains[column].ok_or_else(|| BgpPlanError::EqualsUnknownDomain {
        var: variable.to_string(),
    })?;
    Ok((column, domain))
}

fn constant_matches_domain(value: &CanonicalObject, domain: EqualityValueDomain) -> bool {
    match (value, domain) {
        (CanonicalObject::Node(_), EqualityValueDomain::Node) => true,
        (CanonicalObject::Value { datatype, .. }, EqualityValueDomain::Datatype(expected)) => {
            *datatype == expected
        }
        _ => false,
    }
}

fn compile_equality_constraint(
    relation: &CompiledRelation,
    equals: &crate::compare::Equals,
) -> Result<RelationEqualityPlan, BgpPlanError> {
    let left = equality_operand(&equals.left)?;
    let right = equality_operand(&equals.right)?;
    match (left, right) {
        (EqualityOperand::Variable(left), EqualityOperand::Variable(right)) => {
            let (left_column, left_domain) = bound_equality_column(relation, &left)?;
            let (right_column, right_domain) = bound_equality_column(relation, &right)?;
            if left_domain != right_domain {
                return Err(BgpPlanError::EqualsDomainMismatch);
            }
            Ok(RelationEqualityPlan::Columns {
                left: left_column,
                right: right_column,
                domain: left_domain,
            })
        }
        (EqualityOperand::Variable(variable), EqualityOperand::Constant(value))
        | (EqualityOperand::Constant(value), EqualityOperand::Variable(variable)) => {
            let constant_on_left = matches!(
                equality_operand(&equals.left)?,
                EqualityOperand::Constant(_)
            );
            let (column, domain) = bound_equality_column(relation, &variable)?;
            if !constant_matches_domain(&value, domain) {
                return Err(BgpPlanError::EqualsDomainMismatch);
            }
            Ok(RelationEqualityPlan::ColumnConstant {
                column,
                domain,
                value,
                constant_on_left,
            })
        }
        (EqualityOperand::Constant(left), EqualityOperand::Constant(right)) => {
            if let (
                CanonicalObject::Value { datatype: left, .. },
                CanonicalObject::Value {
                    datatype: right, ..
                },
            ) = (&left, &right)
            {
                if left != right
                    && (numeric_equality_datatype(*left) || numeric_equality_datatype(*right))
                {
                    return Err(BgpPlanError::EqualsCoerciveNumeric);
                }
            }
            Ok(RelationEqualityPlan::Constants { left, right })
        }
    }
}

fn plan_sequential_read_operators(
    query: &Query,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    let mut positive = Vec::new();
    let mut operators = Vec::new();
    collect_read_operator_sequence(query, &mut positive, &mut operators)?;
    if positive.is_empty() {
        return Err(match operators.first() {
            Some(ReadOperator::Optional(_)) => BgpPlanError::OptionalMissingOuter,
            Some(ReadOperator::Equals(_)) => BgpPlanError::EqualsMissingInput,
            _ => BgpPlanError::NotMissingOuter,
        });
    }
    let positive_query = if positive.len() == 1 {
        positive.pop().unwrap()
    } else {
        Query::And(crate::query::And { and: positive })
    };
    let left = plan_bgp(&positive_query)?;
    let mut relation = left.relation;
    let mut effective_collection = collection;
    let first_optional = matches!(operators.first(), Some(ReadOperator::Optional(_)));
    merge_collection(
        &mut effective_collection,
        left.collection.as_ref(),
        if first_optional {
            BgpPlanError::OptionalRoutingConflict
        } else {
            BgpPlanError::NotRoutingConflict
        },
    )?;

    for operator in operators {
        if let ReadOperator::Equals(equals) = operator {
            let constraint = compile_equality_constraint(&relation, &equals)?;
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::Equality {
                    input: Box::new(relation),
                    constraint,
                },
            };
            continue;
        }
        let (right_query, optional) = match operator {
            ReadOperator::Optional(query) => (query, true),
            ReadOperator::Not(query) => (query, false),
            ReadOperator::Equals(_) => unreachable!(),
        };
        let right = plan_bgp(&right_query)?;
        reject_subplan_wrappers(&right, optional)?;
        merge_collection(
            &mut effective_collection,
            right.collection.as_ref(),
            if optional {
                BgpPlanError::OptionalRoutingConflict
            } else {
                BgpPlanError::NotRoutingConflict
            },
        )?;
        let key_pairs = shared_key_pairs(&relation, &right.relation, optional)?;
        if optional {
            let mut schema = relation.schema.clone();
            let mut domains = relation.domains.clone();
            domains.extend(
                right
                    .relation
                    .schema
                    .iter()
                    .zip(&right.relation.domains)
                    .filter(|(name, _)| !relation.schema.contains(name))
                    .map(|(_, domain)| *domain),
            );
            schema.extend(
                right
                    .relation
                    .schema
                    .iter()
                    .filter(|name| !relation.schema.contains(name))
                    .cloned(),
            );
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::LeftOuter {
                    left: Box::new(relation),
                    right: Box::new(right.relation),
                    key_pairs,
                },
            };
        } else {
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::LeftAnti {
                    left: Box::new(relation),
                    right: Box::new(right.relation),
                    key_pairs,
                },
            };
        }
    }
    validate_distincts(
        distincts,
        selections,
        count_variable.as_deref(),
        &relation.schema,
    )?;
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection: effective_collection,
        count_variable,
        full_row_distinct: !distincts.is_empty(),
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

fn validate_compiled_structure(relation: &CompiledRelation) -> Result<(), BgpPlanError> {
    fn visit(
        relation: &CompiledRelation,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<(), BgpPlanError> {
        if depth > MAX_RELATION_PROOF_DEPTH || *nodes >= MAX_RELATION_PROOF_NODES {
            return Err(BgpPlanError::RecursiveStructureLimit);
        }
        *nodes += 1;
        match &relation.statement {
            CompiledRelationStatement::Bgp(_) => Ok(()),
            CompiledRelationStatement::BagUnion { branches, .. } => {
                for branch in branches {
                    visit(branch, depth + 1, nodes)?;
                }
                Ok(())
            }
            CompiledRelationStatement::LeftAnti { left, right, .. }
            | CompiledRelationStatement::LeftOuter { left, right, .. } => {
                visit(left, depth + 1, nodes)?;
                visit(right, depth + 1, nodes)
            }
            CompiledRelationStatement::Equality { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::GroupExtrema { input, .. } => visit(input, depth + 1, nodes),
        }
    }
    let mut nodes = 0;
    visit(relation, 1, &mut nodes)
}

fn reject_subplan_wrappers(compiled: &CompiledBgp, optional: bool) -> Result<(), BgpPlanError> {
    if compiled.projection.is_some()
        || compiled.count_variable.is_some()
        || compiled.full_row_distinct
    {
        Err(if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        })
    } else {
        Ok(())
    }
}

fn shared_key_pairs(
    left: &CompiledRelation,
    right: &CompiledRelation,
    optional: bool,
) -> Result<Vec<(usize, usize)>, BgpPlanError> {
    let shared: Vec<_> = left
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
    if shared.len() > 2 {
        return Err(if optional {
            BgpPlanError::OptionalSharedVariableCount {
                count: shared.len(),
            }
        } else {
            BgpPlanError::NotSharedVariableCount {
                count: shared.len(),
            }
        });
    }
    let left_descriptors = relation_result_descriptors(left).map_err(|_| {
        if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        }
    })?;
    let right_descriptors = relation_result_descriptors(right).map_err(|_| {
        if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        }
    })?;
    for (name, left_index, right_index) in &shared {
        if left_descriptors[*left_index].namespace != right_descriptors[*right_index].namespace {
            return Err(BgpPlanError::VariableNamespaceConflict {
                var: (*name).clone(),
            });
        }
        // Independent authenticated child relations model equality correlation. A
        // nullable outer binding is Prolog-unbound and would instead become bindable,
        // so this composition is deliberately rejected closed.
        if left_descriptors[*left_index].nullable || right_descriptors[*right_index].nullable {
            return Err(if optional {
                BgpPlanError::OptionalUnsupportedSubplan
            } else {
                BgpPlanError::NotUnsupportedSubplan
            });
        }
    }
    Ok(shared
        .into_iter()
        .map(|(_, left, right)| (left, right))
        .collect())
}

fn schema_mapping(expected: &[String], actual: &[String]) -> Option<Vec<usize>> {
    if expected.len() != actual.len() {
        return None;
    }
    expected
        .iter()
        .map(|name| actual.iter().position(|candidate| candidate == name))
        .collect()
}

fn merge_collection(
    current: &mut Option<String>,
    candidate: Option<&String>,
    error: BgpPlanError,
) -> Result<(), BgpPlanError> {
    match (&current, candidate) {
        (Some(expected), Some(actual)) if expected != actual => Err(error),
        (None, Some(actual)) => {
            *current = Some(actual.clone());
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_distincts(
    distincts: &[&[String]],
    selections: &[(&[String], bool)],
    count_variable: Option<&str>,
    schema: &[String],
) -> Result<(), BgpPlanError> {
    if !distincts.is_empty() && (!selections.is_empty() || count_variable.is_some()) {
        return Err(BgpPlanError::DistinctWithProjectionOrCount);
    }
    for variables in distincts {
        if variables.len() != schema.len()
            || !schema.iter().all(|name| {
                variables
                    .iter()
                    .filter(|candidate| *candidate == name)
                    .count()
                    == 1
            })
        {
            return Err(BgpPlanError::DistinctNotFullSchema);
        }
    }
    Ok(())
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
    triples: &mut Vec<TypedTriple>,
    comparisons: &mut Vec<NormalizedRange>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::Triple(triple) => triples.push(TypedTriple {
            triple: triple.clone(),
        }),
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
fn atom_from_triple(typed: &TypedTriple) -> Result<Atom, BgpPlanError> {
    let t = &typed.triple;
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
    canonical_datatype: CanonicalDatatype,
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
    let (datatype, c, canonical_datatype) = encode_range_constant(xsd)?;
    Ok(NormalizedRange {
        var,
        datatype,
        canonical_datatype,
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
    let (start_datatype, start_c, start_canonical_datatype) = encode_range_constant(start)?;
    let (end_datatype, end_c, end_canonical_datatype) = encode_range_constant(end)?;
    if start_datatype != end_datatype || start_canonical_datatype != end_canonical_datatype {
        return Err(BgpPlanError::RangeDatatypeMismatch { var });
    }
    Ok(NormalizedRange {
        var,
        datatype: start_datatype,
        canonical_datatype: start_canonical_datatype,
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
fn encode_range_constant(
    xsd: &XSDAnySimpleType,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
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
fn encode_via_store(
    entry: TypedDictEntry,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
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
    Ok((datatype_tag, c, datatype))
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

#[cfg(test)]
mod group_extrema_planner_tests {
    use super::*;
    use crate::compare::Gte;
    use crate::misc::RangeMin;
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

    #[test]
    fn exact_scalar_group_min_is_compiled_without_exposing_hidden_list() {
        let compiled = plan_bgp(&exact_min()).unwrap();
        assert_eq!(compiled.relation.schema, ["Person", "Youngest"]);
        match compiled.relation.statement {
            CompiledRelationStatement::GroupExtrema {
                key_column,
                value_column,
                datatype,
                kind,
                ..
            } => {
                assert_ne!(key_column, value_column);
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
