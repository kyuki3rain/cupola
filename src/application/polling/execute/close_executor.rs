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

pub(super) async fn close<G, I, P, C, W, F>(
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
    let n = issue.github_issue_number;
    ctx.github.close_issue(n).await?;

    let updates = MetadataUpdates {
        close_finished: Some(true),
        ..Default::default()
    };
    let issue_id = issue.id;
    retry_db_update(n, "CloseIssue", || {
        ctx.issue_repo.update_state_and_metadata(issue_id, &updates)
    })
    .await?;
    issue.close_finished = true;
    Ok(())
}
