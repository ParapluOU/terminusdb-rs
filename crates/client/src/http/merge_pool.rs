//! A pool that manages branch-per-write with a single sequential merger.
//!
//! `with_merge_branch` couples insert + merge in one call, so fanning it out
//! means every writer also rebases *itself* — the concurrent rebases race the
//! target branch's HEAD (`unexpected_rebase_failure`) and, even serialized with a
//! shared lock, each writer stays parked until its own merge lands (so the
//! parallelism cap throttles the whole insert+merge pipeline).
//!
//! [`BranchMergePool`] separates the two concerns:
//!   * **Writers** run with bounded parallelism (a semaphore). Each acquires a
//!     permit, creates a temp branch, runs the caller's insert, squashes, then
//!     **releases the permit** and hands the branch to the merger.
//!   * **A single merger task** drains completed branches and rebases them onto
//!     the target one at a time. Because it is the only thing touching the target
//!     HEAD, there is no contention — no lock, no rebase races.
//!
//! Note: this does not (and cannot) fix *content* conflicts — two branches that
//! insert the same content-addressed `@id` will still conflict on the second
//! rebase. It is the right tool when the branches write **disjoint** doc sets
//! (e.g. chunks of one already-deduped leaf-first layer), not for overlapping
//! writes into one DB.

#[cfg(not(target_arch = "wasm32"))]
use {
    super::{branch_client::BranchClient, client::TerminusDBHttpClient},
    crate::spec::BranchSpec,
    anyhow::{anyhow, Context},
    std::future::Future,
    std::sync::atomic::{AtomicUsize, Ordering},
    std::sync::Arc,
    std::time::Duration,
    tokio::sync::{mpsc, oneshot, Semaphore},
    tracing::debug,
};

/// Configuration for a [`BranchMergePool`].
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct MergePoolOptions {
    /// Maximum number of writer branches inserting concurrently.
    pub max_parallel_writers: usize,
    /// Squash each temp branch to a single commit before merging.
    pub squash: bool,
    /// Author recorded on squash + merge commits.
    pub author: String,
    /// Backoff schedule (ms) for retrying a transient rebase failure. With a
    /// single merger these should be rare; kept small.
    pub merge_backoffs_ms: Vec<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for MergePoolOptions {
    fn default() -> Self {
        Self {
            max_parallel_writers: 16,
            squash: true,
            author: "merge-pool".to_string(),
            merge_backoffs_ms: vec![150, 400, 900, 2000],
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Stats {
    submitted: AtomicUsize,
    writing: AtomicUsize,         // gauge: writers currently inserting
    queued_for_merge: AtomicUsize, // gauge: branches written, awaiting the merger
    merged: AtomicUsize,
    write_failed: AtomicUsize,
    merge_failed: AtomicUsize,
}

/// A snapshot of a [`BranchMergePool`]'s counters.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy)]
pub struct MergePoolStats {
    pub submitted: usize,
    pub writing: usize,
    pub queued_for_merge: usize,
    pub merged: usize,
    pub write_failed: usize,
    pub merge_failed: usize,
}

/// A branch handed from a writer to the single merger.
#[cfg(not(target_arch = "wasm32"))]
struct MergeReq {
    /// Full path of the temp branch (for cleanup).
    temp_branch_path: String,
    /// Commit the temp branch forked from — `apply`'s `before` (diff base).
    before_commit: String,
    /// Temp branch HEAD after the insert — `apply`'s `after`.
    after_commit: String,
    done: oneshot::Sender<anyhow::Result<()>>,
}

/// Bounded-parallel writer branches with a single sequential merger.
///
/// ```no_run
/// # use terminusdb_client::*;
/// # async fn ex(client: TerminusDBHttpClient) -> anyhow::Result<()> {
/// let spec = BranchSpec::new("mydb");
/// let pool = BranchMergePool::new(client, spec, MergePoolOptions::default());
/// // fan chunks of a deduped leaf layer across writers; a single task merges them
/// let jobs = (0..8).map(|i| pool.submit(move |bc| async move {
///     // bc.insert_instances(chunk_i, args).await
///     Ok::<_, anyhow::Error>(i)
/// }));
/// let results = futures::future::join_all(jobs).await;
/// let stats = pool.join().await;
/// # let _ = (results, stats);
/// # Ok(())
/// # }
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub struct BranchMergePool {
    client: TerminusDBHttpClient,
    target: BranchSpec,
    opts: MergePoolOptions,
    writer_permits: Arc<Semaphore>,
    merge_tx: mpsc::UnboundedSender<MergeReq>,
    stats: Arc<Stats>,
    merger: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl BranchMergePool {
    /// Create a pool targeting `target` and spawn its single merger task.
    pub fn new(client: TerminusDBHttpClient, target: BranchSpec, opts: MergePoolOptions) -> Self {
        let (merge_tx, merge_rx) = mpsc::unbounded_channel::<MergeReq>();
        let writer_permits = Arc::new(Semaphore::new(opts.max_parallel_writers.max(1)));
        let stats = Arc::new(Stats::default());
        let merger = tokio::spawn(Self::merger_loop(
            client.clone(),
            target.clone(),
            opts.author.clone(),
            opts.merge_backoffs_ms.clone(),
            merge_rx,
            stats.clone(),
        ));
        Self {
            client,
            target,
            opts,
            writer_permits,
            merge_tx,
            stats,
            merger: std::sync::Mutex::new(Some(merger)),
        }
    }

    /// Live snapshot of the pool's counters.
    pub fn stats(&self) -> MergePoolStats {
        let s = &self.stats;
        MergePoolStats {
            submitted: s.submitted.load(Ordering::Relaxed),
            writing: s.writing.load(Ordering::Relaxed),
            queued_for_merge: s.queued_for_merge.load(Ordering::Relaxed),
            merged: s.merged.load(Ordering::Relaxed),
            write_failed: s.write_failed.load(Ordering::Relaxed),
            merge_failed: s.merge_failed.load(Ordering::Relaxed),
        }
    }

    /// Submit an insert job. The returned future resolves once this job's branch
    /// has been written **and** merged onto the target (or with the first error).
    ///
    /// The closure returns `(value, after_commit)`. Pass the write's commit id as
    /// `after_commit` — it comes free from the `TerminusDB-Data-Version` response
    /// header (`insert_instances(..).extract_commit_id()`), avoiding a follow-up
    /// `log` round-trip (~16-30 ms each) to discover the temp branch HEAD. Pass
    /// `None` to fall back to a `log` lookup. (Only valid when `squash == false`,
    /// since squashing rewrites the HEAD after the insert.)
    ///
    /// Drive many of these concurrently (e.g. `futures::future::join_all`): the
    /// semaphore caps how many actually insert at once, the freed permit lets the
    /// next writer start while this one waits for the shared merger, and the
    /// merger applies completed branches sequentially.
    pub async fn submit<F, Fut, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(BranchClient) -> Fut,
        Fut: Future<Output = anyhow::Result<(T, Option<crate::CommitId>)>>,
    {
        self.stats.submitted.fetch_add(1, Ordering::Relaxed);

        // Bound writer parallelism. The permit covers create+insert+squash only;
        // it is released before the merge so writers don't queue behind merges.
        let permit = self
            .writer_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| anyhow!("merge-pool: writer semaphore closed"))?;

        let temp_name = format!("_merge_branch_{}", uuid::Uuid::new_v4());
        let target_branch = self.target.branch.as_deref().unwrap_or("main");
        let origin_path = format!(
            "{}/{}/local/branch/{}",
            self.client.org, self.target.db, target_branch
        );
        let temp_path = format!(
            "{}/{}/local/branch/{}",
            self.client.org, self.target.db, temp_name
        );

        let t_create = std::time::Instant::now();
        self.client
            .create_branch(&temp_path, &origin_path)
            .await
            .context("merge-pool: create temp branch")?;
        let create_ms = t_create.elapsed();

        let working_spec = BranchSpec {
            db: self.target.db.clone(),
            branch: Some(temp_name.clone()),
            ref_commit: None,
        };
        // The commit the temp branch forked from == its HEAD right after creation.
        // `apply` diffs from here, so it doesn't matter that the target HEAD moves
        // while other branches merge.
        let t_fork = std::time::Instant::now();
        let fork_point = self
            .client
            .get_latest_commit_id(&working_spec)
            .await
            .context("merge-pool: read fork-point commit")?
            .to_string();
        let fork_ms = t_fork.elapsed();
        let bc = BranchClient::new(
            self.client.clone(),
            working_spec.clone(),
            self.target.clone(),
            temp_name.clone(),
        );

        self.stats.writing.fetch_add(1, Ordering::Relaxed);
        let t_ins = std::time::Instant::now();
        let write = f(bc).await;
        let ins_ms = t_ins.elapsed();
        self.stats.writing.fetch_sub(1, Ordering::Relaxed);

        let (value, after_hint) = match write {
            Ok(v) => v,
            Err(e) => {
                let _ = self.client.delete_branch(&temp_path).await;
                self.stats.write_failed.fetch_add(1, Ordering::Relaxed);
                return Err(e).context("merge-pool: writer insert failed");
            }
        };

        if self.opts.squash {
            if let Err(e) = self
                .client
                .squash(&temp_path, &self.opts.author, "merge-pool squash", None)
                .await
            {
                let _ = self.client.delete_branch(&temp_path).await;
                self.stats.write_failed.fetch_add(1, Ordering::Relaxed);
                return Err(e).context("merge-pool: squash failed");
            }
        }

        // The temp branch's HEAD now holds the insert — `apply`'s `after`. Using an
        // explicit commit id (not the branch name) removes any ambiguity in how the
        // server resolves the endpoint of the diff.
        let t_after = std::time::Instant::now();
        let after_commit = match after_hint {
            // Free: the insert's `TerminusDB-Data-Version` header already carried
            // the temp branch's new HEAD.
            Some(c) => c.to_string(),
            // Fall back to a `log` lookup (a full HTTP round-trip).
            None => self
                .client
                .get_latest_commit_id(&working_spec)
                .await
                .context("merge-pool: read temp HEAD commit")?
                .to_string(),
        };
        let after_ms = t_after.elapsed();
        debug!(
            "merge-pool timing: create={:?} fork_commit_lookup={:?} insert={:?} after_commit_lookup={:?}",
            create_ms, fork_ms, ins_ms, after_ms
        );

        // Free the writer slot BEFORE the (single-threaded) merge — the decoupling.
        drop(permit);

        // Hand the branch to the single merger and await the outcome.
        let (done_tx, done_rx) = oneshot::channel();
        self.stats.queued_for_merge.fetch_add(1, Ordering::Relaxed);
        self.merge_tx
            .send(MergeReq {
                temp_branch_path: temp_path,
                before_commit: fork_point,
                after_commit,
                done: done_tx,
            })
            .map_err(|_| anyhow!("merge-pool: merger has shut down"))?;

        let merge_result = done_rx
            .await
            .map_err(|_| anyhow!("merge-pool: merger dropped the job"))?;
        self.stats.queued_for_merge.fetch_sub(1, Ordering::Relaxed);
        merge_result.context("merge-pool: merge onto target failed")?;
        self.stats.merged.fetch_add(1, Ordering::Relaxed);

        Ok(value)
    }

    /// Close the pool: stop accepting work and wait for the merger to drain.
    /// Call after all outstanding [`submit`](Self::submit) futures have resolved.
    pub async fn join(self) -> MergePoolStats {
        // Dropping the only sender lets the merger's recv() return None once the
        // queue is empty, so it exits cleanly.
        drop(self.merge_tx);
        let handle = self.merger.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.await;
        }
        let s = &self.stats;
        MergePoolStats {
            submitted: s.submitted.load(Ordering::Relaxed),
            writing: s.writing.load(Ordering::Relaxed),
            queued_for_merge: s.queued_for_merge.load(Ordering::Relaxed),
            merged: s.merged.load(Ordering::Relaxed),
            write_failed: s.write_failed.load(Ordering::Relaxed),
            merge_failed: s.merge_failed.load(Ordering::Relaxed),
        }
    }

    /// The single merger: `apply` each completed branch's diff onto the target,
    /// one at a time.
    async fn merger_loop(
        client: TerminusDBHttpClient,
        target: BranchSpec,
        author: String,
        backoffs_ms: Vec<u64>,
        mut rx: mpsc::UnboundedReceiver<MergeReq>,
        stats: Arc<Stats>,
    ) {
        while let Some(req) = rx.recv().await {
            // Merge via `apply`, not `rebase`. `apply` patches the diff
            // (fork_point -> temp HEAD) onto the target's CURRENT state, so it does
            // not replay commits on a moved base — disjoint writes land regardless
            // of HEAD movement (rebase throws `unexpected_rebase_failure` there).
            // `keep {@id}` resolves an already-present identical doc as a no-op,
            // which also covers content-addressed overlap across branches.
            let apply_opts = super::apply::ApplyOptions {
                match_final_state: false,
                keep: None,
            };
            let mut attempt = 0usize;
            let result = loop {
                match client
                    .apply_commit_diff(
                        &target,
                        &req.before_commit,
                        &req.after_commit,
                        &author,
                        "merge-pool apply",
                        apply_opts.clone(),
                    )
                    .await
                {
                    Ok(v) => {
                        // apply_commit_diff returns Ok even when the server reports
                        // a document-level conflict (api:conflict). Treat that as a
                        // failure so the pool doesn't silently store nothing.
                        if v.get("api:status").and_then(|s| s.as_str()) == Some("api:conflict") {
                            break Err(anyhow!(
                                "merge-pool: apply reported api:conflict: {}",
                                serde_json::to_string(&v).unwrap_or_default()
                            ));
                        }
                        break Ok(());
                    }
                    Err(e) if attempt < backoffs_ms.len() => {
                        debug!(
                            "merge-pool: apply of {} attempt {} failed ({e:#}); retrying",
                            req.temp_branch_path,
                            attempt + 1
                        );
                        tokio::time::sleep(Duration::from_millis(backoffs_ms[attempt])).await;
                        attempt += 1;
                    }
                    Err(e) => break Err(e),
                }
            };

            // Clean up the temp branch regardless of outcome.
            let _ = client.delete_branch(&req.temp_branch_path).await;
            if result.is_err() {
                stats.merge_failed.fetch_add(1, Ordering::Relaxed);
            }
            let _ = req.done.send(result);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl TerminusDBHttpClient {
    /// Convenience constructor for a [`BranchMergePool`] targeting `spec`.
    pub fn merge_pool(&self, spec: BranchSpec, opts: MergePoolOptions) -> BranchMergePool {
        BranchMergePool::new(self.clone(), spec, opts)
    }
}
