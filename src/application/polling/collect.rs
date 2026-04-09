/// Collect phase: observe GitHub + DB state → build WorldSnapshot per issue.
///
/// This phase is pure observation — no DB writes, no GitHub mutations.
use anyhow::Result;

use crate::application::port::github_client::{GitHubClient, PrStatus};
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::domain::config::Config;
use crate::domain::issue::Issue;
use crate::domain::process_run::ProcessRunType;
use crate::domain::world_snapshot::{
    CiStatus, GithubIssueSnapshot, GithubIssueState, PrSnapshot, PrState, ProcessSnapshot,
    ProcessesSnapshot, WorldSnapshot,
};

use crate::application::polling_use_case::label_to_weight;

/// Observation result for a single issue.
pub struct IssueObservation {
    pub issue: Issue,
    pub snapshot: WorldSnapshot,
}

/// Collect observations for every issue tracked in the DB.
///
/// Terminal states (`Completed`, `Cancelled`) are included on purpose: Completed
/// still runs persistent effects (CleanupWorktree, CloseIssue) until the worktree
/// is gone and the issue is closed; Cancelled may transition back to Idle via the
/// `close_finished && github_issue.state == open` rule. Issues that fail to observe
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
    // Discover newly-labeled GitHub issues and upsert them into the DB as Idle.
    // Without this step the DB stays empty forever and the polling loop has nothing
    // to observe. list_ready_issues returns issues currently carrying agent:ready.
    match github.list_ready_issues().await {
        Ok(ready) => {
            for gi in ready {
                match issue_repo.find_by_issue_number(gi.number).await {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        let issue = Issue::new(gi.number, format!("issue-{}", gi.number));
                        if let Err(e) = issue_repo.save(&issue).await {
                            tracing::warn!(
                                issue_number = gi.number,
                                error = %e,
                                "failed to insert newly-discovered ready issue"
                            );
                        } else {
                            tracing::info!(
                                issue_number = gi.number,
                                "discovered new ready issue, inserted as Idle"
                            );
                        }
                    }
                    Err(e) => tracing::warn!(
                        issue_number = gi.number,
                        error = %e,
                        "find_by_issue_number failed during discovery"
                    ),
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "list_ready_issues failed, skipping discovery"),
    }

    // Load every issue (terminal states included — see above).
    let issues = issue_repo.find_all().await?;

    let mut observations = Vec::new();
    for issue in issues {
        match observe_issue(github, process_repo, config, &issue).await {
            Ok(snapshot) => {
                observations.push(IssueObservation { issue, snapshot });
            }
            Err(e) => {
                tracing::warn!(
                    issue_number = issue.github_issue_number,
                    error = %e,
                    "collect failed for issue, skipping this cycle"
                );
            }
        }
    }
    Ok(observations)
}

/// Build a WorldSnapshot for a single issue.
async fn observe_issue<G, P>(
    github: &G,
    process_repo: &P,
    config: &Config,
    issue: &Issue,
) -> Result<WorldSnapshot>
where
    G: GitHubClient,
    P: ProcessRunRepository,
{
    let n = issue.github_issue_number;
    let id = issue.id;

    // --- GithubIssueSnapshot ---
    let github_issue = observe_github_issue(github, config, n).await?;

    // --- design_pr ---
    let design_pr = observe_pr_for_type(github, process_repo, id, ProcessRunType::Design).await?;

    // --- impl_pr ---
    let impl_pr = observe_pr_for_type(github, process_repo, id, ProcessRunType::Impl).await?;

    // --- ProcessesSnapshot ---
    let processes = observe_processes(process_repo, id).await?;

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

async fn observe_github_issue<G: GitHubClient>(
    github: &G,
    config: &Config,
    issue_number: u64,
) -> Result<GithubIssueSnapshot> {
    let detail = github.get_issue(issue_number).await?;
    let is_open = github.is_issue_open(issue_number).await?;

    const READY_LABEL: &str = "agent:ready";
    let has_ready_label = detail.labels.iter().any(|l| l == READY_LABEL);

    // Determine ready_label_trusted
    let ready_label_trusted = if !has_ready_label {
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

    let weight = label_to_weight(&detail.labels);

    Ok(GithubIssueSnapshot {
        state: if is_open {
            GithubIssueState::Open
        } else {
            GithubIssueState::Closed
        },
        has_ready_label,
        ready_label_trusted,
        weight: Some(weight),
    })
}

async fn observe_pr_for_type<G, P>(
    github: &G,
    process_repo: &P,
    issue_id: i64,
    type_: ProcessRunType,
) -> Result<Option<PrSnapshot>>
where
    G: GitHubClient,
    P: ProcessRunRepository,
{
    // Find latest process run with a pr_number
    let latest = process_repo.find_latest(issue_id, type_).await?;
    let pr_number = match latest.and_then(|r| r.pr_number) {
        Some(n) => n,
        None => return Ok(None),
    };

    // Observe the PR
    let status = match github.get_pr_status(pr_number).await {
        Ok(s) => s,
        Err(e) => {
            // 404 → treat as None
            if e.to_string().contains("404") {
                return Ok(None);
            }
            return Err(e);
        }
    };

    let pr_state = match status {
        PrStatus::Open => PrState::Open,
        PrStatus::Merged => PrState::Merged,
        PrStatus::Closed => PrState::Closed,
    };

    // For open PRs, gather more detail
    let (has_review_comments, ci_status, has_conflict) = if pr_state == PrState::Open {
        let threads = github.list_unresolved_threads(pr_number).await?;
        let has_review_comments = !threads.is_empty();

        let check_runs = github.get_ci_check_runs(pr_number).await?;
        let ci = derive_ci_status(&check_runs);

        let mergeable = github.get_pr_mergeable(pr_number).await?;
        let has_conflict = mergeable == Some(false);

        (has_review_comments, ci, has_conflict)
    } else {
        (false, CiStatus::Unknown, false)
    };

    Ok(Some(PrSnapshot {
        state: pr_state,
        has_review_comments,
        ci_status,
        has_conflict,
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
    issue_id: i64,
) -> Result<ProcessesSnapshot> {
    async fn get_process_snapshot<P: ProcessRunRepository>(
        process_repo: &P,
        issue_id: i64,
        type_: ProcessRunType,
    ) -> Result<Option<ProcessSnapshot>> {
        // Use the combined method to read both the latest run and the consecutive
        // failure count in a single database lock acquisition, preventing a concurrent
        // ProcessRun insert from producing an inconsistent (run, count) pair.
        let Some((run, consecutive_failures)) = process_repo
            .find_latest_with_consecutive_count(issue_id, type_)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(ProcessSnapshot {
            state: run.state,
            index: run.index,
            consecutive_failures,
        }))
    }

    let init = get_process_snapshot(process_repo, issue_id, ProcessRunType::Init).await?;
    let design = get_process_snapshot(process_repo, issue_id, ProcessRunType::Design).await?;
    let design_fix =
        get_process_snapshot(process_repo, issue_id, ProcessRunType::DesignFix).await?;
    let impl_ = get_process_snapshot(process_repo, issue_id, ProcessRunType::Impl).await?;
    let impl_fix = get_process_snapshot(process_repo, issue_id, ProcessRunType::ImplFix).await?;

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
        GitHubCheckRun, GitHubIssue, GitHubIssueDetail, GitHubPr, GitHubPrDetails, ReviewThread,
    };
    use anyhow::Result;

    #[allow(dead_code)]
    struct MockGithub {
        is_open: bool,
        labels: Vec<String>,
    }

    impl MockGithub {
        #[allow(dead_code)]
        fn new(is_open: bool, labels: Vec<String>) -> Self {
            Self { is_open, labels }
        }
    }

    impl GitHubClient for MockGithub {
        async fn list_ready_issues(&self) -> Result<Vec<GitHubIssue>> {
            Ok(vec![])
        }
        async fn get_issue(&self, n: u64) -> Result<GitHubIssueDetail> {
            Ok(GitHubIssueDetail {
                number: n,
                title: format!("issue-{n}"),
                body: String::new(),
                labels: self.labels.clone(),
            })
        }
        async fn is_issue_open(&self, _n: u64) -> Result<bool> {
            Ok(self.is_open)
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
        async fn get_ci_check_runs(&self, _n: u64) -> Result<Vec<GitHubCheckRun>> {
            Ok(vec![])
        }
        async fn get_job_logs(&self, _id: u64) -> Result<String> {
            Ok(String::new())
        }
        async fn get_pr_mergeable(&self, _n: u64) -> Result<Option<bool>> {
            Ok(Some(true))
        }
        async fn get_pr_details(&self, _n: u64) -> Result<GitHubPrDetails> {
            Ok(GitHubPrDetails {
                merged: false,
                mergeable: Some(true),
            })
        }
        async fn get_pr_status(&self, _n: u64) -> Result<PrStatus> {
            Ok(PrStatus::Open)
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
}
