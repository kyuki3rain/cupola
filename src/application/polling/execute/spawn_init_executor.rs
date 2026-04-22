use std::path::Path;

use anyhow::Result;

use crate::application::port::file_generator::FileGenerator;
use crate::application::port::git_worktree::GitWorktree;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::domain::config::Config;
use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::process_run::{ProcessRun, ProcessRunType};

use super::context::ExecuteContext;
use super::{SpawnableGitWorktree, sha256_hex};

pub(super) async fn execute<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &mut Issue,
) -> Result<()>
where
    G: GitHubClient,
    I: crate::application::port::issue_repository::IssueRepository,
    P: ProcessRunRepository,
    C: crate::application::port::claude_code_runner::ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    spawn_init_task(ctx, issue).await
}

async fn spawn_init_task<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &mut Issue,
) -> Result<()>
where
    G: GitHubClient,
    I: crate::application::port::issue_repository::IssueRepository,
    P: ProcessRunRepository,
    C: crate::application::port::claude_code_runner::ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    if !ctx.init_mgr.try_claim(issue.id) {
        tracing::debug!(
            issue_id = issue.id,
            "init task already active or pending, skipping"
        );
        return Ok(());
    }

    match prepare_init_handle(
        ctx.github,
        ctx.process_repo,
        ctx.worktree,
        ctx.file_gen,
        ctx.config,
        issue,
    )
    .await
    {
        Ok((handle, body_hash, run_id)) => {
            let updates = MetadataUpdates {
                body_hash: Some(Some(body_hash.clone())),
                ..Default::default()
            };
            if let Err(e) = ctx
                .issue_repo
                .update_state_and_metadata(issue.id, &updates)
                .await
            {
                handle.abort();
                let _ = ctx
                    .process_repo
                    .mark_failed(run_id, Some(e.to_string()))
                    .await;
                ctx.init_mgr.release_claim(issue.id);
                return Err(e);
            }
            issue.body_hash = Some(body_hash);
            ctx.init_mgr.register(issue.id, handle);
        }
        Err(e) => {
            ctx.init_mgr.release_claim(issue.id);
            return Err(e);
        }
    }
    Ok(())
}

async fn prepare_init_handle<G, P, W, F>(
    github: &G,
    process_repo: &P,
    worktree: &W,
    file_gen: &F,
    config: &Config,
    issue: &Issue,
) -> Result<(tokio::task::JoinHandle<anyhow::Result<String>>, String, i64)>
where
    G: GitHubClient,
    P: ProcessRunRepository,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    let latest = process_repo
        .find_latest(issue.id, ProcessRunType::Init)
        .await?;
    let index = latest.map(|r| r.index + 1).unwrap_or(0);

    let detail = github.get_issue(issue.github_issue_number).await?;
    let body_hash = sha256_hex(&detail.body);

    let run = ProcessRun::new_running(issue.id, ProcessRunType::Init, index, vec![]);
    let run_id = process_repo.save(&run).await?;

    let feature_name = issue.feature_name.clone();
    let worktree_base = config
        .log_dir
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("worktrees");
    let wt_path = worktree_base.join(&feature_name);
    let default_branch = config.default_branch.clone();
    let issue_number = issue.github_issue_number;
    let issue_body = detail.body;
    let language = config.language.clone();
    let worktree_clone = worktree.clone();
    let file_gen_clone = file_gen.clone();

    let is_resume = worktree.exists(&wt_path);

    let lang = &config.language;
    let comment = if is_resume {
        rust_i18n::t!("issue_comment.resuming_design", locale = lang)
    } else {
        rust_i18n::t!("issue_comment.design_starting", locale = lang)
    };
    if let Err(e) = github.comment_on_issue(issue_number, &comment).await {
        let _ = process_repo.mark_failed(run_id, Some(e.to_string())).await;
        return Err(e);
    }

    let handle = tokio::spawn(async move {
        let wt_path_for_task = wt_path.clone();
        tokio::task::spawn_blocking(move || {
            perform_init_sync(
                &worktree_clone,
                &file_gen_clone,
                &wt_path_for_task,
                issue_number,
                &issue_body,
                &language,
                &feature_name,
                &default_branch,
                is_resume,
            )?;
            Ok::<String, anyhow::Error>(wt_path_for_task.to_string_lossy().into_owned())
        })
        .await
        .map_err(|e| anyhow::anyhow!("init blocking task panicked: {e}"))?
    });

    Ok((handle, body_hash, run_id))
}

#[allow(clippy::too_many_arguments)]
fn perform_init_sync<W: GitWorktree, F: FileGenerator>(
    worktree: &W,
    file_gen: &F,
    wt_path: &Path,
    issue_number: u64,
    issue_body: &str,
    language: &str,
    feature_name: &str,
    default_branch: &str,
    is_resume: bool,
) -> Result<()> {
    worktree.fetch()?;

    if is_resume {
        tracing::info!(
            path = %wt_path.display(),
            "worktree already exists; resuming init with idempotent setup steps"
        );
    }

    let main_branch = format!("cupola/{feature_name}/main");
    let start_point = format!("origin/{default_branch}");
    worktree.create(wt_path, &main_branch, &start_point)?;
    worktree.push(wt_path, &main_branch)?;

    let design_branch = format!("cupola/{feature_name}/design");
    worktree.create_branch(wt_path, &design_branch)?;
    worktree.push(wt_path, &design_branch)?;
    file_gen.generate_spec_directory_at(wt_path, issue_number, issue_body, language)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use anyhow::Result;

    use super::{perform_init_sync, prepare_init_handle};
    use crate::application::port::file_generator::FileGenerator;
    use crate::application::port::git_worktree::GitWorktree;
    use crate::application::port::github_client::{
        GitHubIssueDetail, GitHubPr, GitHubPrDetails, RepositoryPermission, ReviewThread,
    };
    use crate::application::port::process_run_repository::ProcessRunRepository;
    use crate::domain::config::Config;
    use crate::domain::issue::Issue;
    use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};

    type CallLog = Arc<Mutex<Vec<String>>>;

    #[derive(Clone, Default)]
    pub(super) struct MockGitWorktree {
        pub calls: CallLog,
        pub worktree_exists: bool,
        pub fail_fetch: bool,
    }

    impl MockGitWorktree {
        pub fn with_log(calls: CallLog) -> Self {
            Self {
                calls,
                worktree_exists: false,
                fail_fetch: false,
            }
        }

        pub fn with_existing_worktree(calls: CallLog) -> Self {
            Self {
                calls,
                worktree_exists: true,
                fail_fetch: false,
            }
        }

        pub fn with_fail_fetch(calls: CallLog) -> Self {
            Self {
                calls,
                worktree_exists: false,
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
    pub(super) struct MockFileGenerator {
        pub calls: CallLog,
    }

    impl MockFileGenerator {
        pub fn with_log(calls: CallLog) -> Self {
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

    #[derive(Clone)]
    pub(super) struct MockGitHubForInit {
        comments: Arc<Mutex<Vec<String>>>,
        fail_comment: bool,
    }

    impl MockGitHubForInit {
        pub fn new() -> Self {
            Self {
                comments: Arc::new(Mutex::new(vec![])),
                fail_comment: false,
            }
        }

        pub fn with_failing_comment(mut self) -> Self {
            self.fail_comment = true;
            self
        }

        pub fn posted_comments(&self) -> Vec<String> {
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
    pub(super) struct MockProcRepo {
        mark_failed_calls: FailedRunLog,
    }

    impl MockProcRepo {
        pub fn new() -> Self {
            Self {
                mark_failed_calls: Arc::new(Mutex::new(vec![])),
            }
        }

        pub fn failed_runs(&self) -> Vec<(i64, Option<String>)> {
            self.mark_failed_calls.lock().expect("lock").clone()
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

    #[test]
    fn perform_init_sync_pushes_branches_and_generates_spec_in_worktree() {
        let log: CallLog = Arc::new(Mutex::new(Vec::new()));
        let worktree = MockGitWorktree::with_log(log.clone());
        let file_gen = MockFileGenerator::with_log(log.clone());
        let wt_path = Path::new("/tmp/cupola-worktree");

        perform_init_sync(
            &worktree,
            &file_gen,
            wt_path,
            193,
            "Issue body",
            "ja",
            "issue-193",
            "main",
            false,
        )
        .expect("perform init");

        assert_eq!(
            log.lock().expect("lock").clone(),
            vec![
                "fetch".to_string(),
                format!(
                    "create:{}:cupola/issue-193/main:origin/main",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/main", wt_path.display()),
                format!(
                    "create_branch:{}:cupola/issue-193/design",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/design", wt_path.display()),
                format!(
                    "generate_spec_directory_at:{}:193:Issue body:ja",
                    wt_path.display()
                ),
            ]
        );
    }

    /// T-3.EX.resume: when worktree already exists (is_resume=true), all idempotent
    /// setup steps still run — same sequence as fresh init.
    #[test]
    fn perform_init_sync_resume_runs_idempotent_steps() {
        let log: CallLog = Arc::new(Mutex::new(Vec::new()));
        let worktree = MockGitWorktree::with_existing_worktree(log.clone());
        let file_gen = MockFileGenerator::with_log(log.clone());
        let wt_path = Path::new("/tmp/cupola-worktree");

        perform_init_sync(
            &worktree,
            &file_gen,
            wt_path,
            193,
            "Issue body",
            "ja",
            "issue-193",
            "main",
            true,
        )
        .expect("perform init resume");

        assert_eq!(
            log.lock().expect("lock").clone(),
            vec![
                "fetch".to_string(),
                format!(
                    "create:{}:cupola/issue-193/main:origin/main",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/main", wt_path.display()),
                format!(
                    "create_branch:{}:cupola/issue-193/design",
                    wt_path.display()
                ),
                format!("push:{}:cupola/issue-193/design", wt_path.display()),
                format!(
                    "generate_spec_directory_at:{}:193:Issue body:ja",
                    wt_path.display()
                ),
            ]
        );
    }

    /// T-3.EX.ph.fresh: prepare_init_handle posts `design_starting` comment when
    /// the worktree does not yet exist.
    #[tokio::test]
    async fn prepare_init_handle_posts_design_starting_on_fresh_init() {
        let dir = tempfile::tempdir().unwrap();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktree::with_log(Arc::new(Mutex::new(vec![])));
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let issue = make_test_issue("issue-1");
        let config = make_test_config(dir.path());

        prepare_init_handle(&github, &proc_repo, &worktree, &file_gen, &config, &issue)
            .await
            .expect("prepare_init_handle should succeed");

        let comments = github.posted_comments();
        assert_eq!(comments.len(), 1);
        assert!(
            !comments[0].contains("resuming"),
            "fresh init should not post a resuming comment, got: {:?}",
            comments[0]
        );
    }

    /// T-3.EX.ph.resume: prepare_init_handle posts `resuming_design` comment when
    /// the worktree path already exists (Cancelled → reopen scenario).
    #[tokio::test]
    async fn prepare_init_handle_posts_resuming_design_on_existing_worktree() {
        let dir = tempfile::tempdir().unwrap();
        let github = MockGitHubForInit::new();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktree::with_existing_worktree(Arc::new(Mutex::new(vec![])));
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let issue = make_test_issue("issue-1");
        let config = make_test_config(dir.path());

        prepare_init_handle(&github, &proc_repo, &worktree, &file_gen, &config, &issue)
            .await
            .expect("prepare_init_handle should succeed");

        let comments = github.posted_comments();
        assert_eq!(comments.len(), 1);
        assert!(
            comments[0].contains("再開"),
            "resume init should post a resuming comment (再開), got: {:?}",
            comments[0]
        );
    }

    /// T-3.EX.ph.comment-fail: when comment_on_issue fails, prepare_init_handle
    /// marks the ProcessRun as failed and propagates the error.
    #[tokio::test]
    async fn prepare_init_handle_marks_run_failed_on_comment_error() {
        let dir = tempfile::tempdir().unwrap();
        let github = MockGitHubForInit::new().with_failing_comment();
        let proc_repo = MockProcRepo::new();
        let worktree = MockGitWorktree::with_log(Arc::new(Mutex::new(vec![])));
        let file_gen = MockFileGenerator::with_log(Arc::new(Mutex::new(vec![])));
        let issue = make_test_issue("issue-1");
        let config = make_test_config(dir.path());

        let result =
            prepare_init_handle(&github, &proc_repo, &worktree, &file_gen, &config, &issue).await;

        assert!(result.is_err(), "should propagate comment error");
        let failed = proc_repo.failed_runs();
        assert_eq!(failed.len(), 1, "should mark exactly one run as failed");
        assert_eq!(failed[0].0, 42, "should use the saved run_id");
        assert!(
            failed[0]
                .1
                .as_deref()
                .unwrap_or("")
                .contains("comment API error"),
            "error message should be propagated"
        );
    }
}
