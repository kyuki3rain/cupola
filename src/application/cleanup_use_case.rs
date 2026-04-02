use anyhow::Result;
use std::path::Path;

use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::issue_repository::IssueRepository;
use crate::domain::issue::Issue;
use crate::domain::state::State;

pub struct CleanupUseCase<I: IssueRepository, W: GitWorktree> {
    pub issue_repo: I,
    pub worktree: W,
}

pub struct CleanupResult {
    pub cleaned: Vec<CleanedIssue>,
}

pub struct CleanedIssue {
    pub issue_number: u64,
    pub worktree_removed: bool,
    pub branches_removed: Vec<String>,
}

impl<I: IssueRepository, W: GitWorktree> CleanupUseCase<I, W> {
    pub fn new(issue_repo: I, worktree: W) -> Self {
        Self {
            issue_repo,
            worktree,
        }
    }

    pub async fn execute(&self) -> Result<CleanupResult> {
        let cancelled_issues = self.issue_repo.find_by_state(State::Cancelled).await?;

        if cancelled_issues.is_empty() {
            return Ok(CleanupResult { cleaned: vec![] });
        }

        let mut cleaned = Vec::new();

        for issue in &cancelled_issues {
            let result = self.cleanup_issue(issue).await;
            match result {
                Ok(cleaned_issue) => cleaned.push(cleaned_issue),
                Err(e) => {
                    tracing::warn!(
                        issue_number = issue.github_issue_number,
                        error = %e,
                        "cleanup failed for issue, continuing with next"
                    );
                }
            }
        }

        Ok(CleanupResult { cleaned })
    }

    async fn cleanup_issue(&self, issue: &Issue) -> Result<CleanedIssue> {
        let n = issue.github_issue_number;
        let mut worktree_removed = false;
        let mut branches_removed = Vec::new();

        // 1. worktree の削除
        if let Some(ref wt_path) = issue.worktree_path {
            let path = Path::new(wt_path);
            if path.exists() {
                match self.worktree.remove(path) {
                    Ok(()) => {
                        tracing::info!(issue_number = n, path = %wt_path, "worktree removed");
                        worktree_removed = true;
                    }
                    Err(e) => {
                        tracing::warn!(issue_number = n, path = %wt_path, error = %e, "failed to remove worktree");
                    }
                }
            } else {
                tracing::info!(issue_number = n, path = %wt_path, "worktree path does not exist, skipping");
            }
        }

        // 2. ブランチの削除
        let main_branch = format!("cupola/{n}/main");
        let design_branch = format!("cupola/{n}/design");
        for branch in [&main_branch, &design_branch] {
            match self.worktree.delete_branch(branch) {
                Ok(()) => {
                    tracing::info!(issue_number = n, branch = %branch, "branch deleted");
                    branches_removed.push(branch.clone());
                }
                Err(e) => {
                    tracing::warn!(issue_number = n, branch = %branch, error = %e, "failed to delete branch, skipping");
                }
            }
        }

        // 3. DB フィールドの NULL 化
        // worktree が残ったまま参照だけ失うと復旧不能になるため、
        // worktree_path は worktree が存在しないことが確認できた場合にのみ消す。
        let can_cleanup_metadata = match &issue.worktree_path {
            None => true,
            Some(wt_path) => {
                let path = Path::new(wt_path);
                if !path.exists() {
                    true
                } else {
                    // パスが存在するので、削除に成功した場合のみクリーンアップする
                    worktree_removed
                }
            }
        };

        let mut updated = issue.clone();
        let mut need_update = false;

        if can_cleanup_metadata {
            if updated.worktree_path.take().is_some() {
                need_update = true;
            }
            if updated.design_pr_number.take().is_some() {
                need_update = true;
            }
            if updated.impl_pr_number.take().is_some() {
                need_update = true;
            }
            if updated.feature_name.take().is_some() {
                need_update = true;
            }
        } else {
            tracing::warn!(
                issue_number = n,
                "worktree removal failed; skipping metadata cleanup to avoid unrecoverable state"
            );
        }

        if need_update {
            self.issue_repo.update(&updated).await?;
        }

        Ok(CleanedIssue {
            issue_number: n,
            worktree_removed,
            branches_removed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::domain::state::State;

    // Mock IssueRepository
    struct MockIssueRepo {
        issues: Vec<Issue>,
        updated: Arc<Mutex<Vec<Issue>>>,
    }

    impl MockIssueRepo {
        fn new(issues: Vec<Issue>) -> (Self, Arc<Mutex<Vec<Issue>>>) {
            let updated = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    issues,
                    updated: updated.clone(),
                },
                updated,
            )
        }
    }

    impl IssueRepository for MockIssueRepo {
        async fn find_by_id(&self, _: i64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_by_issue_number(&self, _: u64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn find_needing_process(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn save(&self, _: &Issue) -> Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, _: i64, _: State) -> Result<()> {
            unimplemented!()
        }
        async fn update(&self, issue: &Issue) -> Result<()> {
            self.updated.lock().unwrap().push(issue.clone());
            Ok(())
        }
        async fn reset_for_restart(&self, _: i64) -> Result<()> {
            unimplemented!()
        }
        async fn find_by_state(&self, state: State) -> Result<Vec<Issue>> {
            Ok(self
                .issues
                .iter()
                .filter(|i| i.state == state)
                .cloned()
                .collect())
        }
    }

    // Mock GitWorktree
    struct MockWorktree {
        removed_paths: Arc<Mutex<Vec<String>>>,
        deleted_branches: Arc<Mutex<Vec<String>>>,
        fail_remove: bool,
    }

    impl MockWorktree {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<String>>>) {
            let removed = Arc::new(Mutex::new(vec![]));
            let deleted = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    removed_paths: removed.clone(),
                    deleted_branches: deleted.clone(),
                    fail_remove: false,
                },
                removed,
                deleted,
            )
        }
    }

    impl GitWorktree for MockWorktree {
        fn fetch(&self) -> Result<()> {
            Ok(())
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
        fn create(&self, _: &Path, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
        fn remove(&self, path: &Path) -> Result<()> {
            if self.fail_remove {
                return Err(anyhow::anyhow!("remove failed"));
            }
            self.removed_paths
                .lock()
                .unwrap()
                .push(path.to_string_lossy().to_string());
            Ok(())
        }
        fn create_branch(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn checkout(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn pull(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn push(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn delete_branch(&self, branch: &str) -> Result<()> {
            self.deleted_branches
                .lock()
                .unwrap()
                .push(branch.to_string());
            Ok(())
        }
    }

    fn cancelled_issue_with_wt(id: i64, issue_number: u64, wt: &str) -> Issue {
        Issue {
            id,
            github_issue_number: issue_number,
            state: State::Cancelled,
            design_pr_number: Some(10),
            impl_pr_number: Some(20),
            worktree_path: Some(wt.to_string()),
            retry_count: 3,
            current_pid: None,
            error_message: Some("error".to_string()),
            feature_name: Some("feature".to_string()),
            fixing_causes: vec![],
            model: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn cleanup_removes_worktree_and_branches_for_cancelled_issue() {
        // 実在するディレクトリを作成して worktree.remove() が呼ばれることを検証する
        let tmp = tempfile::tempdir().expect("tempdir");
        let wt_path = tmp.path().to_string_lossy().to_string();
        let issue = cancelled_issue_with_wt(1, 42, &wt_path);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let (worktree, removed, deleted) = MockWorktree::new();

        let uc = CleanupUseCase::new(repo, worktree);
        let result = uc.execute().await.expect("execute");

        assert_eq!(result.cleaned.len(), 1);
        assert_eq!(result.cleaned[0].issue_number, 42);
        assert!(result.cleaned[0].worktree_removed);

        // worktree.remove() が呼ばれていること
        let removed_paths = removed.lock().unwrap();
        assert!(removed_paths.contains(&wt_path));

        // Branches should be deleted
        let branches = deleted.lock().unwrap();
        assert!(branches.contains(&"cupola/42/main".to_string()));
        assert!(branches.contains(&"cupola/42/design".to_string()));

        // DB should be updated with NULL fields
        let updates = updated.lock().unwrap();
        assert_eq!(updates.len(), 1);
        assert!(updates[0].worktree_path.is_none());
        assert!(updates[0].design_pr_number.is_none());
        assert!(updates[0].impl_pr_number.is_none());
        assert!(updates[0].feature_name.is_none());
    }

    #[tokio::test]
    async fn cleanup_returns_empty_when_no_cancelled_issues() {
        let issue = Issue {
            id: 1,
            github_issue_number: 10,
            state: State::Completed,
            design_pr_number: None,
            impl_pr_number: None,
            worktree_path: None,
            retry_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: None,
            fixing_causes: vec![],
            model: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let (repo, _) = MockIssueRepo::new(vec![issue]);
        let (worktree, _, _) = MockWorktree::new();
        let uc = CleanupUseCase::new(repo, worktree);
        let result = uc.execute().await.expect("execute");
        assert!(result.cleaned.is_empty());
    }

    #[tokio::test]
    async fn cleanup_continues_on_worktree_remove_failure() {
        // 実在するパスを使い、fail_remove=true でworktree 削除を確実に失敗させる
        let tmp = tempfile::tempdir().expect("tempdir");
        let wt_path = tmp.path().to_string_lossy().to_string();
        let issue = cancelled_issue_with_wt(1, 42, &wt_path);
        let (repo, updated) = MockIssueRepo::new(vec![issue]);
        let (mut worktree, _, deleted) = MockWorktree::new();
        worktree.fail_remove = true;

        let uc = CleanupUseCase::new(repo, worktree);
        let result = uc.execute().await.expect("execute");

        // worktree 削除失敗でも処理は継続し、ブランチは削除される
        assert_eq!(result.cleaned.len(), 1);
        assert!(!result.cleaned[0].worktree_removed);
        assert_eq!(deleted.lock().unwrap().len(), 2);

        // worktree が残っているため、メタデータは更新されない
        assert_eq!(updated.lock().unwrap().len(), 0);
    }
}
