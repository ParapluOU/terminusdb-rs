//! The filesystem content-addressed tier — also the shared/NAS tier.
//!
//! Layout is `<root>/<first two hex chars>/<full hex>`, with a sibling `.tmp`
//! directory for in-progress writes. The two-character fan-out keeps directory
//! sizes manageable on filesystems that degrade with very wide directories.
//!
//! Writes are **write-to-temp-then-rename**, which on POSIX is atomic within a
//! filesystem. That single property is what makes this double as the shared tier
//! of design doc §5.7 mode 2: a reader either does not see a file yet or sees it
//! complete, so two nodes pointed at the same directory need no locking, no
//! coherence protocol and no agreement about who writes what. The temp directory
//! lives inside the root precisely so the rename never crosses a filesystem.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Context;
use bytes::Bytes;

use crate::ids::ContentHash;
use crate::store::FragmentStore;

/// A content-addressed store backed by a directory.
pub struct FsCasStore {
    root: PathBuf,
    tmp: PathBuf,
    bytes: AtomicU64,
    /// Monotonic suffix so two concurrent writers in this process never pick
    /// the same temp name.
    nonce: AtomicU64,
    verify_on_read: bool,
    corruptions: AtomicU64,
}

impl FsCasStore {
    /// Open (creating if needed) a store rooted at `root`, and account for what
    /// is already there.
    ///
    /// The initial size walk is startup work: it reads directory metadata only,
    /// never file contents, and tolerates unreadable entries — on a shared tier
    /// another node may be writing while we walk.
    pub fn open(root: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let root = root.into();
        let tmp = root.join(".tmp");
        fs::create_dir_all(&tmp)
            .with_context(|| format!("creating fragment store at {}", root.display()))?;
        let store = Self {
            root,
            tmp,
            bytes: AtomicU64::new(0),
            nonce: AtomicU64::new(0),
            verify_on_read: true,
            corruptions: AtomicU64::new(0),
        };
        store.bytes.store(store.scan_size(), Ordering::Relaxed);
        Ok(store)
    }

    /// Turn off hash verification on read.
    ///
    /// Verification costs one blake3 pass per warm-tier read and protects
    /// against a truncated or corrupted file (which rename alone cannot rule
    /// out — a half-written *temp* file that was renamed by a crashed process
    /// still looks complete). Leave it on unless the warm tier is on known-good
    /// local storage and profiling says it matters.
    pub fn without_read_verification(mut self) -> Self {
        self.verify_on_read = false;
        self
    }

    /// Files whose contents did not match their name. Any non-zero value here
    /// means the directory is damaged or being written by something that is not
    /// a Reactor.
    pub fn corruptions(&self) -> u64 {
        self.corruptions.load(Ordering::Relaxed)
    }

    /// The store root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, hash: &ContentHash) -> PathBuf {
        let hex = hash.to_hex();
        self.root.join(&hex[..2]).join(&hex)
    }

    fn scan_size(&self) -> u64 {
        let mut total = 0;
        let shards = match fs::read_dir(&self.root) {
            Ok(shards) => shards,
            Err(_) => return 0,
        };
        for shard in shards.flatten() {
            if shard.path() == self.tmp {
                continue;
            }
            if let Ok(entries) = fs::read_dir(shard.path()) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            total += meta.len();
                        }
                    }
                }
            }
        }
        total
    }
}

impl FragmentStore for FsCasStore {
    fn get(&self, hash: &ContentHash) -> Option<Bytes> {
        // A missing or unreadable file is a miss, not an error: the caller
        // refills. This is also what makes a concurrently-written shared tier
        // safe to read.
        let body = fs::read(self.path_for(hash)).ok()?;
        if self.verify_on_read && &ContentHash::of(&body) != hash {
            self.corruptions.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                %hash,
                "fragment store entry does not match its content hash; treating as a miss"
            );
            return None;
        }
        Some(Bytes::from(body))
    }

    fn put(&self, hash: &ContentHash, body: &Bytes) -> anyhow::Result<()> {
        let final_path = self.path_for(hash);
        if final_path.exists() {
            // Immutable content: already correct by construction.
            return Ok(());
        }
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating shard directory {}", parent.display()))?;
        }
        let tmp_path = self.tmp.join(format!(
            "{}.{}.{}",
            hash.to_hex(),
            std::process::id(),
            self.nonce.fetch_add(1, Ordering::Relaxed)
        ));
        {
            let mut file = fs::File::create(&tmp_path)
                .with_context(|| format!("creating {}", tmp_path.display()))?;
            file.write_all(body)
                .with_context(|| format!("writing {}", tmp_path.display()))?;
            file.flush()?;
        }
        // Atomic publish. If a sibling node won the race, the rename simply
        // replaces identical bytes with identical bytes.
        match fs::rename(&tmp_path, &final_path) {
            Ok(()) => {
                self.bytes.fetch_add(body.len() as u64, Ordering::Relaxed);
                Ok(())
            }
            Err(err) => {
                let _ = fs::remove_file(&tmp_path);
                Err(err).with_context(|| format!("publishing {}", final_path.display()))
            }
        }
    }

    fn contains(&self, hash: &ContentHash) -> bool {
        self.path_for(hash).exists()
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
    fn round_trips_through_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsCasStore::open(dir.path()).unwrap();
        let b = body("some response body");
        let h = ContentHash::of(&b);

        assert!(store.get(&h).is_none());
        assert!(!store.contains(&h));
        store.put(&h, &b).unwrap();
        assert!(store.contains(&h));
        assert_eq!(store.get(&h).as_deref(), Some(&b[..]));
        assert_eq!(store.size(), b.len() as u64);
    }

    #[test]
    fn uses_a_sharded_layout_and_no_temp_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsCasStore::open(dir.path()).unwrap();
        let b = body("x");
        let h = ContentHash::of(&b);
        store.put(&h, &b).unwrap();

        let hex = h.to_hex();
        assert!(dir.path().join(&hex[..2]).join(&hex).is_file());
        assert_eq!(
            fs::read_dir(dir.path().join(".tmp")).unwrap().count(),
            0,
            "temp files must be renamed away, not left behind"
        );
    }

    #[test]
    fn re_putting_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsCasStore::open(dir.path()).unwrap();
        let b = body("twice");
        let h = ContentHash::of(&b);
        store.put(&h, &b).unwrap();
        store.put(&h, &b).unwrap();
        assert_eq!(store.size(), b.len() as u64, "size must not double-count");
    }

    #[test]
    fn reopening_accounts_for_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = FsCasStore::open(dir.path()).unwrap();
            let b = body("persisted");
            store.put(&ContentHash::of(&b), &b).unwrap();
        }
        let reopened = FsCasStore::open(dir.path()).unwrap();
        assert_eq!(reopened.size(), "persisted".len() as u64);
        assert!(reopened.contains(&ContentHash::of(&body("persisted"))));
    }

    #[test]
    fn two_stores_on_one_directory_share_fragments() {
        // The shared-tier property of design doc §5.7 mode 2, in miniature.
        let dir = tempfile::tempdir().unwrap();
        let node_a = FsCasStore::open(dir.path()).unwrap();
        let node_b = FsCasStore::open(dir.path()).unwrap();
        let b = body("filled once, read twice");
        let h = ContentHash::of(&b);

        node_a.put(&h, &b).unwrap();
        assert_eq!(
            node_b.get(&h).as_deref(),
            Some(&b[..]),
            "an immutable fragment written by one node is readable by another \
             with no coherence protocol at all"
        );
    }

    #[test]
    fn a_corrupted_entry_reads_as_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsCasStore::open(dir.path()).unwrap();
        let b = body("original");
        let h = ContentHash::of(&b);
        store.put(&h, &b).unwrap();

        // Simulate damage: the file no longer matches its name.
        fs::write(store.path_for(&h), b"tampered").unwrap();
        assert!(
            store.get(&h).is_none(),
            "a body that does not hash to its name must never be served"
        );
        assert_eq!(store.corruptions(), 1);
    }

    #[test]
    fn verification_can_be_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsCasStore::open(dir.path())
            .unwrap()
            .without_read_verification();
        let b = body("original");
        let h = ContentHash::of(&b);
        store.put(&h, &b).unwrap();
        fs::write(store.path_for(&h), b"tampered").unwrap();
        assert_eq!(store.get(&h).as_deref(), Some(&b"tampered"[..]));
        assert_eq!(store.corruptions(), 0);
    }
}
