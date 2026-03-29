use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

use crate::application::port::git_worktree::GitWorktree;

pub struct GitWorktreeManager {
    repo_root: std::path::PathBuf,
}

impl GitWorktreeManager {
    pub fn new(repo_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    fn run_git(&self, args: &[&str]) -> Result<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .with_context(|| format!("failed to execute git {}", args.join(" ")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("git {} failed: {}", args.join(" "), stderr.trim()));
        }

        Ok(())
    }

    fn run_git_in_dir(&self, worktree_path: &Path, args: &[&str]) -> Result<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(worktree_path)
            .output()
            .with_context(|| format!("failed to execute git {}", args.join(" ")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("git {} failed: {}", args.join(" "), stderr.trim()));
        }

        Ok(())
    }

    /// Try to run a git command, returning Ok(()) even if it fails
    /// (used for idempotent cleanup operations).
    fn run_git_idempotent(&self, args: &[&str]) {
        let _ = self.run_git(args);
    }
}

impl GitWorktree for GitWorktreeManager {
    fn create(&self, path: &Path, branch: &str, start_point: &str) -> Result<()> {
        if path.exists() {
            tracing::info!(path = %path.display(), "worktree already exists, skipping create");
            return Ok(());
        }

        let path_str = path.to_string_lossy();
        self.run_git(&["worktree", "add", "-b", branch, &path_str, start_point])
            .with_context(|| format!("failed to create worktree at {}", path.display()))
    }

    fn remove(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            tracing::info!(path = %path.display(), "worktree does not exist, skipping remove");
            return Ok(());
        }

        let path_str = path.to_string_lossy();
        self.run_git(&["worktree", "remove", "--force", &path_str])
            .with_context(|| format!("failed to remove worktree at {}", path.display()))
    }

    fn create_branch(&self, worktree_path: &Path, branch: &str) -> Result<()> {
        self.run_git_in_dir(worktree_path, &["checkout", "-b", branch])
            .with_context(|| format!("failed to create branch {branch}"))
    }

    fn checkout(&self, worktree_path: &Path, branch: &str) -> Result<()> {
        self.run_git_in_dir(worktree_path, &["checkout", branch])
            .with_context(|| format!("failed to checkout branch {branch}"))
    }

    fn pull(&self, worktree_path: &Path) -> Result<()> {
        self.run_git_in_dir(worktree_path, &["pull"])
            .context("failed to git pull")
    }

    fn push(&self, worktree_path: &Path, branch: &str) -> Result<()> {
        self.run_git_in_dir(worktree_path, &["push", "-u", "origin", branch])
            .with_context(|| format!("failed to push branch {branch}"))
    }

    fn delete_branch(&self, branch: &str) -> Result<()> {
        // Local delete (idempotent)
        self.run_git_idempotent(&["branch", "-D", branch]);

        // Remote delete (idempotent)
        self.run_git_idempotent(&["push", "origin", "--delete", branch]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_nonexistent_worktree_is_ok() {
        let mgr = GitWorktreeManager::new("/tmp/nonexistent-repo");
        let result = mgr.remove(Path::new("/tmp/nonexistent-worktree"));
        assert!(
            result.is_ok(),
            "removing nonexistent worktree should succeed"
        );
    }

    #[test]
    fn create_existing_worktree_is_ok() {
        let mgr = GitWorktreeManager::new("/tmp");
        // /tmp exists, so create should skip
        let result = mgr.create(Path::new("/tmp"), "branch", "HEAD");
        assert!(result.is_ok(), "creating at existing path should succeed");
    }

    #[test]
    fn delete_branch_is_always_ok() {
        let mgr = GitWorktreeManager::new("/tmp");
        let result = mgr.delete_branch("nonexistent-branch-xyz");
        assert!(result.is_ok(), "deleting nonexistent branch should succeed");
    }
}
