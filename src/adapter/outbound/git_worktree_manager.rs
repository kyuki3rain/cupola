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
    fn fetch(&self) -> Result<()> {
        self.run_git(&["fetch", "origin"])
            .context("failed to fetch from origin")
    }

    fn merge(&self, worktree_path: &Path, branch: &str) -> Result<()> {
        let remote_branch = format!("origin/{branch}");
        self.run_git_in_dir(worktree_path, &["merge", "--no-edit", &remote_branch])
            .with_context(|| format!("failed to merge {remote_branch}"))
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create(&self, path: &Path, branch: &str, start_point: &str) -> Result<()> {
        if self.exists(path) {
            tracing::info!(path = %path.display(), "worktree already exists, skipping create");
            return Ok(());
        }

        let path_str = path.to_string_lossy();
        self.run_git(&["worktree", "add", "-b", branch, &path_str, start_point])
            .with_context(|| format!("failed to create worktree at {}", path.display()))
    }

    fn remove(&self, path: &Path) -> Result<()> {
        if !self.exists(path) {
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

    fn init_git_repo(dir: &std::path::Path) {
        assert!(
            Command::new("git")
                .args(["init"])
                .current_dir(dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.email", "test@example.com"])
                .current_dir(dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.name", "Test"])
                .current_dir(dir)
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn fetch_succeeds_with_valid_origin() {
        let origin_dir = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();

        // Init bare origin repo
        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .current_dir(origin_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // Init local repo and add origin remote
        init_git_repo(repo_dir.path());
        assert!(
            Command::new("git")
                .args([
                    "remote",
                    "add",
                    "origin",
                    origin_dir.path().to_str().unwrap(),
                ])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );

        let mgr = GitWorktreeManager::new(repo_dir.path());
        let result = mgr.fetch();
        assert!(
            result.is_ok(),
            "fetch should succeed with valid origin: {result:?}"
        );
    }

    #[test]
    fn fetch_fails_without_origin_has_context() {
        let repo_dir = tempfile::tempdir().unwrap();
        init_git_repo(repo_dir.path());

        let mgr = GitWorktreeManager::new(repo_dir.path());
        let result = mgr.fetch();
        assert!(
            result.is_err(),
            "fetch should fail when origin is not configured"
        );
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(
            err_msg.contains("failed to fetch from origin"),
            "error should contain context message, got: {err_msg}"
        );
    }

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

    /// Set the initial branch name to "main" before any commits are made.
    fn set_initial_branch_main(dir: &std::path::Path) {
        // git symbolic-ref changes HEAD to point to refs/heads/main so the first
        // commit creates the "main" branch regardless of git's default config.
        let _ = Command::new("git")
            .args(["symbolic-ref", "HEAD", "refs/heads/main"])
            .current_dir(dir)
            .status();
    }

    fn make_commit(dir: &std::path::Path, filename: &str, content: &str, msg: &str) {
        let path = dir.join(filename);
        std::fs::write(&path, content).unwrap();
        assert!(
            Command::new("git")
                .args(["add", filename])
                .current_dir(dir)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["commit", "-m", msg])
                .current_dir(dir)
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn merge_clean_merge_succeeds() {
        // Set up a bare origin repo
        let origin_dir = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .current_dir(origin_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // Set up local repo with initial commit and push to origin
        let repo_dir = tempfile::tempdir().unwrap();
        init_git_repo(repo_dir.path());
        set_initial_branch_main(repo_dir.path());
        assert!(
            Command::new("git")
                .args([
                    "remote",
                    "add",
                    "origin",
                    origin_dir.path().to_str().unwrap(),
                ])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );
        make_commit(repo_dir.path(), "base.txt", "base", "init");
        assert!(
            Command::new("git")
                .args(["push", "-u", "origin", "main"])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // Create a feature branch from main
        assert!(
            Command::new("git")
                .args(["checkout", "-b", "feature"])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );
        make_commit(repo_dir.path(), "feature.txt", "feature change", "feat");

        // Add another commit to main on origin (simulate another PR merged)
        let other_dir = tempfile::tempdir().unwrap();
        init_git_repo(other_dir.path());
        assert!(
            Command::new("git")
                .args([
                    "remote",
                    "add",
                    "origin",
                    origin_dir.path().to_str().unwrap(),
                ])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["fetch", "origin"])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["checkout", "-b", "main", "origin/main"])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );
        make_commit(other_dir.path(), "other.txt", "other change", "other");
        assert!(
            Command::new("git")
                .args(["push", "origin", "main"])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // Fetch origin updates in repo_dir
        assert!(
            Command::new("git")
                .args(["fetch", "origin"])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // merge() should succeed (clean merge, different files)
        let mgr = GitWorktreeManager::new(repo_dir.path());
        let result = mgr.merge(repo_dir.path(), "main");
        assert!(result.is_ok(), "clean merge should succeed: {result:?}");
    }

    #[test]
    fn merge_conflict_returns_err() {
        // Set up a bare origin repo
        let origin_dir = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .current_dir(origin_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // Set up local repo with initial commit
        let repo_dir = tempfile::tempdir().unwrap();
        init_git_repo(repo_dir.path());
        set_initial_branch_main(repo_dir.path());
        assert!(
            Command::new("git")
                .args([
                    "remote",
                    "add",
                    "origin",
                    origin_dir.path().to_str().unwrap(),
                ])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );
        make_commit(repo_dir.path(), "conflict.txt", "original content", "init");
        assert!(
            Command::new("git")
                .args(["push", "-u", "origin", "main"])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // Create feature branch with conflicting change
        assert!(
            Command::new("git")
                .args(["checkout", "-b", "feature"])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );
        make_commit(repo_dir.path(), "conflict.txt", "feature content", "feat");

        // Push conflicting commit to origin/main from another clone
        let other_dir = tempfile::tempdir().unwrap();
        init_git_repo(other_dir.path());
        assert!(
            Command::new("git")
                .args([
                    "remote",
                    "add",
                    "origin",
                    origin_dir.path().to_str().unwrap(),
                ])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["fetch", "origin"])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["checkout", "-b", "main", "origin/main"])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );
        make_commit(
            other_dir.path(),
            "conflict.txt",
            "main content (different)",
            "main change",
        );
        assert!(
            Command::new("git")
                .args(["push", "origin", "main"])
                .current_dir(other_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // Fetch origin updates in repo_dir
        assert!(
            Command::new("git")
                .args(["fetch", "origin"])
                .current_dir(repo_dir.path())
                .status()
                .unwrap()
                .success()
        );

        // merge() should fail due to conflict
        let mgr = GitWorktreeManager::new(repo_dir.path());
        let result = mgr.merge(repo_dir.path(), "main");
        assert!(result.is_err(), "conflicting merge should return Err");
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(
            err_msg.contains("failed to merge origin/main"),
            "error should contain context message, got: {err_msg}"
        );
    }
}
