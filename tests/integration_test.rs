use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use cupola::adapter::outbound::sqlite_connection::SqliteConnection;
use cupola::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
use cupola::application::polling_use_case::PollingUseCase;
use cupola::application::port::claude_code_runner::ClaudeCodeRunner;
use cupola::application::port::execution_log_repository::ExecutionLogRepository;
use cupola::application::port::git_worktree::GitWorktree;
use cupola::application::port::github_client::{
    GitHubCheckRun, GitHubClient, GitHubIssue, GitHubIssueDetail, GitHubPr, GitHubPrDetails,
    ReviewThread,
};
use cupola::application::port::issue_repository::IssueRepository;
use cupola::application::transition_use_case::{TransitionUseCase, prioritize_events};
use cupola::domain::config::Config;
use cupola::domain::event::Event;
use cupola::domain::execution_log::ExecutionLog;
use cupola::domain::issue::Issue;
use cupola::domain::state::State;

// === Mock GitHub Client ===

#[derive(Default)]
struct MockGitHubState {
    comments: Vec<(u64, String)>,
    closed_issues: Vec<u64>,
    merged_prs: Vec<u64>,
    closed_github_issues: Vec<u64>,
}

struct MockGitHubClient {
    state: Arc<Mutex<MockGitHubState>>,
}

impl MockGitHubClient {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockGitHubState::default())),
        }
    }
}

impl GitHubClient for MockGitHubClient {
    async fn list_ready_issues(&self) -> Result<Vec<GitHubIssue>> {
        Ok(vec![])
    }
    async fn get_issue(&self, _n: u64) -> Result<GitHubIssueDetail> {
        Ok(GitHubIssueDetail {
            number: 1,
            title: "test".into(),
            body: "body".into(),
            labels: vec![],
        })
    }
    async fn is_issue_open(&self, issue_number: u64) -> Result<bool> {
        Ok(!self
            .state
            .lock()
            .unwrap()
            .closed_github_issues
            .contains(&issue_number))
    }
    async fn find_pr_by_branches(&self, _h: &str, _b: &str) -> Result<Option<GitHubPr>> {
        Ok(None)
    }
    async fn is_pr_merged(&self, pr_number: u64) -> Result<bool> {
        Ok(self.state.lock().unwrap().merged_prs.contains(&pr_number))
    }
    async fn list_unresolved_threads(&self, _n: u64) -> Result<Vec<ReviewThread>> {
        Ok(vec![])
    }
    async fn create_pr(&self, _h: &str, _b: &str, _t: &str, _body: &str) -> Result<u64> {
        Ok(100)
    }
    async fn reply_to_thread(&self, _id: &str, _body: &str) -> Result<()> {
        Ok(())
    }
    async fn resolve_thread(&self, _id: &str) -> Result<()> {
        Ok(())
    }
    async fn comment_on_issue(&self, issue_number: u64, body: &str) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .comments
            .push((issue_number, body.to_string()));
        Ok(())
    }
    async fn close_issue(&self, issue_number: u64) -> Result<()> {
        self.state.lock().unwrap().closed_issues.push(issue_number);
        Ok(())
    }
    async fn get_ci_check_runs(&self, _pr_number: u64) -> Result<Vec<GitHubCheckRun>> {
        Ok(vec![])
    }
    async fn get_pr_mergeable(&self, _pr_number: u64) -> Result<Option<bool>> {
        Ok(Some(true))
    }
    async fn get_pr_details(&self, pr_number: u64) -> Result<GitHubPrDetails> {
        Ok(GitHubPrDetails {
            merged: self.state.lock().unwrap().merged_prs.contains(&pr_number),
            mergeable: Some(true),
        })
    }
}

// === Mock Git Worktree ===

struct MockGitWorktree;

impl GitWorktree for MockGitWorktree {
    fn fetch(&self) -> Result<()> {
        Ok(())
    }
    fn create(&self, _p: &Path, _b: &str, _s: &str) -> Result<()> {
        Ok(())
    }
    fn remove(&self, _p: &Path) -> Result<()> {
        Ok(())
    }
    fn create_branch(&self, _p: &Path, _b: &str) -> Result<()> {
        Ok(())
    }
    fn checkout(&self, _p: &Path, _b: &str) -> Result<()> {
        Ok(())
    }
    fn pull(&self, _p: &Path) -> Result<()> {
        Ok(())
    }
    fn push(&self, _p: &Path, _b: &str) -> Result<()> {
        Ok(())
    }
    fn delete_branch(&self, _b: &str) -> Result<()> {
        Ok(())
    }
}

// === Helpers ===

fn test_config() -> Config {
    Config::default_with_repo("owner".into(), "repo".into(), "main".into())
}

#[allow(dead_code)]
fn new_issue(issue_number: u64) -> Issue {
    Issue {
        id: 0,
        github_issue_number: issue_number,
        state: State::Idle,
        design_pr_number: None,
        impl_pr_number: None,
        worktree_path: None,
        retry_count: 0,
        current_pid: None,
        error_message: None,
        feature_name: None,
        fixing_causes: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn setup() -> (SqliteIssueRepository, MockGitHubClient, Config) {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    let repo = SqliteIssueRepository::new(db);
    let github = MockGitHubClient::new();
    let config = test_config();
    (repo, github, config)
}

// === Task 8.1: State Machine Integration Tests ===

#[tokio::test]
async fn full_happy_path_idle_to_completed() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    // Create issue and transition through full lifecycle
    let mut issue = uc.handle_issue_detected(42).await.expect("detect");
    assert_eq!(issue.state, State::Initialized);

    // Initialized → DesignRunning (normally done in step2 initialization)
    // We simulate the InitializationCompleted event
    issue.worktree_path = Some("/tmp/wt".into());
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::InitializationCompleted)
        .await
        .expect("init completed");
    assert_eq!(issue.state, State::DesignRunning);
    assert_eq!(issue.retry_count, 0);

    // DesignRunning → DesignReviewWaiting
    issue.design_pr_number = Some(85);
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::ProcessCompletedWithPr)
        .await
        .expect("design done");
    assert_eq!(issue.state, State::DesignReviewWaiting);
    assert_eq!(issue.retry_count, 0);

    // DesignReviewWaiting → ImplementationRunning (merge)
    uc.apply(&mut issue, &Event::DesignPrMerged)
        .await
        .expect("design merged");
    assert_eq!(issue.state, State::ImplementationRunning);
    assert_eq!(issue.retry_count, 0);

    // ImplementationRunning → ImplementationReviewWaiting
    issue.impl_pr_number = Some(90);
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::ProcessCompletedWithPr)
        .await
        .expect("impl done");
    assert_eq!(issue.state, State::ImplementationReviewWaiting);

    // ImplementationReviewWaiting → Completed (merge)
    uc.apply(&mut issue, &Event::ImplementationPrMerged)
        .await
        .expect("impl merged");
    assert_eq!(issue.state, State::Completed);

    // Verify side effects
    let gh_state = github.state.lock().unwrap();
    assert!(
        gh_state
            .comments
            .iter()
            .any(|(n, msg)| *n == 42 && msg.contains("実装を開始"))
    );
    assert!(
        gh_state
            .comments
            .iter()
            .any(|(n, msg)| *n == 42 && msg.contains("全工程が完了"))
    );
    assert!(gh_state.closed_issues.contains(&42));
}

#[tokio::test]
async fn issue_close_cancels_from_any_state() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    let mut issue = uc.handle_issue_detected(50).await.expect("detect");
    assert_eq!(issue.state, State::Initialized);

    // IssueClosed from Initialized → Cancelled
    uc.apply(&mut issue, &Event::IssueClosed)
        .await
        .expect("close");
    assert_eq!(issue.state, State::Cancelled);

    // Verify cleanup comment
    let gh_state = github.state.lock().unwrap();
    assert!(
        gh_state
            .comments
            .iter()
            .any(|(n, msg)| *n == 50 && msg.contains("cleanup"))
    );
}

#[tokio::test]
async fn retry_count_resets_on_normal_transition() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    let mut issue = uc.handle_issue_detected(60).await.expect("detect");

    // Simulate InitializationCompleted
    issue.worktree_path = Some("/tmp/wt".into());
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::InitializationCompleted)
        .await
        .expect("init");
    assert_eq!(issue.state, State::DesignRunning);

    // ProcessFailed → retry_count increments
    uc.apply(&mut issue, &Event::ProcessFailed)
        .await
        .expect("fail");
    let updated = repo
        .find_by_id(issue.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(updated.retry_count, 1);
    assert_eq!(updated.state, State::DesignRunning);

    // Another failure
    issue.retry_count = updated.retry_count;
    uc.apply(&mut issue, &Event::ProcessFailed)
        .await
        .expect("fail2");
    let updated = repo
        .find_by_id(issue.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(updated.retry_count, 2);

    // Success → retry_count resets
    issue.retry_count = updated.retry_count;
    issue.design_pr_number = Some(100);
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::ProcessCompletedWithPr)
        .await
        .expect("success");
    assert_eq!(issue.state, State::DesignReviewWaiting);
    assert_eq!(issue.retry_count, 0);
}

#[tokio::test]
async fn retry_exhausted_leads_to_cancelled() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    let mut issue = uc.handle_issue_detected(70).await.expect("detect");
    issue.worktree_path = Some("/tmp/wt".into());
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::InitializationCompleted)
        .await
        .expect("init");

    // RetryExhausted → Cancelled
    uc.apply(&mut issue, &Event::RetryExhausted)
        .await
        .expect("exhausted");
    assert_eq!(issue.state, State::Cancelled);

    let gh_state = github.state.lock().unwrap();
    assert!(gh_state.closed_issues.contains(&70));
    assert!(
        gh_state
            .comments
            .iter()
            .any(|(n, msg)| *n == 70 && msg.contains("リトライ上限"))
    );
}

#[test]
fn prioritize_events_issue_closed_first() {
    let mut events = vec![
        (1, Event::ProcessFailed),
        (1, Event::IssueClosed),
        (2, Event::ProcessCompletedWithPr),
        (2, Event::IssueClosed),
    ];
    prioritize_events(&mut events);

    // IssueClosed events should come first
    assert_eq!(events[0].1, Event::IssueClosed);
    assert_eq!(events[1].1, Event::IssueClosed);
}

// === Task 8.2: Additional Polling Flow Tests ===

#[tokio::test]
async fn reopen_after_cancel_resets_issue() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    // First run → cancel
    let mut issue = uc.handle_issue_detected(80).await.expect("detect");
    uc.apply(&mut issue, &Event::IssueClosed)
        .await
        .expect("close");
    assert_eq!(issue.state, State::Cancelled);

    // Re-detect same issue → should reset
    let reopened = uc.handle_issue_detected(80).await.expect("re-detect");
    assert_eq!(reopened.state, State::Initialized);
    assert_eq!(reopened.retry_count, 0);
    assert!(reopened.design_pr_number.is_none());
}

#[tokio::test]
async fn design_fixing_flow() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    let mut issue = uc.handle_issue_detected(90).await.expect("detect");
    issue.worktree_path = Some("/tmp/wt".into());
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::InitializationCompleted)
        .await
        .expect("init");

    // DesignRunning → DesignReviewWaiting
    issue.design_pr_number = Some(200);
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::ProcessCompletedWithPr)
        .await
        .expect("pr");

    // DesignReviewWaiting → DesignFixing
    uc.apply(&mut issue, &Event::UnresolvedThreadsDetected)
        .await
        .expect("unresolved");
    assert_eq!(issue.state, State::DesignFixing);
    assert_eq!(issue.retry_count, 0);

    // DesignFixing → DesignReviewWaiting (fix complete)
    uc.apply(&mut issue, &Event::ProcessCompleted)
        .await
        .expect("fix done");
    assert_eq!(issue.state, State::DesignReviewWaiting);
    assert_eq!(issue.retry_count, 0);
}

// === Task 8.3: SessionManager Integration Tests ===
// (These are already well-covered in session_manager unit tests with real processes)

#[tokio::test]
async fn session_manager_lifecycle() {
    use cupola::application::session_manager::SessionManager;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let mut mgr = SessionManager::new();

    // Spawn a short-lived process
    let child = Command::new("echo")
        .arg("integration test output")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn echo");

    mgr.register(999, child);
    assert!(mgr.is_running(999));

    // Wait for it to finish
    std::thread::sleep(Duration::from_millis(200));

    let exited = mgr.collect_exited();
    assert_eq!(exited.len(), 1);
    assert_eq!(exited[0].issue_id, 999);
    assert!(exited[0].exit_status.success());
    assert!(exited[0].stdout.contains("integration test output"));
    assert!(!mgr.is_running(999));
}

#[tokio::test]
async fn session_manager_stall_detection() {
    use cupola::application::session_manager::SessionManager;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let mut mgr = SessionManager::new();

    let child = Command::new("sleep")
        .arg("60")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sleep");

    mgr.register(888, child);

    // Immediate stall check with 0 timeout → stalled
    let stalled = mgr.find_stalled(Duration::from_nanos(1));
    assert!(stalled.contains(&888));

    // Kill it
    mgr.kill(888).expect("kill");
    std::thread::sleep(Duration::from_millis(100));

    let exited = mgr.collect_exited();
    assert_eq!(exited.len(), 1);
    assert!(!exited[0].exit_status.success());
}

// === Polling-bugfix: Issue close + PR merge priority tests ===

#[tokio::test]
async fn impl_review_waiting_issue_close_with_merged_pr_becomes_completed() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    // Set up issue in ImplementationReviewWaiting with impl_pr_number
    let mut issue = uc.handle_issue_detected(100).await.expect("detect");
    issue.worktree_path = Some("/tmp/wt".into());
    issue.state = State::ImplementationReviewWaiting;
    issue.impl_pr_number = Some(500);
    repo.update(&issue).await.expect("update");
    repo.update_state(issue.id, State::ImplementationReviewWaiting)
        .await
        .expect("update state");

    // Mark PR as merged and Issue as closed on GitHub (simulates Closes #N)
    github.state.lock().unwrap().merged_prs.push(500);
    github.state.lock().unwrap().closed_github_issues.push(100);

    // Simulate: Issue close detected, but PR is merged → should become completed
    // This is what resolve_close_event_for_issue does
    use cupola::application::polling_use_case::PollingUseCase;
    use cupola::application::port::claude_code_runner::ClaudeCodeRunner;
    use std::path::Path;
    use std::process::Child;

    struct DummyRunner;
    impl ClaudeCodeRunner for DummyRunner {
        fn spawn(&self, _p: &str, _d: &Path, _s: Option<&str>, _m: &str) -> Result<Child> {
            anyhow::bail!("not implemented")
        }
    }

    struct DummyExecLog;
    impl cupola::application::port::execution_log_repository::ExecutionLogRepository for DummyExecLog {
        async fn record_start(&self, _: i64, _: State) -> Result<i64> {
            Ok(0)
        }
        async fn record_finish(
            &self,
            _: i64,
            _: Option<i32>,
            _: Option<&str>,
            _: Option<&str>,
        ) -> Result<()> {
            Ok(())
        }
        async fn find_by_issue(
            &self,
            _: i64,
        ) -> Result<Vec<cupola::domain::execution_log::ExecutionLog>> {
            Ok(vec![])
        }
    }

    let mut polling =
        PollingUseCase::new(github, repo, DummyExecLog, DummyRunner, worktree, config);

    // Run one cycle — Issue is closed + PR is merged
    // Step 1 should detect close, check merge, emit ImplementationPrMerged
    // Step 6 should apply it → completed
    polling.run_cycle().await.expect("cycle");

    // Verify: The issue should be completed, not cancelled
    let final_issue = polling
        .issue_repo_ref()
        .find_by_id(issue.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(
        final_issue.state,
        State::Completed,
        "Should be completed, not cancelled"
    );
}

#[tokio::test]
async fn impl_review_waiting_issue_close_without_merge_becomes_cancelled() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    let mut issue = uc.handle_issue_detected(101).await.expect("detect");
    issue.worktree_path = Some("/tmp/wt".into());
    issue.state = State::ImplementationReviewWaiting;
    issue.impl_pr_number = Some(501);
    repo.update(&issue).await.expect("update");
    repo.update_state(issue.id, State::ImplementationReviewWaiting)
        .await
        .expect("update state");

    // PR is NOT merged — IssueClosed should lead to cancelled
    // (merged_prs is empty)

    // Apply IssueClosed directly via TransitionUseCase
    uc.apply(&mut issue, &Event::IssueClosed)
        .await
        .expect("close");
    assert_eq!(
        issue.state,
        State::Cancelled,
        "Should be cancelled when PR is not merged"
    );
}

// === INFO ログ出力検証テスト ===
// tracing::info! はクロスカッティング関心事であり、ログ出力自体は tracing の内部で処理される。
// 以下のテストでは、ログ追加後も代表的な状態遷移パスが正常に動作することを検証する。
// 実際のログ出力内容は E2E テスト（cupola run）で目視確認する。

#[tokio::test]
async fn state_transition_log_does_not_break_representative_transition_paths() {
    let (repo, github, config) = setup();
    let worktree = MockGitWorktree;

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    // Path 1: Idle → Initialized (via handle_issue_detected)
    let mut issue = uc.handle_issue_detected(42).await.expect("detect");
    assert_eq!(issue.state, State::Initialized);

    // Path 2: Initialized → DesignRunning
    issue.worktree_path = Some("/tmp/wt".into());
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::InitializationCompleted)
        .await
        .expect("init completed");
    assert_eq!(issue.state, State::DesignRunning);

    // Path 3: DesignRunning → DesignReviewWaiting
    issue.design_pr_number = Some(100);
    repo.update(&issue).await.expect("update");
    uc.apply(&mut issue, &Event::ProcessCompletedWithPr)
        .await
        .expect("pr");
    assert_eq!(issue.state, State::DesignReviewWaiting);

    // Path 4: DesignReviewWaiting → DesignFixing
    uc.apply(&mut issue, &Event::UnresolvedThreadsDetected)
        .await
        .expect("unresolved");
    assert_eq!(issue.state, State::DesignFixing);

    // Path 5: DesignFixing → DesignReviewWaiting
    uc.apply(&mut issue, &Event::ProcessCompleted)
        .await
        .expect("fix done");
    assert_eq!(issue.state, State::DesignReviewWaiting);

    // Path 6: IssueClosed → Cancelled
    let mut issue2 = uc.handle_issue_detected(43).await.expect("detect");
    uc.apply(&mut issue2, &Event::IssueClosed)
        .await
        .expect("close");
    assert_eq!(issue2.state, State::Cancelled);

    // Path 7: ProcessFailed → same state (retry)
    let mut issue3 = uc.handle_issue_detected(44).await.expect("detect");
    issue3.worktree_path = Some("/tmp/wt3".into());
    repo.update(&issue3).await.expect("update");
    uc.apply(&mut issue3, &Event::InitializationCompleted)
        .await
        .expect("init");
    uc.apply(&mut issue3, &Event::ProcessFailed)
        .await
        .expect("fail");
    assert_eq!(issue3.state, State::DesignRunning);
}

// === Concurrent Session Limit Tests ===

struct MockClaudeCodeRunner;

impl ClaudeCodeRunner for MockClaudeCodeRunner {
    fn spawn(
        &self,
        _prompt: &str,
        _working_dir: &Path,
        _json_schema: Option<&str>,
        _model: &str,
    ) -> Result<std::process::Child> {
        use std::process::{Command, Stdio};
        Command::new("sleep")
            .arg("60")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Into::into)
    }
}

struct MockExecutionLogRepository;

impl ExecutionLogRepository for MockExecutionLogRepository {
    async fn record_start(&self, _issue_id: i64, _state: State) -> Result<i64> {
        Ok(0)
    }
    async fn record_finish(
        &self,
        _log_id: i64,
        _exit_code: Option<i32>,
        _structured_output: Option<&str>,
        _error_message: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
    async fn find_by_issue(&self, _issue_id: i64) -> Result<Vec<ExecutionLog>> {
        Ok(vec![])
    }
}

type TestPollingUseCase = PollingUseCase<
    MockGitHubClient,
    SqliteIssueRepository,
    MockExecutionLogRepository,
    MockClaudeCodeRunner,
    MockGitWorktree,
>;

fn setup_polling(config: Config) -> TestPollingUseCase {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    let repo = SqliteIssueRepository::new(db);
    let github = MockGitHubClient::new();
    let exec_log = MockExecutionLogRepository;
    let claude_runner = MockClaudeCodeRunner;
    let worktree = MockGitWorktree;

    PollingUseCase::new(github, repo, exec_log, claude_runner, worktree, config)
}

#[tokio::test]
async fn concurrent_session_limit_restricts_spawning() {
    let mut config = test_config();
    config.max_concurrent_sessions = Some(2);

    let mut polling = setup_polling(config);

    // Create 3 issues in DesignRunning state with worktree paths
    for i in 1..=3u64 {
        let issue = Issue {
            id: 0,
            github_issue_number: i,
            state: State::DesignRunning,
            design_pr_number: None,
            impl_pr_number: None,
            worktree_path: Some(format!("/tmp/wt/{i}")),
            retry_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: None,
            fixing_causes: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        polling.issue_repo_ref().save(&issue).await.expect("save");
    }

    // Run one cycle
    let _ = polling.run_cycle().await;

    // Check that only 2 processes were spawned (via current_pid in DB)
    let issues = polling
        .issue_repo_ref()
        .find_needing_process()
        .await
        .expect("find");
    let spawned_count = issues.iter().filter(|i| i.current_pid.is_some()).count();
    assert_eq!(
        spawned_count, 2,
        "should have spawned exactly 2 processes with limit of 2"
    );

    // Clean up spawned processes
    polling.graceful_shutdown().await;
}

#[tokio::test]
async fn no_limit_spawns_all_processes() {
    let config = test_config(); // max_concurrent_sessions = None

    let mut polling = setup_polling(config);

    // Create 3 issues in DesignRunning state
    for i in 1..=3u64 {
        let issue = Issue {
            id: 0,
            github_issue_number: i,
            state: State::DesignRunning,
            design_pr_number: None,
            impl_pr_number: None,
            worktree_path: Some(format!("/tmp/wt/{i}")),
            retry_count: 0,
            current_pid: None,
            error_message: None,
            feature_name: None,
            fixing_causes: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        polling.issue_repo_ref().save(&issue).await.expect("save");
    }

    let _ = polling.run_cycle().await;

    let issues = polling
        .issue_repo_ref()
        .find_needing_process()
        .await
        .expect("find");
    let spawned_count = issues.iter().filter(|i| i.current_pid.is_some()).count();
    assert_eq!(
        spawned_count, 3,
        "should have spawned all 3 processes with no limit"
    );

    polling.graceful_shutdown().await;
}

#[test]
fn config_validation_rejects_zero() {
    let mut config = test_config();
    config.max_concurrent_sessions = Some(0);
    assert!(config.validate().is_err());
}

#[tokio::test]
async fn session_count_decreases_after_process_exit() {
    use cupola::application::session_manager::SessionManager;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let mut mgr = SessionManager::new();
    assert_eq!(mgr.count(), 0);

    // Spawn 2 short-lived and 1 long-lived process
    let echo1 = Command::new("echo")
        .arg("a")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let echo2 = Command::new("echo")
        .arg("b")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let sleep = Command::new("sleep")
        .arg("60")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    mgr.register(1, echo1);
    mgr.register(2, echo2);
    mgr.register(3, sleep);
    assert_eq!(mgr.count(), 3);

    // Wait for echo processes to finish
    std::thread::sleep(Duration::from_millis(300));

    let exited = mgr.collect_exited();
    assert_eq!(exited.len(), 2, "both echo processes should have exited");
    assert_eq!(mgr.count(), 1, "only sleep process should remain");

    mgr.kill_all();
}

// === Task 3.2: initialize_issue の動作変更テスト ===

struct TrackingGitWorktree {
    call_log: Arc<Mutex<Vec<String>>>,
    fetch_fails: bool,
}

impl TrackingGitWorktree {
    fn new() -> Self {
        Self {
            call_log: Arc::new(Mutex::new(Vec::new())),
            fetch_fails: false,
        }
    }

    fn with_failing_fetch() -> Self {
        Self {
            call_log: Arc::new(Mutex::new(Vec::new())),
            fetch_fails: true,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.call_log.lock().unwrap().clone()
    }
}

impl GitWorktree for TrackingGitWorktree {
    fn fetch(&self) -> Result<()> {
        self.call_log.lock().unwrap().push("fetch".to_string());
        if self.fetch_fails {
            anyhow::bail!("fetch failed: network error");
        }
        Ok(())
    }
    fn create(&self, _p: &Path, _b: &str, start_point: &str) -> Result<()> {
        self.call_log
            .lock()
            .unwrap()
            .push(format!("create:{}", start_point));
        Ok(())
    }
    fn remove(&self, _p: &Path) -> Result<()> {
        Ok(())
    }
    fn create_branch(&self, _p: &Path, _b: &str) -> Result<()> {
        Ok(())
    }
    fn checkout(&self, _p: &Path, _b: &str) -> Result<()> {
        Ok(())
    }
    fn pull(&self, _p: &Path) -> Result<()> {
        Ok(())
    }
    fn push(&self, _p: &Path, _b: &str) -> Result<()> {
        Ok(())
    }
    fn delete_branch(&self, _b: &str) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn initialize_issue_calls_fetch_before_create() {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    let repo = SqliteIssueRepository::new(db);
    let github = MockGitHubClient::new();
    let config = test_config();
    let worktree = TrackingGitWorktree::new();

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    // Issue を Initialized 状態で作成
    let issue = uc.handle_issue_detected(200).await.expect("detect");
    assert_eq!(issue.state, State::Initialized);

    // step2_initialized_recovery を通じて initialize_issue が呼ばれる
    let exec_log = MockExecutionLogRepository;
    let claude_runner = MockClaudeCodeRunner;
    let mut polling = PollingUseCase::new(github, repo, exec_log, claude_runner, worktree, config);
    polling.run_cycle().await.expect("cycle");

    // fetch が create より前に呼ばれていることを確認
    let calls = polling.worktree_ref().calls();
    let fetch_pos = calls.iter().position(|c| c == "fetch");
    let create_pos = calls.iter().position(|c| c.starts_with("create:"));
    assert!(fetch_pos.is_some(), "fetch should have been called");
    assert!(create_pos.is_some(), "create should have been called");
    assert!(
        fetch_pos.unwrap() < create_pos.unwrap(),
        "fetch must be called before create, calls: {:?}",
        calls
    );
}

#[tokio::test]
async fn initialize_issue_uses_remote_default_branch_as_start_point() {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    let repo = SqliteIssueRepository::new(db);
    let github = MockGitHubClient::new();
    let mut config = test_config();
    config.default_branch = "develop".to_string();
    let worktree = TrackingGitWorktree::new();

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    let _issue = uc.handle_issue_detected(201).await.expect("detect");

    let exec_log = MockExecutionLogRepository;
    let claude_runner = MockClaudeCodeRunner;
    let mut polling = PollingUseCase::new(github, repo, exec_log, claude_runner, worktree, config);
    polling.run_cycle().await.expect("cycle");

    // start_point が origin/{default_branch} であることを確認
    let calls = polling.worktree_ref().calls();
    let create_call = calls.iter().find(|c| c.starts_with("create:"));
    assert!(create_call.is_some(), "create should have been called");
    assert_eq!(
        create_call.unwrap(),
        "create:origin/develop",
        "start_point must be origin/{{default_branch}}, calls: {:?}",
        calls
    );
}

#[tokio::test]
async fn initialize_issue_aborts_on_fetch_failure() {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    let repo = SqliteIssueRepository::new(db);
    let github = MockGitHubClient::new();
    let config = test_config();
    let worktree = TrackingGitWorktree::with_failing_fetch();

    let uc = TransitionUseCase {
        github: &github,
        issue_repo: &repo,
        worktree: &worktree,
        config: &config,
    };

    let _issue = uc.handle_issue_detected(202).await.expect("detect");

    let exec_log = MockExecutionLogRepository;
    let claude_runner = MockClaudeCodeRunner;
    let mut polling = PollingUseCase::new(github, repo, exec_log, claude_runner, worktree, config);
    polling.run_cycle().await.expect("cycle");

    // fetch が失敗しても create は呼ばれないことを確認
    let calls = polling.worktree_ref().calls();
    assert!(
        calls.iter().any(|c| c == "fetch"),
        "fetch should have been attempted"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("create:")),
        "create must NOT be called when fetch fails, calls: {:?}",
        calls
    );
}
