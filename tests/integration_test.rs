#![allow(clippy::expect_used)]
// Integration tests that use real processes and in-memory state.
//
// # Retired tests (Phase 7)
//
// The following 18 tests were marked `#[ignore]` with "TODO(phase 7)" and have been
// formally retired in Phase 7 for the following reasons:
//
// ## State machine scenario tests (8 tests)
// - `full_happy_path_idle_to_completed` → covered by `T-7.IT.normal` in `tests/scenarios.rs`
// - `issue_close_cancels_from_any_state` → covered by `T-7.IT.cancel_close` in `tests/scenarios.rs`
// - `retry_count_resets_on_normal_transition` → covered by `domain::decide::tests`
// - `retry_exhausted_leads_to_cancelled` → covered by `T-7.IT.cancel_retry` in `tests/scenarios.rs`
// - `prioritize_events_issue_closed_first` → superseded by `Effect::priority()` in domain model
// - `reopen_after_cancel_resets_issue` → covered by `T-7.IT.reopen` in `tests/scenarios.rs`
// - `design_fixing_flow` → covered by `T-7.IT.fixing_merge` in `tests/scenarios.rs`
//
// ## Polling loop scenario tests (5 tests)
// - `impl_review_waiting_issue_close_with_merged_pr_becomes_completed` → covered by domain decide tests
// - `impl_review_waiting_issue_close_without_merge_becomes_cancelled` → covered by domain decide tests
// - `state_transition_log_does_not_break_representative_transition_paths` → covered by domain decide tests
// - `concurrent_session_limit_restricts_spawning` → covered by `T-3.EX.MaxSessions` unit test
// - `no_limit_spawns_all_processes` → covered by unit test in execute.rs
//
// ## Worktree initialization tests (3 tests)
// - `initialize_issue_calls_fetch_before_create` → covered by `T-4.WT.2` in git_worktree_manager tests
// - `initialize_issue_uses_remote_default_branch_as_start_point` → covered by execute.rs unit tests
// - `initialize_issue_aborts_on_fetch_failure` → covered by `T-4.WT.*` tests
//
// ## i18n tests (3 tests)
// - `english_config_posts_english_comments` → covered by i18n unit tests in polling_use_case.rs
// - `unknown_locale_falls_back_to_english` → covered by i18n unit tests in polling_use_case.rs
// - `english_config_retry_exhausted_posts_english` → covered by i18n unit tests in polling_use_case.rs

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use cupola::adapter::outbound::sqlite_connection::SqliteConnection;
use cupola::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
use cupola::application::polling_use_case::PollingUseCase;
use cupola::application::port::claude_code_runner::ClaudeCodeRunner;
use cupola::application::port::execution_log_repository::ExecutionLogRepository;
use cupola::application::port::file_generator::FileGenerator;
use cupola::application::port::git_worktree::GitWorktree;
use cupola::application::port::github_client::{
    GitHubCheckRun, GitHubClient, GitHubIssue, GitHubIssueDetail, GitHubPr, GitHubPrDetails,
    PrStatus, RepositoryPermission, ReviewThread,
};
use cupola::domain::config::Config;
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
    async fn get_job_logs(&self, _job_id: u64) -> Result<String> {
        Ok(String::new())
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
    async fn get_pr_status(&self, pr_number: u64) -> Result<PrStatus> {
        if self.state.lock().unwrap().merged_prs.contains(&pr_number) {
            Ok(PrStatus::Merged)
        } else {
            Ok(PrStatus::Closed)
        }
    }

    async fn fetch_label_actor_login(
        &self,
        _issue_number: u64,
        _label_name: &str,
    ) -> Result<Option<String>> {
        Ok(Some("test-actor".to_string()))
    }

    async fn fetch_user_permission(&self, _username: &str) -> Result<RepositoryPermission> {
        Ok(RepositoryPermission::Admin)
    }

    async fn remove_label(&self, _issue_number: u64, _label_name: &str) -> Result<()> {
        Ok(())
    }
}

// === Mock Git Worktree ===

#[derive(Default)]
struct MockGitWorktreeState {
    pushes: Vec<(String, String)>,
}

#[allow(dead_code)]
#[derive(Clone)]
struct MockGitWorktree {
    state: Arc<Mutex<MockGitWorktreeState>>,
}

impl Default for MockGitWorktree {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockGitWorktreeState::default())),
        }
    }
}

impl GitWorktree for MockGitWorktree {
    fn fetch(&self) -> Result<()> {
        Ok(())
    }
    fn merge(&self, _p: &Path, _b: &str) -> Result<()> {
        Ok(())
    }
    fn exists(&self, _p: &Path) -> bool {
        false
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
    fn push(&self, p: &Path, b: &str) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .pushes
            .push((p.to_string_lossy().to_string(), b.to_string()));
        Ok(())
    }
    fn delete_branch(&self, _b: &str) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct MockFileGenerator;

impl FileGenerator for MockFileGenerator {
    fn generate_toml_template(&self) -> Result<bool> {
        Ok(false)
    }

    fn install_claude_code_assets(&self) -> Result<bool> {
        Ok(false)
    }

    fn append_gitignore_entries(&self) -> Result<bool> {
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
        _base_dir: &std::path::Path,
        _issue_number: u64,
        _issue_body: &str,
        _language: &str,
    ) -> Result<bool> {
        Ok(false)
    }
}

// === Helpers ===

fn test_config() -> Config {
    Config::default_with_repo("owner".into(), "repo".into(), "main".into())
}

#[allow(dead_code)]
fn _new_issue(issue_number: u64) -> Issue {
    Issue {
        id: 0,
        github_issue_number: issue_number,
        state: State::Idle,
        worktree_path: None,
        ci_fix_count: 0,
        close_finished: false,
        consecutive_failures_epoch: None,
        feature_name: format!("issue-{issue_number}"),
        weight: cupola::domain::task_weight::TaskWeight::Medium,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[allow(dead_code)]
fn setup() -> (SqliteIssueRepository, MockGitHubClient, Config) {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    let repo = SqliteIssueRepository::new(db);
    let github = MockGitHubClient::new();
    let config = test_config();
    (repo, github, config)
}

// === Task 8.3: SessionManager Integration Tests ===

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

    mgr.register(999, State::DesignRunning, child);
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

    mgr.register(888, State::DesignRunning, child);

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

// === Concurrent Session Limit Tests ===

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
type TestPollingUseCase = PollingUseCase<
    MockGitHubClient,
    SqliteIssueRepository,
    MockExecutionLogRepository,
    MockClaudeCodeRunner,
    MockGitWorktree,
    MockFileGenerator,
>;

#[allow(dead_code)]
fn setup_polling(config: Config) -> TestPollingUseCase {
    let db = SqliteConnection::open_in_memory().expect("open");
    db.init_schema().expect("init");
    let repo = SqliteIssueRepository::new(db);
    let github = MockGitHubClient::new();
    let exec_log = MockExecutionLogRepository;
    let claude_runner = MockClaudeCodeRunner;
    let worktree = MockGitWorktree::default();
    let file_gen = MockFileGenerator;

    PollingUseCase::new(
        github,
        repo,
        exec_log,
        claude_runner,
        worktree,
        file_gen,
        config,
    )
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

    mgr.register(1, State::DesignRunning, echo1);
    mgr.register(2, State::DesignRunning, echo2);
    mgr.register(3, State::ImplementationRunning, sleep);
    assert_eq!(mgr.count(), 3);

    // Wait for echo processes to finish
    std::thread::sleep(Duration::from_millis(300));

    let exited = mgr.collect_exited();
    assert_eq!(exited.len(), 2, "both echo processes should have exited");
    assert_eq!(mgr.count(), 1, "only sleep process should remain");

    mgr.kill_all();
}

// === Version flag integration tests ===

#[test]
fn version_flag_exits_with_zero_and_prints_version() {
    let bin = env!("CARGO_BIN_EXE_cupola");
    let output = std::process::Command::new(bin)
        .arg("--version")
        .output()
        .expect("failed to execute cupola binary");

    assert!(
        output.status.success(),
        "--version should exit with code 0, got: {:?}",
        output.status.code()
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cupola"),
        "stdout should contain 'cupola': {stdout}"
    );
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "stdout should contain version '{}': {stdout}",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn version_short_flag_exits_with_zero_and_matches_long() {
    let bin = env!("CARGO_BIN_EXE_cupola");

    let long_out = std::process::Command::new(bin)
        .arg("--version")
        .output()
        .expect("failed to execute cupola --version");
    let short_out = std::process::Command::new(bin)
        .arg("-V")
        .output()
        .expect("failed to execute cupola -V");

    assert!(
        short_out.status.success(),
        "-V should exit with code 0, got: {:?}",
        short_out.status.code()
    );
    assert_eq!(
        short_out.stdout, long_out.stdout,
        "-V and --version should produce identical stdout"
    );
}
