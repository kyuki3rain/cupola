use anyhow::Result;
use std::path::Path;

/// Returned by [`GitWorktree::merge`] when the merge fails due to a conflict
/// (exit code 1).  Other merge failures (fatal errors, exit code 2+) are
/// represented as plain [`anyhow::Error`] values.
#[derive(Debug, thiserror::Error)]
#[error("merge conflict detected")]
pub struct MergeConflictError;

pub trait GitWorktree: Send + Sync {
    fn fetch(&self) -> Result<()>;
    /// Merge `origin/{branch}` into the worktree.
    ///
    /// `branch` must be the plain branch name (e.g. `"main"`). The
    /// implementation prepends `origin/` itself and rejects any name that
    /// already starts with `origin/` with a distinct error (not a
    /// [`MergeConflictError`]), so misuse is visible immediately.
    fn merge(&self, worktree_path: &Path, branch: &str) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn create(&self, path: &Path, branch: &str, start_point: &str) -> Result<()>;
    fn remove(&self, path: &Path) -> Result<()>;
    fn create_branch(&self, worktree_path: &Path, branch: &str) -> Result<()>;
    fn checkout(&self, worktree_path: &Path, branch: &str) -> Result<()>;
    fn pull(&self, worktree_path: &Path) -> Result<()>;
    fn push(&self, worktree_path: &Path, branch: &str) -> Result<()>;
    fn delete_branch(&self, branch: &str) -> Result<()>;
}
