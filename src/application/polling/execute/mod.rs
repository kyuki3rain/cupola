/// Execute phase: run effects decided by Decide.
///
/// Effects are executed in priority order. Non-best-effort effect failures abort
/// the chain. Best-effort failures are logged and the chain continues.
use std::time::Duration;

use anyhow::Result;
use sha2::Digest;

use crate::application::init_task_manager::InitTaskManager;
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::file_generator::FileGenerator;
use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::application::session_manager::SessionManager;
use crate::domain::config::Config;
use crate::domain::effect::Effect;
use crate::domain::issue::Issue;

mod close_executor;
mod comment_executor;
mod context;
mod dispatcher;
mod shared;
mod spawn_init_executor;
mod spawn_process_executor;
mod worktree_executor;

use context::ExecuteContext;

/// Issue 本文が agent:ready 承認後に改変されたことを示すエラー。
#[derive(Debug, thiserror::Error)]
#[error("issue #{issue_number} body was modified after agent:ready approval")]
pub(crate) struct BodyTamperedError {
    pub issue_number: u64,
}

/// Issue 本文の SHA-256 hex ダイジェストを計算する。
fn sha256_hex(text: &str) -> String {
    let result = sha2::Sha256::digest(text.as_bytes());
    format!("{result:x}")
}

/// Backoff delays for DB update retries: [1s, 2s, 4s] — 3 retries after the
/// initial attempt (4 total calls maximum).
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// Retry a DB update closure with exponential backoff on transient failures.
///
/// Calls `db_update()` up to `RETRY_DELAYS.len() + 1` times. On each failure
/// (except the last), logs a `warn!` and sleeps the corresponding backoff
/// duration. After all retries are exhausted, logs an `error!` and returns
/// the last error.
async fn retry_db_update<F, Fut>(
    issue_number: u64,
    effect_name: &str,
    db_update: F,
) -> anyhow::Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    for (attempt, &delay) in RETRY_DELAYS.iter().enumerate() {
        match db_update().await {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::warn!(
                    issue_number,
                    effect = effect_name,
                    attempt = attempt + 1,
                    error = %e,
                    "DB update failed, retrying after backoff"
                );
                tokio::time::sleep(delay).await;
            }
        }
    }
    match db_update().await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!(
                issue_number,
                effect = effect_name,
                max_attempts = RETRY_DELAYS.len() + 1,
                error = %e,
                "DB update permanently failed after all retries"
            );
            Err(e)
        }
    }
}

/// Bound used anywhere SpawnInit needs to hand the worktree to a `tokio::spawn`
/// task. `GitWorktreeManager` is a trivial wrapper around a `PathBuf` so `Clone`
/// is cheap; `'static` is required to move the clone across the task boundary.
pub trait SpawnableGitWorktree: GitWorktree + Clone + 'static {}
impl<T: GitWorktree + Clone + 'static> SpawnableGitWorktree for T {}

/// Execute all effects for one issue.
///
/// Effects are processed in priority order (already sorted by `Decision::new`).
/// A non-best-effort failure aborts the rest. Best-effort failures are logged only.
#[expect(clippy::too_many_arguments)]
pub async fn execute_effects<G, I, P, C, W, F>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    claude_runner: &C,
    worktree: &W,
    file_gen: &F,
    session_mgr: &mut SessionManager,
    init_mgr: &mut InitTaskManager,
    config: &Config,
    issue: &mut Issue,
    effects: &[Effect],
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    let mut ctx = ExecuteContext {
        github,
        issue_repo,
        process_repo,
        claude_runner,
        worktree,
        file_gen,
        session_mgr,
        init_mgr,
        config,
    };

    for effect in effects {
        let best_effort = effect.is_best_effort();
        let result = dispatcher::dispatch(&mut ctx, issue, effect).await;

        match result {
            Ok(()) => {}
            Err(e) => {
                if best_effort {
                    tracing::warn!(
                        issue_number = issue.github_issue_number,
                        effect = ?effect,
                        error = %e,
                        "best-effort effect failed, continuing"
                    );
                } else {
                    tracing::error!(
                        issue_number = issue.github_issue_number,
                        effect = ?effect,
                        error = %e,
                        "effect failed, aborting remaining effects for this issue"
                    );
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use anyhow::Result;

    use super::{RETRY_DELAYS, sha256_hex};
    use crate::application::port::file_generator::FileGenerator;
    use crate::application::port::git_worktree::GitWorktree;
    use crate::application::port::github_client::{
        GitHubIssueDetail, GitHubPr, GitHubPrDetails, RepositoryPermission, ReviewThread,
    };
    use crate::application::port::process_run_repository::ProcessRunRepository;
    use crate::domain::config::Config;
    use crate::domain::effect::Effect;
    use crate::domain::fixing_problem_kind::FixingProblemKind;
    use crate::domain::issue::Issue;
    use crate::domain::process_run::ProcessRunType;
    use crate::domain::process_run::{ProcessRun, ProcessRunState};

    // ── sha256_hex unit tests ────────────────────────────────────────────────

    /// T-2.1.1: sha256_hex は同一入力に対して同一出力を返す
    #[test]
    fn sha256_hex_is_deterministic() {
        let h1 = sha256_hex("hello world");
        let h2 = sha256_hex("hello world");
        assert_eq!(h1, h2);
    }

    /// T-2.1.2: sha256_hex は空文字列に対して既知の SHA-256 値を返す
    #[test]
    fn sha256_hex_empty_string_known_value() {
        let h = sha256_hex("");
        assert_eq!(
            h,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// T-2.2: BodyTamperedError の Display メッセージに issue_number が含まれる
    #[test]
    fn body_tampered_error_display_contains_issue_number() {
        let err = super::BodyTamperedError { issue_number: 42 };
        let msg = err.to_string();
        assert!(
            msg.contains("42"),
            "error message should contain issue number, got: {msg}"
        );
    }

    type CallLog = Arc<Mutex<Vec<String>>>;

    #[derive(Clone, Default)]
    struct MockGitWorktree {
        calls: CallLog,
        worktree_exists: bool,
        fail_fetch: bool,
    }

    impl MockGitWorktree {
        fn with_log(calls: CallLog) -> Self {
            Self {
                calls,
                worktree_exists: false,
                fail_fetch: false,
            }
        }
    }

    impl GitWorktree for MockGitWorktree {
        fn fetch(&self) -> Result<()> {
            self.calls.lock().expect("lock").push("fetch".to_string());
            if self.fail_fetch {
                return Err(anyhow::anyhow!("simulated fetch failure"));
            }
            Ok(())
        }

        fn merge(&self, _p: &Path, _b: &str) -> Result<()> {
            self.calls.lock().expect("lock").push("merge".to_string());
            Ok(())
        }

        fn exists(&self, _p: &Path) -> bool {
            self.worktree_exists
        }

        fn create(&self, p: &Path, b: &str, s: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock")
                .push(format!("create:{}:{}:{}", p.display(), b, s));
            Ok(())
        }

        fn remove(&self, _p: &Path) -> Result<()> {
            Ok(())
        }

        fn create_branch(&self, p: &Path, b: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock")
                .push(format!("create_branch:{}:{}", p.display(), b));
            Ok(())
        }

        fn checkout(&self, _p: &Path, _b: &str) -> Result<()> {
            Ok(())
        }

        fn pull(&self, _p: &Path) -> Result<()> {
            Ok(())
        }

        fn push(&self, p: &Path, b: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock")
                .push(format!("push:{}:{}", p.display(), b));
            Ok(())
        }

        fn delete_branch(&self, _b: &str) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct MockFileGenerator {
        calls: CallLog,
    }

    impl MockFileGenerator {
        fn with_log(calls: CallLog) -> Self {
            Self { calls }
        }
    }

    impl FileGenerator for MockFileGenerator {
        fn generate_toml_template(&self) -> Result<bool> {
            Ok(false)
        }

        fn install_claude_code_assets(&self, _upgrade: bool) -> Result<bool> {
            Ok(false)
        }

        fn append_gitignore_entries(&self, _upgrade: bool) -> Result<bool> {
            Ok(false)
        }

        fn generate_spec_directory(
            &self,
            _issue_number: u64,
            _issue_body: &str,
            _language: &str,
        ) -> Result<bool> {
            Ok(false)
        }

        fn generate_spec_directory_at(
            &self,
            base_dir: &Path,
            issue_number: u64,
            issue_body: &str,
            language: &str,
        ) -> Result<bool> {
            self.calls.lock().expect("lock").push(format!(
                "generate_spec_directory_at:{}:{}:{}:{}",
                base_dir.display(),
                issue_number,
                issue_body,
                language,
            ));
            Ok(true)
        }

        fn write_claude_settings(
            &self,
            _settings: &crate::domain::claude_settings::ClaudeSettings,
        ) -> Result<bool> {
            Ok(false)
        }
    }

    /// T-3.EX.1: effects are executed in priority order
    #[test]
    fn effect_priority_order() {
        let mut effects = [
            Effect::CloseIssue,
            Effect::PostCompletedComment,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
            },
        ];
        effects.sort_by_key(|e| e.priority());
        assert_eq!(effects[0], Effect::PostCompletedComment);
        assert!(matches!(effects[1], Effect::SpawnProcess { .. }));
        assert_eq!(effects[2], Effect::CloseIssue);
    }

    /// T-3.EX.2: best-effort effects are correctly identified
    #[test]
    fn best_effort_classification() {
        assert!(Effect::PostCompletedComment.is_best_effort());
        assert!(Effect::PostCancelComment.is_best_effort());
        assert!(
            Effect::PostRetryExhaustedComment {
                process_type: ProcessRunType::Design,
                consecutive_failures: 3,
            }
            .is_best_effort()
        );
        assert!(Effect::RejectUntrustedReadyIssue.is_best_effort());
        assert!(Effect::PostCiFixLimitComment.is_best_effort());
        assert!(Effect::CleanupWorktree.is_best_effort());
        assert!(Effect::CloseIssue.is_best_effort());

        assert!(!Effect::SpawnInit.is_best_effort());
        assert!(!Effect::SwitchToImplBranch.is_best_effort());
        assert!(
            !Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
                pending_run_id: None,
            }
            .is_best_effort()
        );
    }

    /// T-3.EX.3: FixingProblemKind causes are passed through
    #[test]
    fn fixing_causes_in_spawn_process() {
        let causes = vec![FixingProblemKind::CiFailure, FixingProblemKind::Conflict];
        let effect = Effect::SpawnProcess {
            type_: ProcessRunType::DesignFix,
            causes: causes.clone(),
            pending_run_id: None,
        };
        if let Effect::SpawnProcess {
            type_: t,
            causes: c,
            ..
        } = effect
        {
            assert_eq!(t, ProcessRunType::DesignFix);
            assert_eq!(c, causes);
        } else {
            panic!("expected SpawnProcess");
        }
    }

    #[derive(Clone)]
    struct MockGitWorktreeForCleanup {
        deleted_branches: Arc<Mutex<Vec<String>>>,
        fail_delete_branch: bool,
    }

    impl MockGitWorktreeForCleanup {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let deleted = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    deleted_branches: deleted.clone(),
                    fail_delete_branch: false,
                },
                deleted,
            )
        }

        fn with_failing_delete(mut self) -> Self {
            self.fail_delete_branch = true;
            self
        }
    }

    impl GitWorktree for MockGitWorktreeForCleanup {
        fn fetch(&self) -> Result<()> {
            Ok(())
        }
        fn merge(&self, _: &Path, _: &str) -> Result<()> {
            Ok(())
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
        fn create(&self, _: &Path, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
        fn remove(&self, _: &Path) -> Result<()> {
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
            if self.fail_delete_branch {
                return Err(anyhow::anyhow!("delete_branch failed"));
            }
            self.deleted_branches
                .lock()
                .expect("lock")
                .push(branch.to_string());
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MockIssueRepoForCleanup {
        metadata_updates: Arc<Mutex<Vec<crate::domain::metadata_update::MetadataUpdates>>>,
    }

    impl MockIssueRepoForCleanup {
        fn new() -> (
            Self,
            Arc<Mutex<Vec<crate::domain::metadata_update::MetadataUpdates>>>,
        ) {
            let updates = Arc::new(Mutex::new(vec![]));
            (
                Self {
                    metadata_updates: updates.clone(),
                },
                updates,
            )
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepoForCleanup {
        async fn find_by_id(&self, _: i64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_by_issue_number(&self, _: u64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn find_all(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn save(&self, _: &Issue) -> Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, _: i64, _: crate::domain::state::State) -> Result<()> {
            unimplemented!()
        }
        async fn update(&self, _: &Issue) -> Result<()> {
            unimplemented!()
        }
        async fn update_state_and_metadata(
            &self,
            _: i64,
            updates: &crate::domain::metadata_update::MetadataUpdates,
        ) -> Result<()> {
            self.metadata_updates
                .lock()
                .expect("lock")
                .push(updates.clone());
            Ok(())
        }
        async fn find_by_state(&self, _: crate::domain::state::State) -> Result<Vec<Issue>> {
            unimplemented!()
        }
    }

    struct MockClaudeRunner;

    impl crate::application::port::claude_code_runner::ClaudeCodeRunner for MockClaudeRunner {
        fn spawn(
            &self,
            _: &str,
            _: &Path,
            _: Option<&str>,
            _: &str,
        ) -> Result<std::process::Child> {
            unimplemented!()
        }
    }

    #[derive(Clone)]
    struct MockGitHubForInit {
        comments: Arc<Mutex<Vec<String>>>,
        fail_comment: bool,
    }

    impl MockGitHubForInit {
        fn new() -> Self {
            Self {
                comments: Arc::new(Mutex::new(vec![])),
                fail_comment: false,
            }
        }

        fn with_failing_comment(mut self) -> Self {
            self.fail_comment = true;
            self
        }

        fn posted_comments(&self) -> Vec<String> {
            self.comments.lock().expect("lock").clone()
        }
    }

    impl crate::application::port::github_client::GitHubClient for MockGitHubForInit {
        async fn get_issue(&self, number: u64) -> anyhow::Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number,
                title: "Test Issue".to_string(),
                body: "Issue body".to_string(),
                labels: vec![],
            })
        }

        async fn comment_on_issue(&self, _: u64, body: &str) -> anyhow::Result<()> {
            if self.fail_comment {
                return Err(anyhow::anyhow!("comment API error"));
            }
            self.comments.lock().expect("lock").push(body.to_string());
            Ok(())
        }

        async fn list_open_issues(
            &self,
        ) -> anyhow::Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }

        async fn find_pr_by_branches(&self, _: &str, _: &str) -> anyhow::Result<Option<GitHubPr>> {
            Ok(None)
        }

        async fn is_pr_merged(&self, _: u64) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn list_unresolved_threads(&self, _: u64) -> anyhow::Result<Vec<ReviewThread>> {
            Ok(vec![])
        }

        async fn create_pr(&self, _: &str, _: &str, _: &str, _: &str) -> anyhow::Result<u64> {
            Ok(0)
        }

        async fn reply_to_thread(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn resolve_thread(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn close_issue(&self, _: u64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_job_logs(&self, _: u64) -> anyhow::Result<String> {
            Ok(String::new())
        }

        async fn get_pr_details(&self, _: u64) -> anyhow::Result<GitHubPrDetails> {
            unimplemented!()
        }

        async fn fetch_label_actor_login(&self, _: u64, _: &str) -> anyhow::Result<Option<String>> {
            Ok(None)
        }

        async fn fetch_user_permission(&self, _: &str) -> anyhow::Result<RepositoryPermission> {
            Ok(RepositoryPermission::Read)
        }

        async fn remove_label(&self, _: u64, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn observe_pr(
            &self,
            _: u64,
        ) -> anyhow::Result<Option<crate::application::port::github_client::PrObservation>>
        {
            Ok(None)
        }
    }

    type FailedRunLog = Arc<Mutex<Vec<(i64, Option<String>)>>>;

    #[derive(Clone)]
    struct MockProcRepo {
        mark_failed_calls: FailedRunLog,
    }

    impl MockProcRepo {
        fn new() -> Self {
            Self {
                mark_failed_calls: Arc::new(Mutex::new(vec![])),
            }
        }
    }

    impl ProcessRunRepository for MockProcRepo {
        async fn save(&self, _: &ProcessRun) -> anyhow::Result<i64> {
            Ok(42)
        }

        async fn find_latest(
            &self,
            _: i64,
            _: ProcessRunType,
        ) -> anyhow::Result<Option<ProcessRun>> {
            Ok(None)
        }

        async fn find_latest_with_pr_number(
            &self,
            _: i64,
            _: ProcessRunType,
        ) -> anyhow::Result<Option<ProcessRun>> {
            Ok(None)
        }

        async fn mark_failed(&self, run_id: i64, msg: Option<String>) -> anyhow::Result<()> {
            self.mark_failed_calls
                .lock()
                .expect("lock")
                .push((run_id, msg));
            Ok(())
        }

        async fn update_pid(&self, _: i64, _: u32) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_succeeded(&self, _: i64, _: Option<u64>) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_stale(&self, _: i64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn mark_stale_for_issue(&self, _: i64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn find_by_issue(&self, _: i64) -> anyhow::Result<Vec<ProcessRun>> {
            Ok(vec![])
        }

        async fn count_consecutive_failures(
            &self,
            _: i64,
            _: ProcessRunType,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> anyhow::Result<u32> {
            Ok(0)
        }

        async fn find_latest_with_consecutive_count(
            &self,
            _: i64,
            _: ProcessRunType,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> anyhow::Result<Option<(ProcessRun, u32)>> {
            Ok(None)
        }

        async fn find_all_running(&self) -> anyhow::Result<Vec<ProcessRun>> {
            Ok(vec![])
        }

        async fn update_state(&self, _: i64, _: ProcessRunState) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn make_test_issue(feature_name: &str) -> Issue {
        let mut issue = Issue::new(1, feature_name.to_string());
        issue.id = 1;
        issue
    }

    fn make_test_config(worktrees_parent: &std::path::Path) -> Config {
        let mut config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        config.log_dir = worktrees_parent.join("logs").join("cupola.log");
        config
    }

    fn make_completed_issue_with_worktree(feature_name: &str, wt_path: &str) -> Issue {
        Issue {
            id: 1,
            github_issue_number: 42,
            state: crate::domain::state::State::Completed,
            feature_name: feature_name.to_string(),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            worktree_path: Some(wt_path.to_string()),
            ci_fix_count: 0,
            ci_fix_limit_notified: false,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            body_hash: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// T-3.CW.1: CleanupWorktree deletes both branches after successful worktree removal
    #[tokio::test]
    async fn cleanup_worktree_deletes_both_branches() {
        let tmp = tempfile::tempdir().unwrap();
        let wt_path = tmp.path().to_string_lossy().to_string();
        let mut issue = make_completed_issue_with_worktree("issue-190", &wt_path);

        let (worktree, deleted) = MockGitWorktreeForCleanup::new();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let github = MockGitHubForInit::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        let effects = [Effect::CleanupWorktree];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed");

        let branches = deleted.lock().expect("lock");
        assert!(
            branches.contains(&"cupola/issue-190/main".to_string()),
            "should delete main branch"
        );
        assert!(
            branches.contains(&"cupola/issue-190/design".to_string()),
            "should delete design branch"
        );

        let updates = metadata_updates.lock().expect("lock");
        assert_eq!(updates.len(), 1, "should update metadata once");
        assert!(
            updates[0].worktree_path == Some(None),
            "should clear worktree_path"
        );
        assert!(issue.worktree_path.is_none(), "issue.worktree_path cleared");
    }

    /// T-3.CW.2: CleanupWorktree continues and clears metadata even when delete_branch fails
    #[tokio::test]
    async fn cleanup_worktree_continues_on_delete_branch_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let wt_path = tmp.path().to_string_lossy().to_string();
        let mut issue = make_completed_issue_with_worktree("issue-190", &wt_path);

        let (worktree, _deleted) = MockGitWorktreeForCleanup::new();
        let worktree = worktree.with_failing_delete();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let github = MockGitHubForInit::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        let effects = [Effect::CleanupWorktree];
        let result = super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await;

        assert!(result.is_ok(), "should not fail when delete_branch fails");
        let updates = metadata_updates.lock().expect("lock");
        assert_eq!(
            updates.len(),
            1,
            "should still clear metadata despite branch deletion failure"
        );
    }

    /// T-3.CW.3: CleanupWorktree skips branch deletion when worktree_path is None
    #[tokio::test]
    async fn cleanup_worktree_skips_branch_deletion_when_no_worktree_path() {
        let mut issue = Issue {
            id: 1,
            github_issue_number: 42,
            state: crate::domain::state::State::Completed,
            feature_name: "issue-190".to_string(),
            weight: crate::domain::task_weight::TaskWeight::Medium,
            worktree_path: None,
            ci_fix_count: 0,
            ci_fix_limit_notified: false,
            close_finished: false,
            consecutive_failures_epoch: None,
            last_pr_review_submitted_at: None,
            body_hash: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let (worktree, deleted) = MockGitWorktreeForCleanup::new();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let github = MockGitHubForInit::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        let effects = [Effect::CleanupWorktree];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed");

        assert!(
            deleted.lock().expect("lock").is_empty(),
            "should not call delete_branch when worktree_path is None"
        );
        assert!(
            metadata_updates.lock().expect("lock").is_empty(),
            "should not update metadata when worktree_path is None"
        );
    }

    // ── Tests for PostRetryExhaustedComment payload usage ────────────────────

    /// T-3.EX.re.1: PostRetryExhaustedComment uses consecutive_failures from payload as count
    #[tokio::test]
    async fn retry_exhausted_comment_uses_payload_consecutive_failures() {
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let (issue_repo, _) = MockIssueRepoForCleanup::new();
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-1");

        let effects = [Effect::PostRetryExhaustedComment {
            process_type: ProcessRunType::Design,
            consecutive_failures: 3,
        }];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed");

        let comments = github.posted_comments();
        assert_eq!(comments.len(), 1, "should post exactly one comment");
        assert!(
            comments[0].contains("（3 回）"),
            "comment should contain the consecutive_failures count formatted as '（3 回）', got: {:?}",
            comments[0]
        );
    }

    // ── Mock IssueRepository for retry tests ────────────────────────────────

    #[derive(Clone)]
    struct MockIssueRepoForRetry {
        call_count: Arc<Mutex<u32>>,
        max_failures: u32,
    }

    impl MockIssueRepoForRetry {
        fn fail_then_succeed(max_failures: u32) -> Self {
            Self {
                call_count: Arc::new(Mutex::new(0)),
                max_failures,
            }
        }

        fn always_fail() -> Self {
            Self::fail_then_succeed(u32::MAX)
        }

        fn call_count(&self) -> u32 {
            *self.call_count.lock().expect("lock")
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepoForRetry {
        async fn find_by_id(&self, _: i64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_by_issue_number(&self, _: u64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn find_all(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn save(&self, _: &Issue) -> Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, _: i64, _: crate::domain::state::State) -> Result<()> {
            unimplemented!()
        }
        async fn update(&self, _: &Issue) -> Result<()> {
            unimplemented!()
        }
        async fn update_state_and_metadata(
            &self,
            _: i64,
            _: &crate::domain::metadata_update::MetadataUpdates,
        ) -> Result<()> {
            let mut count = self.call_count.lock().expect("lock");
            *count += 1;
            let n = *count;
            if n <= self.max_failures {
                Err(anyhow::anyhow!("simulated DB failure (call {})", n))
            } else {
                Ok(())
            }
        }
        async fn find_by_state(&self, _: crate::domain::state::State) -> Result<Vec<Issue>> {
            unimplemented!()
        }
    }

    // ── retry_db_update unit tests ───────────────────────────────────────────

    /// T-4.1: 一時的失敗から回復するケース — 1回失敗後に成功する
    #[tokio::test(start_paused = true)]
    async fn retry_db_update_recovers_from_transient_failure() {
        let call_count = Arc::new(Mutex::new(0u32));
        let cc = call_count.clone();

        let result = super::retry_db_update(42, "TestEffect", || {
            let mut n = cc.lock().expect("lock");
            *n += 1;
            let attempt = *n;
            async move {
                if attempt == 1 {
                    Err(anyhow::anyhow!("transient failure"))
                } else {
                    Ok(())
                }
            }
        })
        .await;

        assert!(result.is_ok(), "should succeed after one transient failure");
        assert_eq!(
            *call_count.lock().expect("lock"),
            2,
            "should call db_update exactly twice (1 failure + 1 success)"
        );
    }

    /// T-4.2: 回復不能な失敗のケース — 全試行で失敗する
    #[tokio::test(start_paused = true)]
    async fn retry_db_update_exhausts_all_retries_and_returns_error() {
        let call_count = Arc::new(Mutex::new(0u32));
        let cc = call_count.clone();

        let result = super::retry_db_update(42, "TestEffect", || {
            let mut n = cc.lock().expect("lock");
            *n += 1;
            async move { Err::<(), _>(anyhow::anyhow!("permanent failure")) }
        })
        .await;

        assert!(
            result.is_err(),
            "should return error after all retries exhausted"
        );
        assert_eq!(
            *call_count.lock().expect("lock"),
            RETRY_DELAYS.len() as u32 + 1,
            "should call db_update exactly RETRY_DELAYS.len()+1 times"
        );
    }

    // ── CloseIssue retry integration tests ─────────────────────────────────

    /// T-3.1: CloseIssue で DB 更新が 1 回失敗後に成功したとき close_finished=true になる
    #[tokio::test(start_paused = true)]
    async fn close_issue_sets_close_finished_after_transient_db_failure() {
        let issue_repo = MockIssueRepoForRetry::fail_then_succeed(1);
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-42");

        let effects = [Effect::CloseIssue];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed (CloseIssue is best-effort)");

        assert!(
            issue.close_finished,
            "close_finished should be true after successful retry"
        );
        assert_eq!(
            issue_repo.call_count(),
            2,
            "update_state_and_metadata should be called twice (1 failure + 1 success)"
        );
    }

    /// T-3.4 (CloseIssue): DB 更新が永続的に失敗したとき close_finished は変更されない
    #[tokio::test(start_paused = true)]
    async fn close_issue_does_not_set_close_finished_on_permanent_db_failure() {
        let issue_repo = MockIssueRepoForRetry::always_fail();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-42");

        let effects = [Effect::CloseIssue];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects returns Ok for best-effort effect");

        assert!(
            !issue.close_finished,
            "close_finished must remain false when DB update permanently fails"
        );
        assert_eq!(
            issue_repo.call_count(),
            RETRY_DELAYS.len() as u32 + 1,
            "should exhaust all retries"
        );
    }

    // ── Tests for PostCiFixLimitComment ──────────────────────────────────────

    /// T-3.CFL.1: comment 成功時に ci_fix_limit_notified=true が永続化される
    #[tokio::test]
    async fn post_ci_fix_limit_comment_persists_notified_on_success() {
        let github = MockGitHubForInit::new();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-1");

        let effects = [Effect::PostCiFixLimitComment];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed");

        let updates = metadata_updates.lock().expect("lock");
        assert_eq!(
            updates.len(),
            1,
            "should call update_state_and_metadata once"
        );
        assert_eq!(
            updates[0].ci_fix_limit_notified,
            Some(true),
            "should persist ci_fix_limit_notified=true"
        );
    }

    /// T-3.CFL.2: comment 失敗時に永続化が呼ばれない（warn のみ）
    #[tokio::test]
    async fn post_ci_fix_limit_comment_skips_persist_on_comment_failure() {
        let github = MockGitHubForInit::new().with_failing_comment();
        let (issue_repo, metadata_updates) = MockIssueRepoForCleanup::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        let mut issue = make_test_issue("issue-1");

        let effects = [Effect::PostCiFixLimitComment];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects should succeed (PostCiFixLimitComment is best-effort)");

        let updates = metadata_updates.lock().expect("lock");
        assert!(
            updates.is_empty(),
            "should not call update_state_and_metadata when comment fails"
        );
    }

    // ── CleanupWorktree retry integration tests ─────────────────────────────

    /// T-3.2 / T-3.4 (CleanupWorktree): DB 更新が永続的に失敗したとき worktree_path は変更されない
    #[tokio::test(start_paused = true)]
    async fn cleanup_worktree_does_not_clear_worktree_path_on_permanent_db_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wt_path_str = tmp.path().to_string_lossy().to_string();
        let mut issue = make_completed_issue_with_worktree("issue-42", &wt_path_str);

        let issue_repo = MockIssueRepoForRetry::always_fail();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktreeForCleanup::new().0;
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());

        let effects = [Effect::CleanupWorktree];
        super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await
        .expect("execute_effects returns Ok for best-effort effect");

        assert!(
            issue.worktree_path.is_some(),
            "worktree_path must remain Some when DB update permanently fails"
        );
        assert_eq!(
            issue_repo.call_count(),
            RETRY_DELAYS.len() as u32 + 1,
            "should exhaust all retries"
        );
    }

    // ── SpawnInit body_hash integration tests ────────────────────────────────

    #[derive(Clone)]
    struct MockGitHubWithBody {
        body: String,
        comments: Arc<Mutex<Vec<String>>>,
        removed_labels: Arc<Mutex<Vec<String>>>,
    }

    impl MockGitHubWithBody {
        fn new(body: impl Into<String>) -> Self {
            Self {
                body: body.into(),
                comments: Arc::new(Mutex::new(vec![])),
                removed_labels: Arc::new(Mutex::new(vec![])),
            }
        }

        fn posted_comments(&self) -> Vec<String> {
            self.comments.lock().expect("lock").clone()
        }

        fn removed_labels(&self) -> Vec<String> {
            self.removed_labels.lock().expect("lock").clone()
        }
    }

    impl crate::application::port::github_client::GitHubClient for MockGitHubWithBody {
        async fn get_issue(&self, number: u64) -> anyhow::Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number,
                title: "Test Issue".to_string(),
                body: self.body.clone(),
                labels: vec![],
            })
        }
        async fn comment_on_issue(&self, _: u64, body: &str) -> anyhow::Result<()> {
            self.comments.lock().expect("lock").push(body.to_string());
            Ok(())
        }
        async fn list_open_issues(
            &self,
        ) -> anyhow::Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }
        async fn find_pr_by_branches(&self, _: &str, _: &str) -> anyhow::Result<Option<GitHubPr>> {
            Ok(None)
        }
        async fn is_pr_merged(&self, _: u64) -> anyhow::Result<bool> {
            Ok(false)
        }
        async fn list_unresolved_threads(&self, _: u64) -> anyhow::Result<Vec<ReviewThread>> {
            Ok(vec![])
        }
        async fn create_pr(&self, _: &str, _: &str, _: &str, _: &str) -> anyhow::Result<u64> {
            Ok(0)
        }
        async fn reply_to_thread(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn resolve_thread(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn close_issue(&self, _: u64) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_job_logs(&self, _: u64) -> anyhow::Result<String> {
            Ok(String::new())
        }
        async fn get_pr_details(&self, _: u64) -> anyhow::Result<GitHubPrDetails> {
            unimplemented!()
        }
        async fn fetch_label_actor_login(&self, _: u64, _: &str) -> anyhow::Result<Option<String>> {
            Ok(None)
        }
        async fn fetch_user_permission(&self, _: &str) -> anyhow::Result<RepositoryPermission> {
            Ok(RepositoryPermission::Read)
        }
        async fn remove_label(&self, _: u64, label: &str) -> anyhow::Result<()> {
            self.removed_labels
                .lock()
                .expect("lock")
                .push(label.to_string());
            Ok(())
        }
        async fn observe_pr(
            &self,
            _: u64,
        ) -> anyhow::Result<Option<crate::application::port::github_client::PrObservation>>
        {
            Ok(None)
        }
    }

    #[derive(Clone)]
    struct MockIssueRepoForHash {
        updates: Arc<Mutex<Vec<crate::domain::metadata_update::MetadataUpdates>>>,
        states: Arc<Mutex<Vec<crate::domain::state::State>>>,
    }

    impl MockIssueRepoForHash {
        fn new() -> Self {
            Self {
                updates: Arc::new(Mutex::new(vec![])),
                states: Arc::new(Mutex::new(vec![])),
            }
        }

        fn recorded_updates(&self) -> Vec<crate::domain::metadata_update::MetadataUpdates> {
            self.updates.lock().expect("lock").clone()
        }
    }

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepoForHash {
        async fn find_by_id(&self, _: i64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_by_issue_number(&self, _: u64) -> Result<Option<Issue>> {
            unimplemented!()
        }
        async fn find_active(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn find_all(&self) -> Result<Vec<Issue>> {
            unimplemented!()
        }
        async fn save(&self, _: &Issue) -> Result<i64> {
            unimplemented!()
        }
        async fn update_state(&self, _: i64, state: crate::domain::state::State) -> Result<()> {
            self.states.lock().expect("lock").push(state);
            Ok(())
        }
        async fn update(&self, _: &Issue) -> Result<()> {
            unimplemented!()
        }
        async fn update_state_and_metadata(
            &self,
            _: i64,
            updates: &crate::domain::metadata_update::MetadataUpdates,
        ) -> Result<()> {
            self.updates.lock().expect("lock").push(updates.clone());
            Ok(())
        }
        async fn find_by_state(&self, _: crate::domain::state::State) -> Result<Vec<Issue>> {
            unimplemented!()
        }
    }

    /// T-4.3.cancel: 改変検知時に Cancelled 遷移・ラベル削除・コメントが実行される
    #[tokio::test]
    async fn tamper_detection_transitions_to_cancelled_and_removes_label_and_posts_comment() {
        let dir = tempfile::tempdir().expect("tempdir");
        let github = MockGitHubWithBody::new("Tampered body");
        let issue_repo = MockIssueRepoForHash::new();
        let proc_repo = MockProcRepo::new();
        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log);
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let mut session_mgr = crate::application::session_manager::SessionManager::new();
        let mut init_mgr = crate::application::init_task_manager::InitTaskManager::new();
        let config = make_test_config(dir.path());
        let mut issue = make_test_issue("issue-1");
        issue.worktree_path = Some(dir.path().to_string_lossy().into_owned());
        issue.body_hash = Some(sha256_hex("Original approved body"));

        let effects = [Effect::SpawnProcess {
            type_: ProcessRunType::Design,
            causes: vec![],
            pending_run_id: None,
        }];

        let result = super::execute_effects(
            &github,
            &issue_repo,
            &proc_repo,
            &MockClaudeRunner,
            &worktree,
            &file_gen,
            &mut session_mgr,
            &mut init_mgr,
            &config,
            &mut issue,
            &effects,
        )
        .await;

        assert!(result.is_ok(), "tamper response should return Ok");
        assert_eq!(
            issue.state,
            crate::domain::state::State::Cancelled,
            "in-memory state should be Cancelled"
        );

        let removed = github.removed_labels();
        assert!(
            removed.contains(&"agent:ready".to_string()),
            "agent:ready label should be removed"
        );

        let comments = github.posted_comments();
        assert!(
            !comments.is_empty(),
            "at least one comment should be posted"
        );

        let updates = issue_repo.recorded_updates();
        let cancelled_update = updates
            .iter()
            .find(|u| u.state == Some(crate::domain::state::State::Cancelled));
        assert!(
            cancelled_update.is_some(),
            "should have a Cancelled state update in DB"
        );
    }
}
