//! Proof-of-WOQL: the query-proof compiler + authenticated executor.
//!
//! Extracted from `terminusdb-woql2` (which stays the lightweight WOQL2 AST). This
//! crate lowers a WOQL2 `Query` AST into the Dory/sumcheck query arguments
//! implemented by the Parture `terminus-store` fork, and exposes the app-controlled
//! prove/verify entry points. Ordinary AST/client users depend on `terminusdb-woql2`
//! and never pull this cryptographic stack.
pub mod proof;
pub mod reconciled;

/// Prover-backend selector (`StoreProverContext` + Blitzar types), surfaced at the
/// crate root as `terminusdb_woql_proof::prover` for the high-level prove API.
pub use proof::prover;
