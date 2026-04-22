use std::path::Path;
use std::process::Child;

use anyhow::Result;

use crate::application::io::{clear_inputs_dir, write_issue_input, write_review_threads_input};
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::file_generator::FileGenerator;
use crate::application::port::git_worktree::MergeConflictError;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::application::prompt::{
    FIXING_SCHEMA, OutputSchemaKind, PR_CREATION_SCHEMA, build_session_config,
};
use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};
use crate::domain::state::State;

use super::context::ExecuteContext;
use super::shared;
use super::{BodyTamperedError, SpawnableGitWorktree, sha256_hex};

pub(super) async fn execute<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &mut Issue,
    type_: ProcessRunType,
    causes: &[FixingProblemKind],
    pending_run_id: Option<i64>,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    let result = spawn_process(ctx, issue, type_, causes, pending_run_id).await;

    if let Err(ref e) = result
        && e.downcast_ref::<BodyTamperedError>().is_some()
    {
        handle_body_tampered(ctx, issue).await;
        return Ok(());
    }
    result?;
    Ok(())
}

async fn spawn_process<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &Issue,
    type_: ProcessRunType,
    causes: &[FixingProblemKind],
    pending_run_id: Option<i64>,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    let reserved = if let Some(max) = ctx.config.max_concurrent_sessions {
        if !ctx.session_mgr.try_reserve(max as usize) {
            tracing::info!(
                issue_number = issue.github_issue_number,
                "concurrent session limit reached, deferring spawn"
            );
            if pending_run_id.is_none() {
                let latest = ctx.process_repo.find_latest(issue.id, type_).await?;
                let index = latest.map(|r| r.index + 1).unwrap_or(0);
                let run = ProcessRun::new_pending(issue.id, type_, index, causes.to_vec());
                let run_id = ctx.process_repo.save(&run).await?;
                tracing::info!(
                    issue_number = issue.github_issue_number,
                    run_id,
                    type_ = ?type_,
                    "created pending process run (session limit)"
                );
            }
            return Ok(());
        }
        true
    } else {
        false
    };

    match prepare_process_spawn(ctx, issue, type_, causes, pending_run_id).await {
        Ok((child, run_id, pid)) => {
            ctx.session_mgr
                .register(issue.id, issue.state, child, run_id);
            tracing::info!(
                issue_number = issue.github_issue_number,
                run_id,
                pid,
                type_ = ?type_,
                "spawned process"
            );
        }
        Err(e) => {
            if reserved {
                ctx.session_mgr.release_reservation();
            }
            return Err(e);
        }
    }

    Ok(())
}

pub(super) async fn prepare_process_spawn<G, I, P, C, W, F>(
    ctx: &ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &Issue,
    type_: ProcessRunType,
    causes: &[FixingProblemKind],
    pending_run_id: Option<i64>,
) -> Result<(Child, i64, u32)>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    let wt_path_str = issue
        .worktree_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("worktree_path is None for SpawnProcess"))?;
    let wt_path = Path::new(wt_path_str);

    let detail = ctx.github.get_issue(issue.github_issue_number).await?;

    let current_hash = sha256_hex(&detail.body);
    if let Some(saved) = &issue.body_hash
        && saved != &current_hash
    {
        return Err(BodyTamperedError {
            issue_number: issue.github_issue_number,
        }
        .into());
    }

    clear_inputs_dir(wt_path)?;
    write_issue_input(wt_path, &detail)?;

    let pr_number_opt = shared::get_pr_number_for_type(ctx.process_repo, issue.id, type_).await?;
    if matches!(type_, ProcessRunType::ImplFix) {
        if let Err(e) = ctx.worktree.fetch() {
            tracing::warn!(
                error = %e,
                "fetch failed before fixing merge, proceeding with stale origin"
            );
        }

        if let Err(e) = ctx.worktree.merge(wt_path, &ctx.config.default_branch) {
            if e.downcast_ref::<MergeConflictError>().is_some() {
                tracing::warn!(
                    error = %e,
                    "merge conflict detected before fixing spawn, delegating resolution to Claude Code"
                );
            } else {
                tracing::warn!(
                    error = %e,
                    "merge failed before fixing spawn (non-conflict), proceeding anyway"
                );
            }
        }
    }
    if matches!(type_, ProcessRunType::DesignFix | ProcessRunType::ImplFix)
        && let Some(pr_number) = pr_number_opt
    {
        let threads = ctx.github.list_unresolved_threads(pr_number).await?;
        write_review_threads_input(wt_path, &threads, ctx.config)?;
    }

    let state = crate::domain::phase::Phase::from_state(issue.state)
        .map(|p| shared::state_from_phase(p, type_))
        .unwrap_or(issue.state);

    let session_config = build_session_config(
        state,
        issue.github_issue_number,
        ctx.config,
        pr_number_opt,
        &issue.feature_name,
        causes,
    )?;

    let schema = match session_config.output_schema {
        OutputSchemaKind::PrCreation => Some(PR_CREATION_SCHEMA),
        OutputSchemaKind::Fixing => Some(FIXING_SCHEMA),
    };

    let model_phase = shared::phase_for_type(type_);
    let model = ctx.config.models.resolve(issue.weight, model_phase);

    let run_id = if let Some(existing_run_id) = pending_run_id {
        ctx.process_repo
            .update_state(existing_run_id, ProcessRunState::Running)
            .await?;
        existing_run_id
    } else {
        let latest = ctx.process_repo.find_latest(issue.id, type_).await?;
        let index = latest.map(|r| r.index + 1).unwrap_or(0);
        let run = ProcessRun::new_running(issue.id, type_, index, causes.to_vec());
        ctx.process_repo.save(&run).await?
    };

    match ctx
        .claude_runner
        .spawn(&session_config.prompt, wt_path, schema, model)
    {
        Ok(mut child) => {
            let pid = child.id();
            if let Err(e) = ctx.process_repo.update_pid(run_id, pid).await {
                let _ = child.kill();
                let _ = child.wait();
                let _ = ctx
                    .process_repo
                    .mark_failed(run_id, Some(format!("update_pid failed: {e}")))
                    .await;
                return Err(e);
            }
            Ok((child, run_id, pid))
        }
        Err(e) => {
            let _ = ctx
                .process_repo
                .mark_failed(run_id, Some(e.to_string()))
                .await;
            Err(e)
        }
    }
}

async fn handle_body_tampered<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &mut Issue,
) where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    let n = issue.github_issue_number;
    let lang = &ctx.config.language;

    let updates = MetadataUpdates {
        state: Some(State::Cancelled),
        ..Default::default()
    };
    if let Err(e) = ctx
        .issue_repo
        .update_state_and_metadata(issue.id, &updates)
        .await
    {
        tracing::error!(
            issue_number = n,
            error = %e,
            "failed to transition issue to Cancelled after body tampering"
        );
    }

    if let Err(e) = ctx.github.remove_label(n, "agent:ready").await {
        tracing::warn!(
            issue_number = n,
            error = %e,
            "failed to remove agent:ready label after body tampering"
        );
    }

    let msg = rust_i18n::t!("issue_comment.body_tampered", locale = lang);
    if let Err(e) = ctx.github.comment_on_issue(n, &msg).await {
        tracing::warn!(
            issue_number = n,
            error = %e,
            "failed to post tamper notification comment"
        );
    }

    issue.state = State::Cancelled;

    tracing::warn!(
        issue_number = n,
        "Issue body was tampered after agent:ready approval — cancelled"
    );
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use anyhow::Result;

    use super::prepare_process_spawn;
    use crate::application::init_task_manager::InitTaskManager;
    use crate::application::polling::execute::context::ExecuteContext;
    use crate::application::port::file_generator::FileGenerator;
    use crate::application::port::git_worktree::GitWorktree;
    use crate::application::port::github_client::{
        GitHubIssueDetail, GitHubPr, GitHubPrDetails, RepositoryPermission, ReviewThread,
    };
    use crate::application::port::process_run_repository::ProcessRunRepository;
    use crate::application::session_manager::SessionManager;
    use crate::domain::config::Config;
    use crate::domain::issue::Issue;
    use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};

    use super::super::sha256_hex;

    type CallLog = Arc<Mutex<Vec<String>>>;

    #[derive(Clone, Default)]
    struct MockGitWorktree {
        calls: CallLog,
        fail_fetch: bool,
    }

    impl MockGitWorktree {
        fn with_log(calls: CallLog) -> Self {
            Self {
                calls,
                fail_fetch: false,
            }
        }

        fn with_fail_fetch(calls: CallLog) -> Self {
            Self {
                calls,
                fail_fetch: true,
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

        fn push(&self, _p: &Path, _b: &str) -> Result<()> {
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
            _base_dir: &Path,
            _issue_number: u64,
            _issue_body: &str,
            _language: &str,
        ) -> Result<bool> {
            Ok(false)
        }
    }

    #[derive(Clone)]
    struct MockGitHub {
        body: String,
        comments: Arc<Mutex<Vec<String>>>,
        removed_labels: Arc<Mutex<Vec<String>>>,
    }

    impl MockGitHub {
        fn with_body(body: impl Into<String>) -> Self {
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

    impl crate::application::port::github_client::GitHubClient for MockGitHub {
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
    struct MockProcRepo;

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
        async fn mark_failed(&self, _: i64, _: Option<String>) -> anyhow::Result<()> {
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

    struct MockIssueRepoStub;

    impl crate::application::port::issue_repository::IssueRepository for MockIssueRepoStub {
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
            unimplemented!()
        }
        async fn find_by_state(&self, _: crate::domain::state::State) -> Result<Vec<Issue>> {
            unimplemented!()
        }
    }

    struct MockClaudeRunnerFailing;

    impl crate::application::port::claude_code_runner::ClaudeCodeRunner for MockClaudeRunnerFailing {
        fn spawn(
            &self,
            _: &str,
            _: &Path,
            _: Option<&str>,
            _: &str,
        ) -> Result<std::process::Child> {
            Err(anyhow::anyhow!("spawn intentionally failed in test"))
        }
    }

    fn make_fixing_issue(wt_path: &str) -> Issue {
        let mut issue = Issue::new(42, "issue-42".to_string());
        issue.id = 1;
        issue.state = crate::domain::state::State::DesignFixing;
        issue.worktree_path = Some(wt_path.to_string());
        issue
    }

    fn make_test_config(worktrees_parent: &std::path::Path) -> Config {
        let mut config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        config.log_dir = worktrees_parent.join("logs").join("cupola.log");
        config
    }

    fn make_test_issue(feature_name: &str) -> Issue {
        let mut issue = Issue::new(1, feature_name.to_string());
        issue.id = 1;
        issue
    }

    /// T-fix.1: ImplFix spawn で fetch → merge の順序が保証されること（要件 2.1）
    #[tokio::test]
    async fn impl_fixing_spawn_calls_fetch_before_merge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");
        let mut issue = make_fixing_issue(wt_path);
        issue.state = crate::domain::state::State::ImplementationFixing;
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log.clone());
        let github = MockGitHub::with_body("Issue body");
        let proc_repo = MockProcRepo;
        let issue_repo = MockIssueRepoStub;
        let file_gen = MockFileGenerator;
        let mut session_mgr = SessionManager::new();
        let mut init_mgr = InitTaskManager::new();

        let ctx = ExecuteContext {
            github: &github,
            issue_repo: &issue_repo,
            process_repo: &proc_repo,
            claude_runner: &MockClaudeRunnerFailing,
            worktree: &worktree,
            file_gen: &file_gen,
            session_mgr: &mut session_mgr,
            init_mgr: &mut init_mgr,
            config: &config,
        };

        let _ = prepare_process_spawn(&ctx, &issue, ProcessRunType::ImplFix, &[], None).await;

        let calls = log.lock().expect("lock").clone();
        let fetch_pos = calls.iter().position(|c| c == "fetch");
        let merge_pos = calls.iter().position(|c| c == "merge");

        assert!(
            fetch_pos.is_some(),
            "fetch should be called in ImplFix spawn, calls: {:?}",
            calls
        );
        assert!(
            merge_pos.is_some(),
            "merge should be called in ImplFix spawn, calls: {:?}",
            calls
        );
        assert!(
            fetch_pos.unwrap() < merge_pos.unwrap(),
            "fetch must be called before merge, calls: {:?}",
            calls
        );
    }

    /// T-fix.2: ImplFix で fetch が失敗しても merge が呼ばれること（要件 2.2）
    #[tokio::test]
    async fn impl_fixing_spawn_continues_to_merge_when_fetch_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");
        let mut issue = make_fixing_issue(wt_path);
        issue.state = crate::domain::state::State::ImplementationFixing;
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_fail_fetch(log.clone());
        let github = MockGitHub::with_body("Issue body");
        let proc_repo = MockProcRepo;
        let issue_repo = MockIssueRepoStub;
        let file_gen = MockFileGenerator;
        let mut session_mgr = SessionManager::new();
        let mut init_mgr = InitTaskManager::new();

        let ctx = ExecuteContext {
            github: &github,
            issue_repo: &issue_repo,
            process_repo: &proc_repo,
            claude_runner: &MockClaudeRunnerFailing,
            worktree: &worktree,
            file_gen: &file_gen,
            session_mgr: &mut session_mgr,
            init_mgr: &mut init_mgr,
            config: &config,
        };

        let _ = prepare_process_spawn(&ctx, &issue, ProcessRunType::ImplFix, &[], None).await;

        let calls = log.lock().expect("lock").clone();
        assert!(
            calls.contains(&"fetch".to_string()),
            "fetch should be attempted even when it fails, calls: {:?}",
            calls
        );
        assert!(
            calls.contains(&"merge".to_string()),
            "merge should be called even when fetch fails, calls: {:?}",
            calls
        );
    }

    /// T-fix.3: Design などの non-fixing spawn type では fetch が呼ばれないこと（要件 2.3）
    #[tokio::test]
    async fn non_fixing_spawn_does_not_call_fetch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");

        let mut issue = make_fixing_issue(wt_path);
        issue.state = crate::domain::state::State::DesignRunning;
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log.clone());
        let github = MockGitHub::with_body("Issue body");
        let proc_repo = MockProcRepo;
        let issue_repo = MockIssueRepoStub;
        let file_gen = MockFileGenerator;
        let mut session_mgr = SessionManager::new();
        let mut init_mgr = InitTaskManager::new();

        let ctx = ExecuteContext {
            github: &github,
            issue_repo: &issue_repo,
            process_repo: &proc_repo,
            claude_runner: &MockClaudeRunnerFailing,
            worktree: &worktree,
            file_gen: &file_gen,
            session_mgr: &mut session_mgr,
            init_mgr: &mut init_mgr,
            config: &config,
        };

        let _ = prepare_process_spawn(&ctx, &issue, ProcessRunType::Design, &[], None).await;

        let calls = log.lock().expect("lock").clone();
        assert!(
            !calls.contains(&"fetch".to_string()),
            "fetch should NOT be called for non-fixing spawn types, calls: {:?}",
            calls
        );
    }

    /// T-fix.4: DesignFix spawn では fetch/merge が呼ばれないこと
    #[tokio::test]
    async fn design_fixing_spawn_does_not_call_fetch_or_merge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wt_path = dir.path().to_str().expect("utf8");
        let issue = make_fixing_issue(wt_path);
        let config = make_test_config(dir.path());

        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log.clone());
        let github = MockGitHub::with_body("Issue body");
        let proc_repo = MockProcRepo;
        let issue_repo = MockIssueRepoStub;
        let file_gen = MockFileGenerator;
        let mut session_mgr = SessionManager::new();
        let mut init_mgr = InitTaskManager::new();

        let ctx = ExecuteContext {
            github: &github,
            issue_repo: &issue_repo,
            process_repo: &proc_repo,
            claude_runner: &MockClaudeRunnerFailing,
            worktree: &worktree,
            file_gen: &file_gen,
            session_mgr: &mut session_mgr,
            init_mgr: &mut init_mgr,
            config: &config,
        };

        let _ = prepare_process_spawn(&ctx, &issue, ProcessRunType::DesignFix, &[], None).await;

        let calls = log.lock().expect("lock").clone();
        assert!(
            !calls.contains(&"fetch".to_string()),
            "fetch should NOT be called for DesignFix spawn, calls: {:?}",
            calls
        );
        assert!(
            !calls.contains(&"merge".to_string()),
            "merge should NOT be called for DesignFix spawn, calls: {:?}",
            calls
        );
    }

    /// T-4.3.match: body_hash が一致する場合は spawn が成功（改変なし）
    #[tokio::test]
    async fn prepare_process_spawn_succeeds_when_hash_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = "Approved issue body";
        let expected_hash = sha256_hex(body);

        let github = MockGitHub::with_body(body);
        let proc_repo = MockProcRepo;
        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log);
        let config = make_test_config(dir.path());
        let issue_repo = MockIssueRepoStub;
        let file_gen = MockFileGenerator;
        let mut session_mgr = SessionManager::new();
        let mut init_mgr = InitTaskManager::new();
        let mut issue = make_test_issue("issue-1");
        issue.state = crate::domain::state::State::DesignRunning;
        issue.worktree_path = Some(dir.path().to_string_lossy().into_owned());
        issue.body_hash = Some(expected_hash);

        let ctx = ExecuteContext {
            github: &github,
            issue_repo: &issue_repo,
            process_repo: &proc_repo,
            claude_runner: &MockClaudeRunnerFailing,
            worktree: &worktree,
            file_gen: &file_gen,
            session_mgr: &mut session_mgr,
            init_mgr: &mut init_mgr,
            config: &config,
        };

        let result = prepare_process_spawn(&ctx, &issue, ProcessRunType::Design, &[], None).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.downcast_ref::<super::super::BodyTamperedError>()
                .is_none(),
            "should not be BodyTamperedError when hash matches"
        );
    }

    /// T-4.3.tamper: body_hash が不一致の場合は BodyTamperedError が返る
    #[tokio::test]
    async fn prepare_process_spawn_returns_body_tampered_error_when_hash_differs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let github = MockGitHub::with_body("Tampered body content");
        let proc_repo = MockProcRepo;
        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log);
        let config = make_test_config(dir.path());
        let issue_repo = MockIssueRepoStub;
        let file_gen = MockFileGenerator;
        let mut session_mgr = SessionManager::new();
        let mut init_mgr = InitTaskManager::new();
        let mut issue = make_test_issue("issue-1");
        issue.worktree_path = Some(dir.path().to_string_lossy().into_owned());
        issue.body_hash = Some(sha256_hex("Original approved body"));

        let ctx = ExecuteContext {
            github: &github,
            issue_repo: &issue_repo,
            process_repo: &proc_repo,
            claude_runner: &MockClaudeRunnerFailing,
            worktree: &worktree,
            file_gen: &file_gen,
            session_mgr: &mut session_mgr,
            init_mgr: &mut init_mgr,
            config: &config,
        };

        let result = prepare_process_spawn(&ctx, &issue, ProcessRunType::Design, &[], None).await;

        assert!(result.is_err(), "should fail when hash differs");
        let err = result.unwrap_err();
        assert!(
            err.downcast_ref::<super::super::BodyTamperedError>()
                .is_some(),
            "should be BodyTamperedError when body was tampered"
        );
    }

    /// T-4.3.none: body_hash が None の場合はハッシュ比較をスキップして spawn が続行
    #[tokio::test]
    async fn prepare_process_spawn_skips_hash_check_when_body_hash_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let github = MockGitHub::with_body("Any body content");
        let proc_repo = MockProcRepo;
        let log: CallLog = Arc::new(Mutex::new(vec![]));
        let worktree = MockGitWorktree::with_log(log);
        let config = make_test_config(dir.path());
        let issue_repo = MockIssueRepoStub;
        let file_gen = MockFileGenerator;
        let mut session_mgr = SessionManager::new();
        let mut init_mgr = InitTaskManager::new();
        let mut issue = make_test_issue("issue-1");
        issue.state = crate::domain::state::State::DesignRunning;
        issue.worktree_path = Some(dir.path().to_string_lossy().into_owned());

        let ctx = ExecuteContext {
            github: &github,
            issue_repo: &issue_repo,
            process_repo: &proc_repo,
            claude_runner: &MockClaudeRunnerFailing,
            worktree: &worktree,
            file_gen: &file_gen,
            session_mgr: &mut session_mgr,
            init_mgr: &mut init_mgr,
            config: &config,
        };

        let result = prepare_process_spawn(&ctx, &issue, ProcessRunType::Design, &[], None).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.downcast_ref::<super::super::BodyTamperedError>()
                .is_none(),
            "should not be BodyTamperedError when body_hash is None"
        );
    }
}
