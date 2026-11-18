//! Reactor — a reactive, graph-tracked cache engine for TerminusDB 12.
//!
//! Reactor sits in front of one TerminusDB node as a sidecar, passes the whole
//! API through byte-for-byte, and caches what it can *prove* it may cache. It
//! learns what to invalidate by watching commits: every commit is turned into a
//! set of dependency [`term::Term`]s, and every cached fragment remembers the
//! terms it was derived from. Design rationale: `docs/reactive-cache-design.md`.
//!
//! # The invariants everything else follows from
//!
//! 1. **Byte-faithful.** Cached and proxied bodies are [`bytes::Bytes`] and are
//!    never round-tripped through `serde_json::Value`. v12 returns numbers at
//!    full rational precision and this workspace enables
//!    `serde_json/arbitrary_precision`; re-serializing a body would reorder keys
//!    and could corrupt values. We parse to *inspect*, never to re-emit.
//! 2. **Fills are pinned.** Every upstream fill is issued against
//!    `…/local/commit/<id>` at the position the reader latched, so a fragment is
//!    exactly "the value at that commit" and never a read torn across commits.
//! 3. **Commit ids carry no order.** They are content hashes. All ordering is by
//!    the Reactor-local [`ids::Position`].
//! 4. **The watermark only advances past a commit we have actually read**, so
//!    capture lag costs staleness, never correctness.
//! 5. **Soundness over precision.** An analyzer that cannot prove a tight term
//!    emits a coarser one, or bypasses. Over-invalidation is a performance bug;
//!    under-invalidation is a correctness bug.
//! 6. **Unknown routes are forwarded, never cached.**
//! 7. **Never memoize an error.**
//!
//! # Where to start reading
//!
//! * [`term`] — the invalidation algebra. `Term::subsumes` is the heart of the
//!   whole design; if it is wrong, data goes silently stale.
//! * [`change`] — `fire()`: commit → fired terms.
//! * [`engine`] — the dependency graph, the single writer per namespace, and the
//!   insert protocol that makes the fill/commit race exactly solvable.
//! * [`store`] — immutable, content-addressed bodies; also the shared-tier story
//!   for multi-node deployments.
//! * [`analyze`] — request → plan. Where soundness is decided: a dependency
//!   this layer fails to emit is data that goes stale and never recovers.
//!
//! # Implementation status
//!
//! Phases A through D are implemented: identity, terms, the schema index,
//! firing, the engine state machine, the fragment store, request analysis,
//! commit capture and the HTTP front end. What remains (phase E) is integration
//! testing against a live `terminusdb_bin::TerminusDBServer`, plus the
//! russian-doll decomposition that the `children` field is reserved for.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod analyze;
pub mod capture;
pub mod change;
pub mod config;
pub mod engine;
pub mod frontend;
pub mod ids;
pub mod metrics;
pub mod schema_index;
pub mod store;
pub mod term;

#[cfg(test)]
mod testutil;

pub use analyze::{
    augment_terms_from_response, AnalyzeConfig, Augment, InjectedField, Plan, ReqView, Rewrite,
    Target,
};
pub use capture::{Applier, ChangeResolver, CommitEvent, CommitSource, ReadinessGate};
pub use change::{fire, ChangeSet, DocChange};
pub use config::{Cli, Config, Driver, MinVersionPolicy};
pub use engine::{
    BodyRef, Engine, EngineConfig, FillResult, FragmentKey, FragmentSlot, FragmentVersion, Lookup,
    NamespaceWriter, Policy, ResponseMeta, RootKey, TermIndex, WriterMsg,
};
pub use frontend::{router, AppState};
pub use ids::{
    ClassName, ContentHash, DocId, Epoch, Namespace, Position, PropName, Seq, ShapeId, Watermark,
};
pub use metrics::Metrics;
pub use schema_index::{PropKind, SchemaIndex};
pub use store::{FragmentStore, FsCasStore, MemoryStore, TieredStore};
pub use term::{Term, TermBucket, TermSet, ValueKey};

/// Re-exported for convenience: Reactor uses the client's opaque commit id type
/// rather than defining its own, so a `CommitId` crossing module boundaries is
/// the same type the capture layer got from TerminusDB.
pub use terminusdb_client::CommitId;
