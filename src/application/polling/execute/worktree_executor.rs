use std::path::Path;

use anyhow::Result;

use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::file_generator::FileGenerator;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;

use super::context::ExecuteContext;
use super::{SpawnableGitWorktree, retry_db_update};

pub(super) async fn cleanup<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &mut Issue,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    if let Some(ref wt) = issue.worktree_path {
        let path = Path::new(wt);
        if path.exists() {
            ctx.worktree.remove(path)?;
        }

        let feature_name = &issue.feature_name;
        let n = issue.github_issue_number;
        for branch in [
            format!("cupola/{feature_name}/main"),
            format!("cupola/{feature_name}/design"),
        ] {
            match ctx.worktree.delete_branch(&branch) {
                Ok(()) => {
                    tracing::info!(
                        issue_number = n,
                        branch = %branch,
                        "branch deleted"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        issue_number = n,
                        branch = %branch,
                        error = %e,
                        "failed to delete branch, skipping"
                    );
                }
            }
        }

        let updates = MetadataUpdates {
            worktree_path: Some(None),
            ..Default::default()
        };
        let issue_id = issue.id;
        retry_db_update(n, "CleanupWorktree", || {
            ctx.issue_repo.update_state_and_metadata(issue_id, &updates)
        })
        .await?;
        issue.worktree_path = None;
    }
    Ok(())
}

pub(super) async fn switch_to_impl_branch<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &Issue,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    let wt_path = issue
        .worktree_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("worktree_path is None for SwitchToImplBranch"))?;
    let path = Path::new(wt_path);
    let branch = format!("cupola/{}/main", issue.feature_name);
    ctx.worktree.checkout(path, &branch)?;
    ctx.worktree.pull(path)?;
    Ok(())
}
