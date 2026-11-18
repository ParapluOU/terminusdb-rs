//! The fragment body store: immutable, content-addressed bytes.
//!
//! Bodies are separated from fragment *versions* because a body is immutable and
//! node-independent while a version is a statement about this node's history
//! (design doc §5.7). That split buys three things:
//!
//! * **Deduplication.** Most documents are unchanged between commits, so the
//!   same bytes are referenced by many versions across many commits and
//!   branches, and stored once.
//! * **A shared tier with no coherence protocol.** Point two Reactors at the
//!   same [`FsCasStore`] directory and they share fragments safely, because
//!   nothing is ever updated in place: a hash either names the bytes it always
//!   named, or is absent. There is no invalidation message, no versioning, no
//!   tombstone and no way for one node to read another's *wrong* value — only a
//!   value it would not otherwise have had yet.
//! * **Survivable eviction.** Dropping a body is never a correctness event; the
//!   version that referenced it just has to be refilled.

mod fscas;
mod memory;

pub use fscas::FsCasStore;
pub use memory::MemoryStore;

use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;

use crate::ids::ContentHash;

/// A content-addressed body store.
///
/// Implementations must be safe to call concurrently from readers, the writer
/// task and fill tasks at once, hence `Send + Sync` and `&self` everywhere. A
/// `get` that returns `None` is always acceptable — it costs a refill, not
/// correctness — so implementations should prefer dropping a body to blocking.
pub trait FragmentStore: Send + Sync {
    /// Fetch a body by hash. `None` means "not here", never "wrong".
    fn get(&self, hash: &ContentHash) -> Option<Bytes>;

    /// Store a body under its hash.
    ///
    /// Callers must pass the hash *of these bytes*; implementations may verify.
    /// Writing an already-present hash is a no-op, because the content cannot
    /// have changed.
    fn put(&self, hash: &ContentHash, body: &Bytes) -> anyhow::Result<()>;

    /// True when the hash is present without fetching the body.
    fn contains(&self, hash: &ContentHash) -> bool;

    /// Approximate bytes held, for metrics and eviction.
    fn size(&self) -> u64;
}

/// A hot in-memory tier in front of an optional warm filesystem tier.
///
/// Reads that hit the warm tier are **promoted** into the hot tier, so the
/// working set migrates upwards without a separate warming pass. Writes go to
/// both: the hot tier so the fill that just completed is served from memory, the
/// warm tier so a restart (or a sibling node sharing the directory) starts warm.
///
/// A warm-tier write failure is logged and counted, not propagated: the body is
/// already safely in the hot tier, so the fill succeeded. A warm tier that is
/// full or read-only degrades this to a plain memory cache.
pub struct TieredStore {
    hot: MemoryStore,
    warm: Option<FsCasStore>,
    promotions: AtomicU64,
    warm_errors: AtomicU64,
}

impl TieredStore {
    /// Compose a hot tier with an optional warm tier.
    pub fn new(hot: MemoryStore, warm: Option<FsCasStore>) -> Self {
        Self {
            hot,
            warm,
            promotions: AtomicU64::new(0),
            warm_errors: AtomicU64::new(0),
        }
    }

    /// Memory-only: the default single-node deployment (design doc §5.7 mode 1).
    pub fn memory_only(budget_bytes: u64) -> Self {
        Self::new(MemoryStore::new(budget_bytes), None)
    }

    /// Bodies promoted from the warm tier into the hot tier on read.
    pub fn promotions(&self) -> u64 {
        self.promotions.load(Ordering::Relaxed)
    }

    /// Warm-tier writes that failed. Non-fatal, but worth alerting on.
    pub fn warm_errors(&self) -> u64 {
        self.warm_errors.load(Ordering::Relaxed)
    }

    /// The hot tier, for metrics.
    pub fn hot(&self) -> &MemoryStore {
        &self.hot
    }

    /// The warm tier, if configured.
    pub fn warm(&self) -> Option<&FsCasStore> {
        self.warm.as_ref()
    }
}

impl FragmentStore for TieredStore {
    fn get(&self, hash: &ContentHash) -> Option<Bytes> {
        if let Some(body) = self.hot.get(hash) {
            return Some(body);
        }
        let body = self.warm.as_ref()?.get(hash)?;
        // Promote. A failure here is irrelevant: we already have the bytes.
        let _ = self.hot.put(hash, &body);
        self.promotions.fetch_add(1, Ordering::Relaxed);
        Some(body)
    }

    fn put(&self, hash: &ContentHash, body: &Bytes) -> anyhow::Result<()> {
        self.hot.put(hash, body)?;
        if let Some(warm) = &self.warm {
            if let Err(err) = warm.put(hash, body) {
                self.warm_errors.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(%hash, error = %err, "warm fragment tier write failed");
            }
        }
        Ok(())
    }

    fn contains(&self, hash: &ContentHash) -> bool {
        self.hot.contains(hash) || self.warm.as_ref().is_some_and(|w| w.contains(hash))
    }

    fn size(&self) -> u64 {
        self.hot.size() + self.warm.as_ref().map_or(0, FsCasStore::size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(s: &str) -> Bytes {
        Bytes::from(s.to_owned())
    }

    #[test]
    fn tiered_reads_fall_through_and_promote() {
        let dir = tempfile::tempdir().unwrap();
        let warm = FsCasStore::open(dir.path()).unwrap();
        let b = body("warm only");
        let h = ContentHash::of(&b);
        warm.put(&h, &b).unwrap();

        let tiered = TieredStore::new(MemoryStore::new(1024), Some(warm));
        assert!(
            !tiered.hot().contains(&h),
            "precondition: the hot tier is cold"
        );
        assert_eq!(tiered.get(&h).as_deref(), Some(&b[..]));
        assert_eq!(tiered.promotions(), 1);
        assert!(
            tiered.hot().contains(&h),
            "a warm hit must be promoted into the hot tier"
        );
        // Second read is a hot hit, so no further promotion.
        assert!(tiered.get(&h).is_some());
        assert_eq!(tiered.promotions(), 1);
    }

    #[test]
    fn tiered_writes_reach_both_tiers() {
        let dir = tempfile::tempdir().unwrap();
        let tiered = TieredStore::new(
            MemoryStore::new(1024),
            Some(FsCasStore::open(dir.path()).unwrap()),
        );
        let b = body("both");
        let h = ContentHash::of(&b);
        tiered.put(&h, &b).unwrap();
        assert!(tiered.hot().contains(&h));
        assert!(tiered.warm().unwrap().contains(&h));
        assert!(tiered.size() >= b.len() as u64 * 2);
    }

    #[test]
    fn memory_only_tier_misses_cleanly() {
        let tiered = TieredStore::memory_only(64);
        let h = ContentHash::of(b"absent");
        assert!(tiered.get(&h).is_none());
        assert!(!tiered.contains(&h));
        assert_eq!(tiered.size(), 0);
        assert!(tiered.warm().is_none());
    }
}
