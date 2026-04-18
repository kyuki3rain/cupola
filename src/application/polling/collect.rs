/// Collect phase: observe GitHub + DB state → build WorldSnapshot per issue.
///
/// This phase is pure observation — no DB writes, no GitHub mutations
/// (except for discovery of new Idle issues).
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::domain::config::Config;
use crate::domain::issue::Issue;
use crate::domain::process_run::ProcessRunType;
use crate::domain::state::State;
use crate::domain::world_snapshot::{
    CiStatus, GithubIssueSnapshot, PrSnapshot, PrState, ProcessSnapshot, ProcessesSnapshot,
    WorldSnapshot,
};

use crate::application::polling_use_case::label_to_weight;

/// Observation result for a single issue.
pub struct IssueObservation {
    pub issue: Issue,
    pub snapshot: WorldSnapshot,
}

/// Collect observations for every issue tracked in the DB.
///
/// Uses `list_open_issues` to fetch all open issues in a single paginated call,
/// then matches against the DB to determine which issues need observation.
///
/// Terminal states (`Completed`, `Cancelled`) are included on purpose: Completed
/// still runs persistent effects (CleanupWorktree, CloseIssue) until the worktree
/// is gone and the issue is closed; Cancelled may transition back to Idle via the
/// `close_finished && github_issue is Open` rule. Issues that fail to observe
/// (e.g. GitHub API 5xx) are skipped and logged.
pub async fn collect_all<G, I, P>(
    github: &G,
    issue_repo: &I,
    process_repo: &P,
    config: &Config,
) -> Result<Vec<IssueObservation>>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
{
    // Step 1: Fetch all open issues (single paginated REST call).
    // Build a set of open issue numbers and a map of issue_number → labels.
    // On failure, propagate the error to abort collect (and thus decide/execute).
    // The resolve phase has already completed at this point, so async process
    // resolution is unaffected. Falling back to an empty open set would cause
    // all active issues to be observed as Closed, triggering unintended
    // Cancelled transitions.
    let open_issues = github.list_open_issues().await?;
    let mut open_numbers = HashSet::new();
    let mut labels_map = HashMap::new();
    for oi in open_issues {
        open_numbers.insert(oi.number);
        labels_map.insert(oi.number, oi.labels);
    }

    // Step 2: Discovery — find open issues with agent:ready that are not yet in DB.
    const READY_LABEL: &str = "agent:ready";
    for (&issue_number, labels) in &labels_map {
        if !labels.iter().any(|l| l == READY_LABEL) {
            continue;
        }
        match issue_repo.find_by_issue_number(issue_number).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                let issue = Issue::new(issue_number, format!("issue-{issue_number}"));
                if let Err(e) = issue_repo.save(&issue).await {
                    tracing::warn!(
                        issue_number,
                        error = %e,
                        "failed to insert newly-discovered ready issue"
                    );
                } else {
                    tracing::info!(issue_number, "discovered new ready issue, inserted as Idle");
                }
            }
            Err(e) => tracing::warn!(
                issue_number,
                error = %e,
                "find_by_issue_number failed during discovery"
            ),
        }
    }

    // Step 3: Load all tracked issues from DB and build observations.
    let issues = issue_repo.find_all().await?;

    let mut observations = Vec::new();
    for issue in issues {
        let n = issue.github_issue_number;
        let is_open = open_numbers.contains(&n);
        let is_terminal = issue.state.is_terminal();
        let is_converged = issue.worktree_path.is_none() && issue.close_finished;

        // Skip fully-converged terminal issues that are closed
        if is_terminal && is_converged && !is_open {
            continue;
        }

        let labels = if is_open {
            labels_map.get(&n).map(|v| v.as_slice())
        } else {
            None
        };

        match observe_issue(github, process_repo, config, &issue, is_open, labels).await {
            Ok(snapshot) => {
                observations.push(IssueObservation { issue, snapshot });
            }
            Err(e) => {
                tracing::warn!(
                    issue_number = n,
                    error = %e,
                    "collect failed for issue, skipping this cycle"
                );
            }
        }
    }
    Ok(observations)
}

/// Build a WorldSnapshot for a single issue.
///
/// `is_open`: whether this issue number was found in the `list_open_issues` result.
/// `labels`: the issue's labels (from `list_open_issues`), or `None` if closed.
async fn observe_issue<G, P>(
    github: &G,
    process_repo: &P,
    config: &Config,
    issue: &Issue,
    is_open: bool,
    labels: Option<&[String]>,
) -> Result<WorldSnapshot>
where
    G: GitHubClient,
    P: ProcessRunRepository,
{
    // --- GithubIssueSnapshot ---
    let github_issue = observe_github_issue(
        github,
        config,
        issue.github_issue_number,
        issue.state,
        is_open,
        labels,
    )
    .await?;

    // --- PR observation ---
    // Always observe PRs for active issues (even if closed — merged impl PR → Completed).
    // For terminal + not converged issues, observe PRs for retry (CleanupWorktree/CloseIssue).
    // For terminal + converged + open, skip PR observe (decide handles Cancelled→Idle).
    let is_terminal = issue.state.is_terminal();
    let is_converged = issue.worktree_path.is_none() && issue.close_finished;
    let should_observe_prs = !is_terminal || !is_converged;

    let (design_pr, impl_pr) = if should_observe_prs {
        let d = observe_pr_for_type(github, process_repo, config, issue, ProcessRunType::Design)
            .await?;
        let i =
            observe_pr_for_type(github, process_repo, config, issue, ProcessRunType::Impl).await?;
        (d, i)
    } else {
        (None, None)
    };

    // --- ProcessesSnapshot ---
    let processes = observe_processes(process_repo, issue).await?;

    // --- ci_fix_exhausted ---
    let ci_fix_exhausted = issue.ci_fix_count >= config.max_ci_fix_cycles;

    Ok(WorldSnapshot {
        github_issue,
        design_pr,
        impl_pr,
        processes,
        ci_fix_exhausted,
    })
}

/// Build a GithubIssueSnapshot from pre-fetched open-set membership and labels.
///
/// No per-issue GitHub API calls are made here (except `check_label_actor`
/// for open Idle issues with the ready label).
async fn observe_github_issue<G: GitHubClient>(
    github: &G,
    config: &Config,
    issue_number: u64,
    issue_state: State,
    is_open: bool,
    labels: Option<&[String]>,
) -> Result<GithubIssueSnapshot> {
    if !is_open {
        return Ok(GithubIssueSnapshot::Closed);
    }

    let labels = labels.unwrap_or(&[]);

    const READY_LABEL: &str = "agent:ready";
    let has_ready_label = labels.iter().any(|l| l == READY_LABEL);

    let ready_label_trusted = if !has_ready_label || issue_state != State::Idle {
        // Non-Idle issues skip check_label_actor: the result is only consumed by
        // decide_idle, so calling the API for active/terminal issues wastes 2 API
        // calls per cycle with no effect on behaviour.
        false
    } else {
        use crate::application::association_guard::{AssociationCheckResult, check_label_actor};
        match check_label_actor(
            github,
            issue_number,
            READY_LABEL,
            &config.trusted_associations,
            &config.owner,
        )
        .await
        {
            Ok(AssociationCheckResult::Trusted) => true,
            Ok(AssociationCheckResult::Rejected { .. }) => false,
            Err(e) => {
                tracing::warn!(
                    issue_number,
                    error = %e,
                    "failed to check label actor association, treating as untrusted"
                );
                false
            }
        }
    };

    let weight = label_to_weight(labels);

    Ok(GithubIssueSnapshot::Open {
        has_ready_label,
        ready_label_trusted,
        weight: Some(weight),
    })
}

async fn observe_pr_for_type<G, P>(
    github: &G,
    process_repo: &P,
    config: &Config,
    issue: &Issue,
    type_: ProcessRunType,
) -> Result<Option<PrSnapshot>>
where
    G: GitHubClient,
    P: ProcessRunRepository,
{
    // Find latest process run with a pr_number (SQL-level filter ensures non-NULL)
    let run = match process_repo
        .find_latest_with_pr_number(issue.id, type_)
        .await?
    {
        Some(r) => r,
        None => return Ok(None),
    };
    let pr_number = run.pr_number.ok_or_else(|| {
        anyhow::anyhow!(
            "invariant violation: find_latest_with_pr_number returned a run with pr_number=None"
        )
    })?;

    // Single GraphQL call for all PR observation data
    let observation = match github.observe_pr(pr_number).await? {
        Some(obs) => obs,
        None => return Ok(None), // PR not found
    };

    use crate::application::port::github_client::PrStatus;
    let pr_state = match observation.state {
        PrStatus::Open => PrState::Open,
        PrStatus::Merged => PrState::Merged,
        PrStatus::Closed => PrState::Closed,
    };

    // For open PRs, derive review comments, CI status, and conflict from the observation
    let (has_review_comments, ci_status, has_conflict, newest_pr_review_submitted_at) =
        if pr_state == PrState::Open {
            let has_thread_comments = observation.unresolved_threads.iter().any(|t| {
                t.comments
                    .iter()
                    .any(|c| config.is_comment_trusted(&c.author_association, &c.author))
            });

            // PR レベルレビューをフィルタリング
            // - issue.last_pr_review_submitted_at より新しいもののみ
            // - 信頼済み author のみ
            let threshold = issue
                .last_pr_review_submitted_at
                .unwrap_or(DateTime::<Utc>::MIN_UTC);
            let new_pr_reviews: Vec<_> = observation
                .pr_level_reviews
                .iter()
                .filter(|r| r.submitted_at > threshold)
                .filter(|r| config.is_comment_trusted(&r.author_association, &r.author))
                .collect();

            let newest_pr_review_submitted_at = new_pr_reviews.iter().map(|r| r.submitted_at).max();

            if !new_pr_reviews.is_empty() {
                tracing::info!(
                    pr_number,
                    count = new_pr_reviews.len(),
                    newest = ?newest_pr_review_submitted_at,
                    "detected new PR-level reviews"
                );
            }

            let has_review_comments = has_thread_comments || !new_pr_reviews.is_empty();
            let ci = derive_ci_status(&observation.check_runs);
            let has_conflict = observation.mergeable == Some(false);

            (
                has_review_comments,
                ci,
                has_conflict,
                newest_pr_review_submitted_at,
            )
        } else {
            (false, CiStatus::Unknown, false, None)
        };

    Ok(Some(PrSnapshot {
        state: pr_state,
        has_review_comments,
        ci_status,
        has_conflict,
        newest_pr_review_submitted_at,
    }))
}

fn derive_ci_status(
    check_runs: &[crate::application::port::github_client::GitHubCheckRun],
) -> CiStatus {
    // Aggregation rules (docs/architecture/observations.md):
    //   failure | timed_out       → Failure (hard signal)
    //   cancelled | null (pending)→ Unknown (transient)
    //   success | neutral | skipped etc. → counts toward "all good"
    //   no check runs at all      → Unknown (no information)
    if check_runs.is_empty() {
        return CiStatus::Unknown;
    }

    let mut any_unknown = false;
    for run in check_runs {
        match run.conclusion.as_deref() {
            Some("failure") | Some("timed_out") => return CiStatus::Failure,
            Some("cancelled") | None => any_unknown = true,
            _ => {} // success, neutral, skipped — clean
        }
    }

    if any_unknown {
        CiStatus::Unknown
    } else {
        CiStatus::Ok
    }
}

async fn observe_processes<P: ProcessRunRepository>(
    process_repo: &P,
    issue: &Issue,
) -> Result<ProcessesSnapshot> {
    async fn get_process_snapshot<P: ProcessRunRepository>(
        process_repo: &P,
        issue_id: i64,
        type_: ProcessRunType,
        since: Option<DateTime<Utc>>,
    ) -> Result<Option<ProcessSnapshot>> {
        // Use the combined method to read both the latest run and the consecutive
        // failure count in a single database lock acquisition, preventing a concurrent
        // ProcessRun insert from producing an inconsistent (run, count) pair.
        let Some((run, consecutive_failures)) = process_repo
            .find_latest_with_consecutive_count(issue_id, type_, since)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(ProcessSnapshot {
            state: run.state,
            index: run.index,
            run_id: run.id,
            consecutive_failures,
        }))
    }

    let id = issue.id;
    let since = issue.consecutive_failures_epoch;
    let init = get_process_snapshot(process_repo, id, ProcessRunType::Init, since).await?;
    let design = get_process_snapshot(process_repo, id, ProcessRunType::Design, since).await?;
    let design_fix =
        get_process_snapshot(process_repo, id, ProcessRunType::DesignFix, since).await?;
    let impl_ = get_process_snapshot(process_repo, id, ProcessRunType::Impl, since).await?;
    let impl_fix = get_process_snapshot(process_repo, id, ProcessRunType::ImplFix, since).await?;

    Ok(ProcessesSnapshot {
        init,
        design,
        design_fix,
        impl_,
        impl_fix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::github_client::{
        GitHubCheckRun, GitHubIssueDetail, GitHubPr, GitHubPrDetails, PrStatus, ReviewThread,
    };
    use anyhow::Result;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    struct MockGithub {
        /// Set to true when fetch_label_actor_login is called.
        label_actor_called: Arc<AtomicBool>,
    }

    impl MockGithub {
        fn with_tracker(tracker: Arc<AtomicBool>) -> Self {
            Self {
                label_actor_called: tracker,
            }
        }
    }

    impl GitHubClient for MockGithub {
        async fn list_open_issues(
            &self,
        ) -> Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }
        async fn get_issue(&self, n: u64) -> Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: n,
                title: format!("issue-{n}"),
                body: String::new(),
                labels: vec![],
            })
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
            Ok(0)
        }
        async fn reply_to_thread(&self, _id: &str, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn resolve_thread(&self, _id: &str) -> Result<()> {
            Ok(())
        }
        async fn comment_on_issue(&self, _n: u64, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn close_issue(&self, _n: u64) -> Result<()> {
            Ok(())
        }
        async fn get_job_logs(&self, _id: u64) -> Result<String> {
            Ok(String::new())
        }
        async fn get_pr_details(&self, _n: u64) -> Result<GitHubPrDetails> {
            Ok(GitHubPrDetails {
                merged: false,
                mergeable: Some(true),
            })
        }
        async fn fetch_label_actor_login(&self, _n: u64, _label: &str) -> Result<Option<String>> {
            self.label_actor_called.store(true, Ordering::SeqCst);
            Ok(None)
        }
        async fn fetch_user_permission(
            &self,
            _user: &str,
        ) -> Result<crate::application::port::github_client::RepositoryPermission> {
            Ok(crate::application::port::github_client::RepositoryPermission::Admin)
        }
        async fn remove_label(&self, _n: u64, _label: &str) -> Result<()> {
            Ok(())
        }
        async fn observe_pr(
            &self,
            _n: u64,
        ) -> Result<Option<crate::application::port::github_client::PrObservation>> {
            Ok(None)
        }
    }

    fn test_config() -> Config {
        let mut config =
            Config::default_with_repo("owner".to_string(), "repo".to_string(), "main".to_string());
        config.trusted_associations = crate::domain::author_association::TrustedAssociations::All;
        config
    }

    /// closed issue → Closed snapshot, API 未呼び出し（issue_state は任意の値でよい）
    #[tokio::test]
    async fn closed_issue_returns_closed_snapshot() {
        let tracker = Arc::new(AtomicBool::new(false));
        let github = MockGithub::with_tracker(Arc::clone(&tracker));
        let config = test_config();

        // Closed issue: is_open=false, labels=None
        let snapshot = observe_github_issue(&github, &config, 1, State::Idle, false, None)
            .await
            .unwrap();

        assert!(matches!(snapshot, GithubIssueSnapshot::Closed));
        assert!(
            !tracker.load(Ordering::SeqCst),
            "fetch_label_actor_login must NOT be called for closed issues"
        );
    }

    /// Idle + ready label → API 呼び出しあり（既存動作の回帰）
    #[tokio::test]
    async fn open_issue_with_ready_label_calls_permission_api() {
        let tracker = Arc::new(AtomicBool::new(false));
        let github = MockGithub::with_tracker(Arc::clone(&tracker));
        // Use Specific associations so the API is actually called.
        let mut config = test_config();
        config.trusted_associations =
            crate::domain::author_association::TrustedAssociations::Specific(vec![
                crate::domain::author_association::AuthorAssociation::Owner,
            ]);

        let labels = vec!["agent:ready".to_string()];
        let snapshot = observe_github_issue(&github, &config, 1, State::Idle, true, Some(&labels))
            .await
            .unwrap();

        assert!(matches!(
            snapshot,
            GithubIssueSnapshot::Open {
                has_ready_label: true,
                ready_label_trusted: false,
                ..
            }
        ));
        assert!(
            tracker.load(Ordering::SeqCst),
            "fetch_label_actor_login MUST be called for Idle open issues with ready label"
        );
    }

    /// Idle + ラベルなし → ready_label_trusted == false, API 未呼び出し
    #[tokio::test]
    async fn open_issue_without_ready_label_is_not_trusted() {
        let tracker = Arc::new(AtomicBool::new(false));
        let github = MockGithub::with_tracker(Arc::clone(&tracker));
        let config = test_config();

        let labels: Vec<String> = vec![];
        let snapshot = observe_github_issue(&github, &config, 1, State::Idle, true, Some(&labels))
            .await
            .unwrap();

        assert!(matches!(
            snapshot,
            GithubIssueSnapshot::Open {
                has_ready_label: false,
                ready_label_trusted: false,
                ..
            }
        ));
        assert!(
            !tracker.load(Ordering::SeqCst),
            "fetch_label_actor_login must NOT be called when no ready label"
        );
    }

    /// 非 Idle 全状態 + ready label → API 未呼び出し（Req 1.2, 3.1）
    #[tokio::test]
    async fn non_idle_states_with_ready_label_skip_api() {
        use crate::domain::state::State;

        let non_idle_states: Vec<State> = State::all()
            .into_iter()
            .filter(|s| *s != State::Idle)
            .collect();

        for issue_state in non_idle_states {
            let tracker = Arc::new(AtomicBool::new(false));
            let github = MockGithub::with_tracker(Arc::clone(&tracker));
            let mut config = test_config();
            // Use Specific so that if the API were called it would actually be invoked.
            config.trusted_associations =
                crate::domain::author_association::TrustedAssociations::Specific(vec![
                    crate::domain::author_association::AuthorAssociation::Owner,
                ]);

            let labels = vec!["agent:ready".to_string()];
            let snapshot =
                observe_github_issue(&github, &config, 1, issue_state, true, Some(&labels))
                    .await
                    .unwrap();

            assert!(
                matches!(
                    snapshot,
                    GithubIssueSnapshot::Open {
                        has_ready_label: true,
                        ready_label_trusted: false,
                        ..
                    }
                ),
                "state {issue_state:?}: expected Open with ready_label_trusted=false"
            );
            assert!(
                !tracker.load(Ordering::SeqCst),
                "state {issue_state:?}: fetch_label_actor_login must NOT be called for non-Idle issues"
            );
        }
    }

    /// T-3.CO.1: derive_ci_status returns Failure on failure conclusion
    #[test]
    fn ci_status_failure_on_failure_conclusion() {
        let runs = vec![GitHubCheckRun {
            id: 1,
            name: "ci".into(),
            status: "completed".into(),
            conclusion: Some("failure".into()),
            output_summary: None,
            output_text: None,
        }];
        assert_eq!(derive_ci_status(&runs), CiStatus::Failure);
    }

    /// derive_ci_status returns Ok when every check run is a clean success.
    #[test]
    fn ci_status_ok_when_all_success() {
        let runs = vec![GitHubCheckRun {
            id: 1,
            name: "ci".into(),
            status: "completed".into(),
            conclusion: Some("success".into()),
            output_summary: None,
            output_text: None,
        }];
        assert_eq!(derive_ci_status(&runs), CiStatus::Ok);
    }

    /// derive_ci_status returns Unknown on cancelled (transient — a new push
    /// likely auto-cancelled the run; wait for the next round).
    #[test]
    fn ci_status_unknown_on_cancelled() {
        let runs = vec![GitHubCheckRun {
            id: 1,
            name: "ci".into(),
            status: "completed".into(),
            conclusion: Some("cancelled".into()),
            output_summary: None,
            output_text: None,
        }];
        assert_eq!(derive_ci_status(&runs), CiStatus::Unknown);
    }

    /// derive_ci_status returns Unknown while a check run is still in progress.
    #[test]
    fn ci_status_unknown_while_in_progress() {
        let runs = vec![GitHubCheckRun {
            id: 1,
            name: "ci".into(),
            status: "in_progress".into(),
            conclusion: None,
            output_summary: None,
            output_text: None,
        }];
        assert_eq!(derive_ci_status(&runs), CiStatus::Unknown);
    }

    /// derive_ci_status returns Unknown when there are no check runs at all.
    #[test]
    fn ci_status_unknown_when_no_runs() {
        assert_eq!(derive_ci_status(&[]), CiStatus::Unknown);
    }

    /// If any run is in progress, the aggregation stays Unknown even when
    /// another run has already succeeded.
    #[test]
    fn ci_status_unknown_when_one_in_progress_one_success() {
        let runs = vec![
            GitHubCheckRun {
                id: 1,
                name: "test".into(),
                status: "completed".into(),
                conclusion: Some("success".into()),
                output_summary: None,
                output_text: None,
            },
            GitHubCheckRun {
                id: 2,
                name: "lint".into(),
                status: "in_progress".into(),
                conclusion: None,
                output_summary: None,
                output_text: None,
            },
        ];
        assert_eq!(derive_ci_status(&runs), CiStatus::Unknown);
    }

    // ── observe_pr_for_type unit tests ──────────────────────────────────────

    use crate::domain::process_run::{ProcessRun, ProcessRunState, ProcessRunType};
    use std::sync::Mutex;

    struct MockProcessRunRepository {
        /// Result returned by find_latest_with_pr_number.
        find_latest_with_pr_number_result: Option<ProcessRun>,
    }

    impl MockProcessRunRepository {
        fn returning_none() -> Self {
            Self {
                find_latest_with_pr_number_result: None,
            }
        }

        fn returning_run(run: ProcessRun) -> Self {
            Self {
                find_latest_with_pr_number_result: Some(run),
            }
        }
    }

    impl crate::application::port::process_run_repository::ProcessRunRepository
        for MockProcessRunRepository
    {
        async fn save(&self, _: &ProcessRun) -> Result<i64> {
            Ok(0)
        }
        async fn update_pid(&self, _: i64, _: u32) -> Result<()> {
            Ok(())
        }
        async fn mark_succeeded(&self, _: i64, _: Option<u64>) -> Result<()> {
            Ok(())
        }
        async fn mark_failed(&self, _: i64, _: Option<String>) -> Result<()> {
            Ok(())
        }
        async fn mark_stale(&self, _: i64) -> Result<()> {
            Ok(())
        }
        async fn mark_stale_for_issue(&self, _: i64) -> Result<()> {
            Ok(())
        }
        async fn find_latest(&self, _: i64, _: ProcessRunType) -> Result<Option<ProcessRun>> {
            Ok(None)
        }
        async fn find_latest_with_pr_number(
            &self,
            _: i64,
            _: ProcessRunType,
        ) -> Result<Option<ProcessRun>> {
            Ok(self.find_latest_with_pr_number_result.clone())
        }
        async fn find_by_issue(&self, _: i64) -> Result<Vec<ProcessRun>> {
            Ok(vec![])
        }
        async fn count_consecutive_failures(
            &self,
            _: i64,
            _: ProcessRunType,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> Result<u32> {
            Ok(0)
        }
        async fn find_latest_with_consecutive_count(
            &self,
            _: i64,
            _: ProcessRunType,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> Result<Option<(ProcessRun, u32)>> {
            Ok(None)
        }
        async fn find_all_running(&self) -> Result<Vec<ProcessRun>> {
            Ok(vec![])
        }
        async fn update_state(&self, _: i64, _: ProcessRunState) -> Result<()> {
            Ok(())
        }
    }

    fn make_run_with_pr_number(pr_number: u64) -> ProcessRun {
        let mut run = ProcessRun::new_running(1, ProcessRunType::Design, 0, vec![]);
        run.pr_number = Some(pr_number);
        run
    }

    fn make_test_issue(id: i64) -> crate::domain::issue::Issue {
        let mut issue = crate::domain::issue::Issue::new(1, "test".to_string());
        issue.id = id;
        issue
    }

    /// T-4.CO.1: find_latest_with_pr_number が None を返す場合、
    /// observe_pr_for_type は Ok(None) を返す
    #[tokio::test]
    async fn observe_pr_for_type_returns_none_when_no_pr_run() {
        let tracker = Arc::new(AtomicBool::new(false));
        let github = MockGithub::with_tracker(Arc::clone(&tracker));
        let repo = MockProcessRunRepository::returning_none();
        let issue = make_test_issue(1);

        let config = test_config();
        let result = observe_pr_for_type(&github, &repo, &config, &issue, ProcessRunType::Design)
            .await
            .expect("should not error");

        assert!(
            result.is_none(),
            "expected None when no pr_number run exists"
        );
    }

    /// T-4.CO.2: find_latest_with_pr_number が Some(run) (pr_number=42) を返す場合、
    /// github.observe_pr(42) が呼ばれて PrSnapshot が返る
    #[tokio::test]
    async fn observe_pr_for_type_calls_observe_pr_with_pr_number() {
        let observe_pr_calls = Arc::new(Mutex::new(vec![]));
        let run = make_run_with_pr_number(42);

        let github = MockGithubWithPrTracking::new(Arc::clone(&observe_pr_calls));
        let repo = MockProcessRunRepository::returning_run(run);
        let issue = make_test_issue(1);

        let config = test_config();
        let result = observe_pr_for_type(&github, &repo, &config, &issue, ProcessRunType::Design)
            .await
            .expect("should not error");

        assert!(
            result.is_some(),
            "expected Some(PrSnapshot) for pr_number=42"
        );
        let calls = observe_pr_calls.lock().unwrap();
        assert_eq!(
            calls.as_slice(),
            &[42u64],
            "observe_pr should be called with 42"
        );
    }

    /// T-4.CO.3: github.observe_pr が None を返す場合（PR が存在しない）、
    /// observe_pr_for_type は Ok(None) を返す
    #[tokio::test]
    async fn observe_pr_for_type_returns_none_when_pr_not_found() {
        let observe_pr_calls = Arc::new(Mutex::new(vec![]));
        let run = make_run_with_pr_number(99);

        let github = MockGithubWith404::new(Arc::clone(&observe_pr_calls));
        let repo = MockProcessRunRepository::returning_run(run);
        let issue = make_test_issue(1);

        let config = test_config();
        let result = observe_pr_for_type(&github, &repo, &config, &issue, ProcessRunType::Design)
            .await
            .expect("should not error when PR not found");

        assert!(
            result.is_none(),
            "expected None when observe_pr returns None"
        );
    }

    /// MockGithub that records observe_pr calls and returns a PrObservation with PrStatus::Open
    struct MockGithubWithPrTracking {
        observe_pr_calls: Arc<Mutex<Vec<u64>>>,
    }

    impl MockGithubWithPrTracking {
        fn new(calls: Arc<Mutex<Vec<u64>>>) -> Self {
            Self {
                observe_pr_calls: calls,
            }
        }
    }

    impl GitHubClient for MockGithubWithPrTracking {
        async fn list_open_issues(
            &self,
        ) -> Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }
        async fn get_issue(&self, n: u64) -> Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: n,
                title: String::new(),
                body: String::new(),
                labels: vec![],
            })
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
            Ok(0)
        }
        async fn reply_to_thread(&self, _id: &str, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn resolve_thread(&self, _id: &str) -> Result<()> {
            Ok(())
        }
        async fn comment_on_issue(&self, _n: u64, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn close_issue(&self, _n: u64) -> Result<()> {
            Ok(())
        }
        async fn get_job_logs(&self, _id: u64) -> Result<String> {
            Ok(String::new())
        }
        async fn get_pr_details(&self, _n: u64) -> Result<GitHubPrDetails> {
            Ok(GitHubPrDetails {
                merged: false,
                mergeable: Some(true),
            })
        }
        async fn fetch_label_actor_login(&self, _n: u64, _label: &str) -> Result<Option<String>> {
            Ok(None)
        }
        async fn fetch_user_permission(
            &self,
            _user: &str,
        ) -> Result<crate::application::port::github_client::RepositoryPermission> {
            Ok(crate::application::port::github_client::RepositoryPermission::Admin)
        }
        async fn remove_label(&self, _n: u64, _label: &str) -> Result<()> {
            Ok(())
        }
        async fn observe_pr(
            &self,
            n: u64,
        ) -> Result<Option<crate::application::port::github_client::PrObservation>> {
            self.observe_pr_calls.lock().unwrap().push(n);
            Ok(Some(
                crate::application::port::github_client::PrObservation {
                    state: PrStatus::Open,
                    mergeable: Some(true),
                    unresolved_threads: vec![],
                    check_runs: vec![],
                    pr_level_reviews: vec![],
                },
            ))
        }
    }

    /// MockGithub that returns None from observe_pr (PR not found)
    struct MockGithubWith404 {
        #[expect(dead_code)]
        observe_pr_calls: Arc<Mutex<Vec<u64>>>,
    }

    impl MockGithubWith404 {
        fn new(calls: Arc<Mutex<Vec<u64>>>) -> Self {
            Self {
                observe_pr_calls: calls,
            }
        }
    }

    impl GitHubClient for MockGithubWith404 {
        async fn list_open_issues(
            &self,
        ) -> Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }
        async fn get_issue(&self, n: u64) -> Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: n,
                title: String::new(),
                body: String::new(),
                labels: vec![],
            })
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
            Ok(0)
        }
        async fn reply_to_thread(&self, _id: &str, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn resolve_thread(&self, _id: &str) -> Result<()> {
            Ok(())
        }
        async fn comment_on_issue(&self, _n: u64, _body: &str) -> Result<()> {
            Ok(())
        }
        async fn close_issue(&self, _n: u64) -> Result<()> {
            Ok(())
        }
        async fn get_job_logs(&self, _id: u64) -> Result<String> {
            Ok(String::new())
        }
        async fn get_pr_details(&self, _n: u64) -> Result<GitHubPrDetails> {
            Ok(GitHubPrDetails {
                merged: false,
                mergeable: Some(true),
            })
        }
        async fn fetch_label_actor_login(&self, _n: u64, _label: &str) -> Result<Option<String>> {
            Ok(None)
        }
        async fn fetch_user_permission(
            &self,
            _user: &str,
        ) -> Result<crate::application::port::github_client::RepositoryPermission> {
            Ok(crate::application::port::github_client::RepositoryPermission::Admin)
        }
        async fn remove_label(&self, _n: u64, _label: &str) -> Result<()> {
            Ok(())
        }
        async fn observe_pr(
            &self,
            _n: u64,
        ) -> Result<Option<crate::application::port::github_client::PrObservation>> {
            Ok(None)
        }
    }
}
