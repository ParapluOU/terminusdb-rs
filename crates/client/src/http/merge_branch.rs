//! Merge branch operations for atomic multi-operation transactions
//!
//! This module provides `with_merge_branch`, a higher-level API for executing
//! multiple operations atomically on a temporary branch and merging them back
//! on success.

use {
    super::{branch_client::BranchClient, client::TerminusDBHttpClient},
    crate::spec::BranchSpec,
    anyhow::Context,
    std::future::Future,
    std::sync::Arc,
    tokio::sync::Mutex,
    tracing::{debug, error, instrument},
};

/// Options for the merge branch operation
#[derive(Debug, Clone, Default)]
pub struct MergeBranchOptions {
    /// Whether to squash all commits before merging
    pub squash: bool,
    /// Author name for squash/rebase commits
    pub author: String,
    /// Message for squash commit (if squashing)
    pub squash_message: Option<String>,
    /// Message for rebase/merge commit
    pub merge_message: Option<String>,
    /// Optional lock serializing the final rebase onto the target branch.
    ///
    /// The insert + squash on the temporary branch parallelize freely, but the
    /// rebase moves the *target* branch's HEAD. When many merge-branches rebase
    /// onto the same target concurrently they race on that HEAD and the server
    /// answers "Unexpected failure in request handler". Sharing one lock across
    /// all concurrent `with_merge_branch` calls that target the same branch
    /// serializes just the (cheap) rebase step while keeping the (expensive)
    /// elaboration parallel. `None` preserves the prior unsynchronized behavior.
    pub merge_lock: Option<Arc<Mutex<()>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl TerminusDBHttpClient {
    /// Execute operations on a temporary branch and merge back on success.
    ///
    /// This method creates a temporary branch from the specified target,
    /// executes the provided closure with a `BranchClient` bound to that branch,
    /// and on success optionally squashes and rebases the changes onto the target.
    ///
    /// If the closure returns an error, the temporary branch is deleted
    /// without merging.
    ///
    /// # Arguments
    /// * `spec` - The target branch specification (where changes will be merged)
    /// * `options` - Merge options (squash, author, messages)
    /// * `f` - Async closure that receives a `BranchClient` and returns a Result
    ///
    /// # Returns
    /// The result of the closure on success, or the error on failure
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let client = TerminusDBHttpClient::local_node().await;
    /// # let spec = BranchSpec::new("mydb");
    /// let result = client.with_merge_branch(
    ///     &spec,
    ///     MergeBranchOptions { squash: true, author: "system".into(), ..Default::default() },
    ///     |branch_client| async move {
    ///         // All operations here happen on the temporary branch
    ///         // branch_client.save_instance(&model, args).await?;
    ///         Ok(())
    ///     }
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.merge_branch",
        skip(self, f),
        fields(
            db = %spec.db,
            target_branch = ?spec.branch,
            squash = %options.squash
        ),
        err
    )]
    pub async fn with_merge_branch<F, Fut, T>(
        &self,
        spec: &BranchSpec,
        options: MergeBranchOptions,
        f: F,
    ) -> anyhow::Result<T>
    where
        F: FnOnce(BranchClient) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        // 1. Generate temporary branch name
        let temp_branch_name = format!("_merge_branch_{}", uuid::Uuid::new_v4());

        // 2. Build paths
        let target_branch = spec.branch.as_deref().unwrap_or("main");
        let origin_path = format!("{}/{}/local/branch/{}", self.org, spec.db, target_branch);
        let temp_branch_path =
            format!("{}/{}/local/branch/{}", self.org, spec.db, temp_branch_name);

        debug!(
            "Creating temporary branch {} from {}",
            temp_branch_name, origin_path
        );

        // 3. Create temporary branch from target
        self.create_branch(&temp_branch_path, &origin_path)
            .await
            .context("Failed to create temporary merge branch")?;

        // 4. Create BranchClient bound to temp branch
        let working_spec = BranchSpec {
            db: spec.db.clone(),
            branch: Some(temp_branch_name.clone()),
            ref_commit: None,
        };

        let branch_client = BranchClient::new(
            self.clone(),
            working_spec,
            spec.clone(),
            temp_branch_name.clone(),
        );

        // 5. Execute the closure
        let result = f(branch_client).await;

        // 6. Handle result
        match result {
            Ok(value) => {
                // Success path: optionally squash, then rebase
                if options.squash {
                    let squash_msg = options
                        .squash_message
                        .unwrap_or_else(|| "Squashed merge branch commits".to_string());

                    debug!("Squashing commits on {}", temp_branch_path);

                    if let Err(e) = self
                        .squash(&temp_branch_path, &options.author, &squash_msg, None)
                        .await
                    {
                        // Cleanup on squash failure
                        error!("Squash failed, cleaning up temporary branch");
                        let _ = self.delete_branch(&temp_branch_path).await;
                        return Err(e).context("Failed to squash merge branch");
                    }
                }

                // Rebase onto target
                let merge_msg = options
                    .merge_message
                    .unwrap_or_else(|| "Merge from temporary branch".to_string());

                debug!("Rebasing {} onto {}", temp_branch_path, origin_path);

                // Serialize the rebase if a shared merge lock was supplied:
                // concurrent rebases onto the same target race on its HEAD. The
                // temp branch's commits survive a failed rebase, so we retry the
                // rebase (not the insert) with backoff — under contention the
                // server intermittently answers "Unexpected failure in request
                // handler", which clears once other writers settle.
                let rebase_result = {
                    let _merge_guard = match &options.merge_lock {
                        Some(lock) => Some(lock.lock().await),
                        None => None,
                    };
                    let backoffs_ms = [150u64, 400, 900, 2000, 4000];
                    let mut attempt = 0;
                    loop {
                        let r = self
                            .rebase(&origin_path, &temp_branch_path, &options.author, &merge_msg)
                            .await;
                        match r {
                            Ok(v) => break Ok(v),
                            Err(e) if attempt < backoffs_ms.len() => {
                                debug!(
                                    "Rebase of {} attempt {} failed ({e:#}); retrying",
                                    temp_branch_path,
                                    attempt + 1
                                );
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    backoffs_ms[attempt],
                                ))
                                .await;
                                attempt += 1;
                            }
                            Err(e) => break Err(e),
                        }
                    }
                };

                // Cleanup: delete temp branch regardless of rebase outcome
                debug!("Cleaning up temporary branch {}", temp_branch_path);
                let _ = self.delete_branch(&temp_branch_path).await;

                rebase_result.context("Failed to rebase merge branch onto target")?;

                Ok(value)
            }
            Err(e) => {
                // Error path: just cleanup
                eprintln!(
                    "[merge_branch] Closure failed with error: {:?}, cleaning up temporary branch {} without merging",
                    e, temp_branch_path
                );
                let delete_result = self.delete_branch(&temp_branch_path).await;
                eprintln!("[merge_branch] Delete branch result: {:?}", delete_result);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_branch_options_default() {
        let opts = MergeBranchOptions::default();
        assert!(!opts.squash);
        assert!(opts.author.is_empty());
        assert!(opts.squash_message.is_none());
        assert!(opts.merge_message.is_none());
    }
}
