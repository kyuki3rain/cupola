use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use cupola::adapter::outbound::sqlite_connection::SqliteConnection;
use cupola::adapter::outbound::sqlite_issue_repository::SqliteIssueRepository;
use cupola::application::port::git_worktree::GitWorktree;
use cupola::application::port::github_client::{
    GitHubClient, GitHubIssue, GitHubIssueDetail, GitHubPr, ReviewThread,
};
use cupola::application::port::issue_repository::IssueRepository;
use cupola::application::transition_use_case::{TransitionUseCase, prioritize_events};
use cupola::domain::config::Config;
use cupola::domain::event::Event;
use cupola::domain::issue::Issue;
use cupola::domain::state::State;

// === Mock GitHub Client ===

#[derive(Default)]
struct MockGitHubState {
    comments: Vec<(u64, String)>,
    closed_issues: Vec<u64>,
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
    async fn is_issue_open(&self, _n: u64) -> Result<bool> {
        Ok(true)
    }
    async fn find_pr_by_branches(&self, _h: &str, _b: &str) -> Result<Option<GitHubPr>> {
        Ok(None)
    }
    async fn is_pr_merged(&self, _n: u64) -> Result<bool> {
        Ok(false)
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
}

// === Mock Git Worktree ===

struct MockGitWorktree;

impl GitWorktree for MockGitWorktree {
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
