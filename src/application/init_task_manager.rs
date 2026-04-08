use std::collections::HashMap;

use tokio::task::JoinHandle;

/// Manages tokio `JoinHandle`s for async init tasks (worktree creation).
///
/// Unlike Claude Code processes (managed by SessionManager), init tasks are
/// async Rust tasks rather than child processes.  We store them keyed by
/// `issue_id` and poll them each Resolve phase.
pub struct InitTaskManager {
    handles: HashMap<i64, JoinHandle<anyhow::Result<String>>>,
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
        }
    }

    /// Register a new JoinHandle for the given issue.
    /// If a handle already exists for this issue, it is dropped (and the task
    /// is cancelled by Tokio when all handles are dropped).
    pub fn register(&mut self, issue_id: i64, handle: JoinHandle<anyhow::Result<String>>) {
        self.handles.insert(issue_id, handle);
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

    /// Returns true if there is an active (possibly still running) handle for the issue.
    pub fn is_active(&self, issue_id: i64) -> bool {
        self.handles.contains_key(&issue_id)
    }

    /// Cancel and remove the handle for an issue (e.g. on cancellation).
    pub fn cancel(&mut self, issue_id: i64) {
        if let Some(handle) = self.handles.remove(&issue_id) {
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
}
