//! The hot in-memory tier.

use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use dashmap::DashMap;

use crate::ids::ContentHash;
use crate::store::FragmentStore;

struct Entry {
    body: Bytes,
    /// Logical timestamp of the last read, from the store's own counter. A
    /// logical clock rather than `Instant` because it is cheaper, is totally
    /// ordered, and eviction only ever needs *relative* recency.
    last_read: AtomicU64,
}

/// A bounded in-memory body store with least-recently-read eviction.
///
/// Approximate LRU on purpose. An exact LRU needs an intrusive list under a
/// single lock, which would serialize every read across the whole store; the
/// hot path here is `get`, which must be a sharded hash lookup plus one relaxed
/// store. Eviction, by contrast, happens rarely and off the critical path, so it
/// can afford to scan and sort.
///
/// Eviction is never a correctness event: a dropped body just makes the next
/// read of the version that references it a miss.
pub struct MemoryStore {
    entries: DashMap<ContentHash, Entry>,
    bytes: AtomicU64,
    budget: u64,
    clock: AtomicU64,
    evictions: AtomicU64,
}

impl MemoryStore {
    /// A store that will evict to stay at or below `budget_bytes`.
    ///
    /// The budget counts body bytes only; per-entry bookkeeping is ignored, so
    /// a store full of tiny bodies will exceed its nominal footprint. Sizing
    /// should assume a margin.
    pub fn new(budget_bytes: u64) -> Self {
        Self {
            entries: DashMap::new(),
            bytes: AtomicU64::new(0),
            budget: budget_bytes,
            clock: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Number of bodies held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no bodies are held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bodies dropped to stay within budget.
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    /// The configured byte budget.
    pub fn budget(&self) -> u64 {
        self.budget
    }

    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed)
    }

    /// Drop least-recently-read bodies until we are within budget.
    ///
    /// Snapshots the recency of every entry once and then removes in order,
    /// rather than re-scanning per eviction. Entries inserted during the scan
    /// may survive a round; the budget is a target, not a hard ceiling.
    fn evict_to_budget(&self) {
        if self.bytes.load(Ordering::Relaxed) <= self.budget {
            return;
        }
        let mut candidates: Vec<(u64, ContentHash)> = self
            .entries
            .iter()
            .map(|e| (e.value().last_read.load(Ordering::Relaxed), *e.key()))
            .collect();
        candidates.sort_unstable_by_key(|(recency, _)| *recency);

        for (_, hash) in candidates {
            if self.bytes.load(Ordering::Relaxed) <= self.budget {
                break;
            }
            if let Some((_, entry)) = self.entries.remove(&hash) {
                self.bytes
                    .fetch_sub(entry.body.len() as u64, Ordering::Relaxed);
                self.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl FragmentStore for MemoryStore {
    fn get(&self, hash: &ContentHash) -> Option<Bytes> {
        let entry = self.entries.get(hash)?;
        entry
            .value()
            .last_read
            .store(self.tick(), Ordering::Relaxed);
        Some(entry.value().body.clone())
    }

    fn put(&self, hash: &ContentHash, body: &Bytes) -> anyhow::Result<()> {
        let now = self.tick();
        // Content addressing means a present hash already holds these exact
        // bytes, so a re-put is a touch, not a write.
        if let Some(entry) = self.entries.get(hash) {
            entry.value().last_read.store(now, Ordering::Relaxed);
            return Ok(());
        }
        let len = body.len() as u64;
        let replaced = self.entries.insert(
            *hash,
            Entry {
                body: body.clone(),
                last_read: AtomicU64::new(now),
            },
        );
        match replaced {
            // Lost a race with another `put` of the same hash: same bytes, so
            // the accounting must not double-count.
            Some(old) => {
                self.bytes.fetch_add(len, Ordering::Relaxed);
                self.bytes
                    .fetch_sub(old.body.len() as u64, Ordering::Relaxed);
            }
            None => {
                self.bytes.fetch_add(len, Ordering::Relaxed);
            }
        }
        self.evict_to_budget();
        Ok(())
    }

    fn contains(&self, hash: &ContentHash) -> bool {
        self.entries.contains_key(hash)
    }

    fn size(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(s: &str) -> Bytes {
        Bytes::from(s.to_owned())
    }

    #[test]
    fn round_trips_a_body() {
        let store = MemoryStore::new(1024);
        let b = body("hello");
        let h = ContentHash::of(&b);
        assert!(store.get(&h).is_none());
        store.put(&h, &b).unwrap();
        assert!(store.contains(&h));
        assert_eq!(store.get(&h).as_deref(), Some(&b[..]));
        assert_eq!(store.size(), 5);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn re_putting_the_same_hash_does_not_double_count() {
        let store = MemoryStore::new(1024);
        let b = body("hello");
        let h = ContentHash::of(&b);
        store.put(&h, &b).unwrap();
        store.put(&h, &b).unwrap();
        assert_eq!(store.size(), 5);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn evicts_over_budget() {
        let store = MemoryStore::new(10);
        for i in 0..5 {
            let b = body(&format!("{i}{i}{i}{i}")); // 4 bytes each
            store.put(&ContentHash::of(&b), &b).unwrap();
        }
        assert!(
            store.size() <= 10,
            "store must stay within budget, got {}",
            store.size()
        );
        assert!(store.evictions() > 0);
        assert!(store.len() < 5);
    }

    #[test]
    fn evicts_the_least_recently_read_first() {
        let store = MemoryStore::new(8); // room for two 4-byte bodies
        let keep = body("keep");
        let drop = body("drop");
        let keep_h = ContentHash::of(&keep);
        let drop_h = ContentHash::of(&drop);
        store.put(&keep_h, &keep).unwrap();
        store.put(&drop_h, &drop).unwrap();

        // Touch `keep` so `drop` is the oldest read.
        assert!(store.get(&keep_h).is_some());

        let fresh = body("new!");
        store.put(&ContentHash::of(&fresh), &fresh).unwrap();

        assert!(store.contains(&keep_h), "recently read body must survive");
        assert!(
            !store.contains(&drop_h),
            "least recently read body must be evicted"
        );
    }

    #[test]
    fn a_body_larger_than_the_budget_does_not_wedge_the_store() {
        let store = MemoryStore::new(4);
        let big = body("much too large to fit");
        let h = ContentHash::of(&big);
        store.put(&h, &big).unwrap();
        // Either it was kept or immediately evicted; what matters is that the
        // loop terminated and the store is still usable.
        let small = body("ok");
        store.put(&ContentHash::of(&small), &small).unwrap();
        assert!(store.get(&ContentHash::of(&small)).is_some() || store.is_empty());
    }
}
