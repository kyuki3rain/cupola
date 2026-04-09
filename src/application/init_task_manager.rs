use std::collections::{HashMap, HashSet};

use tokio::task::JoinHandle;

/// Manages tokio `JoinHandle`s for async init tasks (worktree creation).
///
/// Unlike Claude Code processes (managed by SessionManager), init tasks are
/// async Rust tasks rather than child processes.  We store them keyed by
/// `issue_id` and poll them each Resolve phase.
///
/// The `pending_ids` set tracks issues that have been *claimed* via
/// [`try_claim`] but whose `tokio::spawn` handle has not yet been registered.
/// This closes the TOCTOU window between the `is_active` check and
/// `register`: a second concurrent call for the same `issue_id` will see the
/// issue as already claimed and bail out immediately.
pub struct InitTaskManager {
    handles: HashMap<i64, JoinHandle<anyhow::Result<String>>>,
    pending_ids: HashSet<i64>,
}

/// Result collected from a completed init task.
pub struct InitTaskResult {
    pub issue_id: i64,
    /// `Ok(worktree_path)` or `Err(error)`.
    pub outcome: anyhow::Result<String>,
}

impl InitTaskManager {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
            pending_ids: HashSet::new(),
        }
    }

    /// Atomically claim a slot for the given issue before performing async I/O.
    ///
    /// Returns `true` if neither an active handle nor a pending claim exists
    /// for `issue_id`, and marks the issue as pending.  Returns `false`
    /// (without mutating state) when the issue is already active or claimed.
    ///
    /// The caller **must** subsequently call either [`register`] (to convert
    /// the claim into a live handle) or [`release_claim`] (to free the slot on
    /// any error path that prevents registration).
    pub fn try_claim(&mut self, issue_id: i64) -> bool {
        if self.handles.contains_key(&issue_id) || self.pending_ids.contains(&issue_id) {
            return false;
        }
        self.pending_ids.insert(issue_id);
        true
    }

    /// Release a claim without registering a handle (on error paths).
    pub fn release_claim(&mut self, issue_id: i64) {
        self.pending_ids.remove(&issue_id);
    }

    /// Register a new JoinHandle for the given issue (consumes the claim from
    /// [`try_claim`]).
    ///
    /// If a handle already exists for this issue it is explicitly aborted
    /// before being replaced.  Note: dropping a `JoinHandle` in Tokio does
    /// **not** cancel the task — the task becomes detached and keeps running.
    /// `abort()` is required to actually stop it.
    pub fn register(&mut self, issue_id: i64, handle: JoinHandle<anyhow::Result<String>>) {
        // Consume the pending claim (if one was taken via try_claim).
        self.pending_ids.remove(&issue_id);
        if let Some(old_handle) = self.handles.insert(issue_id, handle) {
            old_handle.abort();
        }
    }

    /// Collect all finished init tasks and remove them from the map.
    pub async fn collect_finished(&mut self) -> Vec<InitTaskResult> {
        let finished_ids: Vec<i64> = self
            .handles
            .iter()
            .filter(|(_, h)| h.is_finished())
            .map(|(&id, _)| id)
            .collect();

        let mut results = Vec::new();
        for id in finished_ids {
            if let Some(handle) = self.handles.remove(&id) {
                let outcome = match handle.await {
                    Ok(inner) => inner,
                    Err(e) => Err(anyhow::anyhow!("init task panicked: {e}")),
                };
                results.push(InitTaskResult {
                    issue_id: id,
                    outcome,
                });
            }
        }
        results
    }

    /// Returns true if there is an active handle or a pending claim for the issue.
    pub fn is_active(&self, issue_id: i64) -> bool {
        self.handles.contains_key(&issue_id) || self.pending_ids.contains(&issue_id)
    }

    /// Cancel and remove the handle for an issue (e.g. on cancellation).
    pub fn cancel(&mut self, issue_id: i64) {
        if let Some(handle) = self.handles.remove(&issue_id) {
            handle.abort();
        }
    }

    /// Abort all in-flight init tasks (e.g. on graceful shutdown).
    /// Drains the internal map so all handles are aborted.
    pub fn abort_all(&mut self) {
        for (_issue_id, handle) in self.handles.drain() {
            handle.abort();
        }
    }

    /// Number of active (in-flight) init tasks.
    pub fn count(&self) -> usize {
        self.handles.len()
    }
}

impl Default for InitTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- try_claim / release_claim tests ---

    /// T-3.IT.C1: try_claim prevents duplicate claims for the same issue_id.
    #[tokio::test]
    async fn try_claim_prevents_duplicate_for_same_issue() {
        let mut mgr = InitTaskManager::new();
        assert!(mgr.try_claim(42), "first claim should succeed");
        assert!(!mgr.try_claim(42), "duplicate claim should fail");
        assert!(mgr.is_active(42), "pending claim counts as active");
        mgr.release_claim(42);
        assert!(!mgr.is_active(42));
    }

    /// T-3.IT.C2: try_claim allows different issue_ids independently.
    #[tokio::test]
    async fn try_claim_allows_different_issues() {
        let mut mgr = InitTaskManager::new();
        assert!(mgr.try_claim(1));
        assert!(mgr.try_claim(2));
        mgr.release_claim(1);
        mgr.release_claim(2);
    }

    /// T-3.IT.C3: release_claim frees the slot so a subsequent try_claim
    /// can succeed.
    #[tokio::test]
    async fn release_claim_frees_slot() {
        let mut mgr = InitTaskManager::new();
        assert!(mgr.try_claim(42));
        assert!(!mgr.try_claim(42));
        mgr.release_claim(42);
        assert!(mgr.try_claim(42), "slot freed — should succeed now");
        mgr.release_claim(42);
    }

    /// T-3.IT.C4: register() consumes the pending claim and installs the handle.
    #[tokio::test]
    async fn register_consumes_claim() {
        let mut mgr = InitTaskManager::new();
        assert!(mgr.try_claim(42));
        assert!(mgr.is_active(42));

        let handle = tokio::spawn(async { Ok("/tmp/wt".to_string()) });
        mgr.register(42, handle);

        // Pending claim is gone, but the handle is now active.
        assert!(mgr.is_active(42));
        assert!(!mgr.try_claim(42), "handle present — claim should fail");

        mgr.cancel(42);
    }

    /// T-3.IT.C5: is_active returns true for both pending claims and live handles.
    #[tokio::test]
    async fn is_active_reflects_pending_and_registered() {
        let mut mgr = InitTaskManager::new();
        assert!(!mgr.is_active(99));

        // Pending claim.
        mgr.try_claim(99);
        assert!(mgr.is_active(99));

        // Convert to handle.
        let handle = tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            Ok("/tmp/never".to_string())
        });
        mgr.register(99, handle);
        assert!(mgr.is_active(99));

        mgr.cancel(99);
        assert!(!mgr.is_active(99));
    }

    /// T-3.IT.C6: N sequential try_claim calls for the same issue result in
    /// exactly one success (sequential stress test).
    #[tokio::test]
    async fn at_most_one_claim_succeeds_under_sequential_stress() {
        let mut mgr = InitTaskManager::new();
        let issue_id = 99i64;

        let mut successes = 0usize;
        for _ in 0..5 {
            if mgr.try_claim(issue_id) {
                successes += 1;
            }
        }

        assert_eq!(successes, 1, "exactly one claim should succeed");
        mgr.release_claim(issue_id);
        assert!(!mgr.is_active(issue_id));
    }

    /// T-3.IT.1: register + is_active
    #[tokio::test]
    async fn register_and_is_active() {
        let mut mgr = InitTaskManager::new();
        assert!(!mgr.is_active(1));

        let handle = tokio::spawn(async { Ok("/tmp/wt".to_string()) });
        mgr.register(1, handle);
        assert!(mgr.is_active(1));
        assert_eq!(mgr.count(), 1);
    }

    /// T-3.IT.2: collect_finished returns Ok result
    #[tokio::test]
    async fn collect_finished_ok() {
        let mut mgr = InitTaskManager::new();
        let handle = tokio::spawn(async { Ok("/tmp/wt/issue-42".to_string()) });
        mgr.register(42, handle);

        // Wait for task to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let results = mgr.collect_finished().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].issue_id, 42);
        assert!(results[0].outcome.is_ok());
        assert_eq!(results[0].outcome.as_deref().unwrap(), "/tmp/wt/issue-42");
        assert!(!mgr.is_active(42));
    }

    /// T-3.IT.3: collect_finished returns Err result
    #[tokio::test]
    async fn collect_finished_err() {
        let mut mgr = InitTaskManager::new();
        let handle = tokio::spawn(async { Err(anyhow::anyhow!("git fetch failed")) });
        mgr.register(99, handle);

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let results = mgr.collect_finished().await;
        assert_eq!(results.len(), 1);
        assert!(results[0].outcome.is_err());
    }

    /// T-3.IT.4: collect_finished does not return still-running tasks
    #[tokio::test]
    async fn collect_finished_skips_running() {
        let mut mgr = InitTaskManager::new();
        let handle = tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            Ok("/tmp/never".to_string())
        });
        mgr.register(1, handle);

        let results = mgr.collect_finished().await;
        assert!(results.is_empty());
        assert!(mgr.is_active(1));

        mgr.cancel(1);
    }

    /// T-3.IT.5: cancel aborts the task and removes it
    #[tokio::test]
    async fn cancel_removes_handle() {
        let mut mgr = InitTaskManager::new();
        let handle = tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            Ok("/tmp/never".to_string())
        });
        mgr.register(5, handle);
        assert!(mgr.is_active(5));

        mgr.cancel(5);
        assert!(!mgr.is_active(5));
        assert_eq!(mgr.count(), 0);
    }

    /// T-3.IT.6: abort_all aborts all in-flight tasks and empties the map
    #[tokio::test]
    async fn abort_all_drains_handles() {
        let mut mgr = InitTaskManager::new();
        for id in 1..=3i64 {
            let handle = tokio::spawn(async {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                Ok("/tmp/never".to_string())
            });
            mgr.register(id, handle);
        }
        assert_eq!(mgr.count(), 3);

        mgr.abort_all();

        assert_eq!(
            mgr.count(),
            0,
            "all handles should be removed after abort_all"
        );
        assert!(!mgr.is_active(1));
        assert!(!mgr.is_active(2));
        assert!(!mgr.is_active(3));
    }
}
