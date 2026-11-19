//! Identity and ordering primitives.
//!
//! Two ideas in this module carry most of the design's weight:
//!
//! * **A commit id is not a revision number.** TerminusDB commit ids are content
//!   hashes of the layer they name. They are stable, globally meaningful and
//!   completely unordered — `"abc123" < "def456"` is a string comparison, not a
//!   history comparison. Every ordering decision in Reactor is therefore made
//!   against a Reactor-local [`Position`], and [`terminusdb_client::CommitId`]
//!   is treated as an opaque label. (It helps that `CommitId` does not even
//!   implement `PartialOrd`, so the mistake does not compile.)
//! * **Identifiers are cloned constantly** — into terms, into index buckets,
//!   into fragment keys — so [`DocId`], [`ClassName`] and [`PropName`] are
//!   `Arc<str>` newtypes rather than `String`s. Cloning one is a refcount bump.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use terminusdb_client::{BranchSpec, CommitId};

/// A cache universe: one database branch on one node.
///
/// Everything in the engine is partitioned by namespace, because everything that
/// is *mutable* in the engine (watermark, epoch, term index, fired log) is a
/// statement about one branch on one node. Two namespaces never interact, which
/// is what lets each one have its own single writer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Namespace {
    /// The organization segment of the upstream path (`admin` by default).
    pub org: String,
    /// The database name.
    pub db: String,
    /// The branch name (`main` by default).
    pub branch: String,
}

impl Namespace {
    /// Build a namespace from its three segments.
    pub fn new(org: impl Into<String>, db: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            org: org.into(),
            db: db.into(),
            branch: branch.into(),
        }
    }

    /// `admin/mydb/local/branch/main` — the upstream path segment.
    ///
    /// This is the canonical rendering: it is what goes into an upstream URL and
    /// what identifies the namespace in logs and metrics.
    pub fn path(&self) -> String {
        format!("{}/{}/local/branch/{}", self.org, self.db, self.branch)
    }

    /// `admin/mydb/local/commit/<id>` — the *pinned* upstream path segment.
    ///
    /// Invariant 2 of the spec: every fill is issued against this form, never
    /// against [`Namespace::path`], so a fragment is exactly "the value at that
    /// commit" rather than "the value at whatever the head happened to be while
    /// the request was in flight".
    pub fn commit_path(&self, commit: &CommitId) -> String {
        format!("{}/{}/local/commit/{}", self.org, self.db, commit)
    }

    /// A [`BranchSpec`] naming this namespace's branch, for `terminusdb-client`
    /// calls on the capture path.
    ///
    /// Note that `BranchSpec` carries no organization: the client holds the org
    /// itself, so a client used with this spec must be configured with the same
    /// `org` as this namespace.
    pub fn to_branch_spec(&self) -> BranchSpec {
        BranchSpec::with_branch(self.db.clone(), self.branch.clone())
    }

    /// A [`BranchSpec`] pinned at `commit`, for fills and for before/after
    /// diffing. See [`Namespace::commit_path`] for why pinning is mandatory.
    pub fn to_commit_spec(&self, commit: &CommitId) -> BranchSpec {
        let mut spec = self.to_branch_spec();
        spec.ref_commit = Some(commit.clone());
        spec
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path())
    }
}

/// Reactor-local monotonic position inside an epoch. Commit ids are content
/// hashes with no order; THIS is the revision counter.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct Seq(pub u64);

/// Bumped on non-fast-forward head movement (rebase/reset/squash/apply, or a
/// pull that lands a head which is not a descendant of our watermark).
///
/// The epoch exists so that a fill which was pinned at a position in a history
/// we have since abandoned can be rejected by a single integer comparison. On a
/// replica, non-fast-forward movement is routine rather than exotic (design doc
/// §5.7), so this has to be cheap and unmissable.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct Epoch(pub u64);

/// Totally ordered position in this Reactor's view of a branch's history.
///
/// Lexicographic on `(epoch, seq)` — the field order below *is* the comparison
/// order, which is why `epoch` is declared first.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct Position {
    /// Which history generation this position belongs to.
    pub epoch: Epoch,
    /// Monotonic counter within the epoch.
    pub seq: Seq,
}

impl Position {
    /// The position before anything has been applied. A fragment can never be
    /// valid *before* this.
    pub const ZERO: Position = Position {
        epoch: Epoch(0),
        seq: Seq(0),
    };

    /// The next position in the same epoch: what a fast-forward commit gets.
    pub fn next(self) -> Position {
        Position {
            epoch: self.epoch,
            seq: Seq(self.seq.0.saturating_add(1)),
        }
    }

    /// The first position of the next epoch: what a non-fast-forward head
    /// movement gets. Resetting `seq` is deliberate — positions are only ever
    /// compared, never subtracted, and the epoch dominates the comparison.
    pub fn next_epoch(self) -> Position {
        Position {
            epoch: Epoch(self.epoch.0.saturating_add(1)),
            seq: Seq(0),
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.epoch.0, self.seq.0)
    }
}

/// What a reader latches at request start, and what we report to clients.
///
/// The pair is published atomically and only ever *after* a pinned read at
/// `commit` has succeeded (invariant 4). That is what makes capture lag cost
/// staleness instead of correctness: we never claim to be at a commit we have
/// not actually read from the local node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watermark {
    /// Reactor-local position of `commit`.
    pub pos: Position,
    /// The upstream commit id this position corresponds to. Opaque; never
    /// compared for order.
    pub commit: CommitId,
}

/// The key of an application-computed value.
///
/// Reactor did not run the transformation that produced the value and cannot see
/// inside it, so it cannot derive a key from it the way [`ShapeId`] is derived
/// from a request: a closure has no normal form to hash. The key is therefore
/// the caller's, and it is load-bearing — anything that changes what the
/// computation *means* (its parameters, the user it is scoped to, the version of
/// the code) has to be in here, or two different values share one slot.
///
/// `terminusdb-cache` composes it as `name/params-hash/epoch`; Reactor treats it
/// as opaque text and only requires that it be non-empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DerivedKey(Arc<str>);

impl DerivedKey {
    /// Wrap caller-supplied key text. `None` for an empty or whitespace-only
    /// key, which would otherwise be a cache slot nobody could name twice.
    pub fn new(text: &str) -> Option<Self> {
        let text = text.trim();
        (!text.is_empty()).then(|| DerivedKey(Arc::from(text)))
    }

    /// The key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DerivedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A document id in short form, e.g. `Person/alice`.
///
/// `Arc<str>` because these are cloned constantly into terms and keys. Always
/// constructed through [`DocId::parse`]: hand-formatting an IRI is a workspace
/// rule violation (see `CLAUDE.md`) and the main way id normalization drifts.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocId(Arc<str>);

impl DocId {
    /// Accepts a short id (`Person/alice`) or a full IRI
    /// (`terminusdb:///data/Person/alice`); normalizes to short form.
    ///
    /// Normalization matters because the same document arrives spelled three
    /// different ways depending on the route: `compress_ids` responses give the
    /// short form, unfolded documents and WOQL bindings give the full data IRI,
    /// and schema-ish IRIs are fragment-delimited. A term built from one
    /// spelling must match a term built from another, or invalidation silently
    /// misses.
    ///
    /// The rules, in order:
    /// 1. everything after the last `/data/` (the instance-graph base), else
    /// 2. everything after the last `#` (fragment-delimited IRIs), else
    /// 3. the input as given,
    ///
    /// then leading slashes are trimmed. A key containing the literal `/data/`
    /// would be mis-split by rule 1; that is accepted as a non-case.
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        let tail = if let Some(idx) = s.rfind("/data/") {
            &s[idx + "/data/".len()..]
        } else if let Some(idx) = s.rfind('#') {
            &s[idx + 1..]
        } else {
            s
        };
        DocId(Arc::from(tail.trim_start_matches('/')))
    }

    /// The normalized short form.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Everything before the first `/`; `None` if there is no class prefix.
    ///
    /// The first separator, not the last: subdocument ids nest
    /// (`Person/alice/address/Address/9f3`) and the *outermost* class is the one
    /// that owns the document for invalidation purposes.
    pub fn class(&self) -> Option<ClassName> {
        let (head, rest) = self.0.split_once('/')?;
        if head.is_empty() || rest.is_empty() {
            return None;
        }
        Some(ClassName::new(head))
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for DocId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DocId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(DocId::parse(&s))
    }
}

/// A schema class name, unprefixed (`Person`, not `@schema:Person`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassName(Arc<str>);

/// A property name as it appears on a document (`name`, `author`, `@type`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PropName(Arc<str>);

macro_rules! str_newtype {
    ($t:ident, $what:literal) => {
        impl $t {
            #[doc = concat!("Wrap an already-normalized ", $what, ".")]
            pub fn new(s: impl Into<Arc<str>>) -> Self {
                $t(s.into())
            }

            #[doc = concat!("The ", $what, " as a string slice.")]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $t {
            fn from(s: &str) -> Self {
                $t(Arc::from(s))
            }
        }

        impl Serialize for $t {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                Ok($t(Arc::from(s.as_str())))
            }
        }
    };
}

str_newtype!(ClassName, "class name");
str_newtype!(PropName, "property name");

/// blake3-256. Content addresses and cache keys.
///
/// One hash function does three jobs: it addresses immutable bodies in the
/// content-addressed store (so the same document body is stored once across
/// commits and branches), it is the `ETag` we hand clients (TerminusDB has no
/// ETags of its own), and it is how backdating recognizes "this recomputation
/// produced the same bytes, so nothing downstream needs to change".
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash([u8; 32]);

// Serialized as a hex string rather than as the derived 32-element byte array.
// These appear in `/_reactor/explain` output and in `ETag`s, where a JSON array
// of 32 integers is unreadable and four times the size; hex round-trips exactly
// and is the same spelling used for store paths.
impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ContentHash::from_hex(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("not a 64-char hex digest: {s}")))
    }
}

impl ContentHash {
    /// Hash a body.
    pub fn of(bytes: &[u8]) -> Self {
        ContentHash(*blake3::hash(bytes).as_bytes())
    }

    /// Order-independent combination, for composing child hashes into a parent.
    ///
    /// Sorts in place and then hashes the concatenation, so a composed response
    /// gets the same hash regardless of the order its children were collected
    /// in. That is why the argument is `&mut`.
    pub fn combine(parts: &mut [ContentHash]) -> Self {
        parts.sort_unstable();
        let mut hasher = blake3::Hasher::new();
        for part in parts.iter() {
            hasher.update(&part.0);
        }
        ContentHash(*hasher.finalize().as_bytes())
    }

    /// The full 64-character lowercase hex rendering, used for store paths and
    /// `ETag`s. `Display` deliberately prints something shorter, so this is the
    /// only way to get a round-trippable string.
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
            out.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
        }
        out
    }

    /// Parse a 64-character hex string; `None` on any malformed input. Used for
    /// `/_reactor/fragment/<hash>` and `If-None-Match`, both of which are
    /// attacker-reachable, hence no panic path.
    pub fn from_hex(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, out_byte) in out.iter_mut().enumerate() {
            let hi = (bytes[2 * i] as char).to_digit(16)?;
            let lo = (bytes[2 * i + 1] as char).to_digit(16)?;
            *out_byte = ((hi << 4) | lo) as u8;
        }
        Some(ContentHash(out))
    }

    /// The raw digest, for callers that need to feed it to another hasher.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Short form: these appear in every log line and trace field, and 64
        // hex characters per fragment makes logs unreadable.
        write!(f, "{}", &self.to_hex()[..12])
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.to_hex()[..12])
    }
}

/// "Which projection of a thing" a fragment holds: hash of a normalized
/// request/selection shape with ids and variables factored out.
///
/// Factoring the ids out is what makes the cache entity-precise: `Person/alice`
/// and `Person/bob` fetched through the same GraphQL selection share one
/// `ShapeId` and differ only in their [`crate::engine::RootKey`], so a schema
/// change can invalidate a shape across all documents, and a document change can
/// invalidate all shapes of one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShapeId(pub ContentHash);

impl ShapeId {
    /// Hash normalized shape text into a shape id.
    pub fn of(normalized: &[u8]) -> Self {
        ShapeId(ContentHash::of(normalized))
    }
}

impl fmt::Display for ShapeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_paths() {
        let ns = Namespace::new("admin", "mydb", "main");
        assert_eq!(ns.path(), "admin/mydb/local/branch/main");
        assert_eq!(
            ns.commit_path(&CommitId::new("abc123")),
            "admin/mydb/local/commit/abc123"
        );
        assert_eq!(ns.to_string(), "admin/mydb/local/branch/main");
    }

    #[test]
    fn namespace_specs_carry_branch_and_commit() {
        let ns = Namespace::new("admin", "mydb", "feature");
        let branch = ns.to_branch_spec();
        assert_eq!(branch.db, "mydb");
        assert_eq!(branch.branch.as_deref(), Some("feature"));
        assert!(branch.ref_commit.is_none());

        let pinned = ns.to_commit_spec(&CommitId::new("deadbeef"));
        assert!(pinned.is_commit_ref());
        assert_eq!(pinned.commit_id().map(|c| c.as_str()), Some("deadbeef"));
    }

    #[test]
    fn position_orders_by_epoch_then_seq() {
        let a = Position {
            epoch: Epoch(0),
            seq: Seq(9),
        };
        let b = Position {
            epoch: Epoch(1),
            seq: Seq(0),
        };
        assert!(a < b, "a later epoch always wins over a higher seq");
        assert_eq!(Position::ZERO.next().seq, Seq(1));
        assert_eq!(Position::ZERO.next().epoch, Epoch(0));
        assert_eq!(a.next_epoch(), b);
    }

    #[test]
    fn doc_id_parses_short_id() {
        let id = DocId::parse("Person/alice");
        assert_eq!(id.as_str(), "Person/alice");
        assert_eq!(id.class(), Some(ClassName::from("Person")));
    }

    #[test]
    fn doc_id_parses_full_iri() {
        let id = DocId::parse("terminusdb:///data/Person/alice");
        assert_eq!(id.as_str(), "Person/alice");
        assert_eq!(id.class(), Some(ClassName::from("Person")));
        assert_eq!(id, DocId::parse("Person/alice"));
    }

    #[test]
    fn doc_id_parses_fragment_iri() {
        let id = DocId::parse("terminusdb:///schema#Person");
        assert_eq!(id.as_str(), "Person");
        assert_eq!(id.class(), None, "no `/`, so no class prefix");
    }

    #[test]
    fn doc_id_without_class_prefix_has_no_class() {
        assert_eq!(DocId::parse("alice").class(), None);
        assert_eq!(DocId::parse("Person/").class(), None, "empty key");
        assert_eq!(DocId::parse("/alice").as_str(), "alice");
    }

    #[test]
    fn doc_id_with_extra_slashes_keeps_outermost_class() {
        let id = DocId::parse("Person/alice/address/Address/9f3");
        assert_eq!(id.class(), Some(ClassName::from("Person")));
        assert_eq!(id.as_str(), "Person/alice/address/Address/9f3");

        let iri = DocId::parse("terminusdb:///data/Person/alice/address/Address/9f3");
        assert_eq!(iri, id, "the IRI and short spellings must unify");
    }

    #[test]
    fn doc_id_serde_round_trips_through_short_form() {
        let json = serde_json::to_string(&DocId::parse("terminusdb:///data/Person/alice")).unwrap();
        assert_eq!(json, "\"Person/alice\"");
        let back: DocId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, DocId::parse("Person/alice"));
    }

    #[test]
    fn class_and_prop_serde_as_strings() {
        assert_eq!(
            serde_json::to_string(&ClassName::from("Person")).unwrap(),
            "\"Person\""
        );
        let p: PropName = serde_json::from_str("\"name\"").unwrap();
        assert_eq!(p.as_str(), "name");
        assert_eq!(p.to_string(), "name");
    }

    #[test]
    fn content_hash_hex_round_trip() {
        let h = ContentHash::of(b"hello");
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(ContentHash::from_hex(&hex), Some(h));
        assert_eq!(ContentHash::from_hex("short"), None);
        assert_eq!(ContentHash::from_hex(&"z".repeat(64)), None);
        assert_eq!(format!("{h}").len(), 12, "Display is abbreviated");
    }

    #[test]
    fn content_hash_combine_is_order_independent() {
        let a = ContentHash::of(b"a");
        let b = ContentHash::of(b"b");
        let c = ContentHash::of(b"c");
        let one = ContentHash::combine(&mut [a, b, c]);
        let two = ContentHash::combine(&mut [c, a, b]);
        assert_eq!(one, two);
        assert_ne!(one, ContentHash::combine(&mut [a, b]));
    }

    #[test]
    fn content_hash_differs_by_content() {
        assert_ne!(ContentHash::of(b"1"), ContentHash::of(b"1.0"));
    }
}
