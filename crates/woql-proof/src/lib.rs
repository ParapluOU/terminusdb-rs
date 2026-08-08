//! Phase 7.8 — compile a real woql2 conjunctive BGP AST into a safe, variable-blind
//! [`terminus_store::proof::query::LayerBgpPlan`].
//!
//! The audited Proof-of-WOQL prover/verifier
//! ([`terminus_store::proof::query::prove_layer_bgp`] /
//! [`verify_layer_bgp`](terminus_store::proof::query::verify_layer_bgp)) faithfully
//! proves *whatever* single-key join tree a [`LayerBgpPlan`] encodes. That makes the
//! PLAN the sole safety line: a plan that does not faithfully encode the intended BGP
//! (a composite/cyclic key, a repeated variable inside one atom, or a disconnected
//! step) would yield a wrong-but-verifying SUPERSET of the true answer.
//!
//! [`plan_bgp`] is that safety line. It accepts only the fragment the single-key
//! left-deep chain can encode faithfully and rejects everything else *closed*.

#![forbid(unsafe_code)]

mod compile;

pub use compile::{plan_bgp, BgpPlanError, CompiledBgp};
