use anyhow::Result;

use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::file_generator::FileGenerator;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::domain::effect::Effect;
use crate::domain::issue::Issue;

use super::SpawnableGitWorktree;
use super::context::ExecuteContext;
use super::{
    close_executor, comment_executor, spawn_init_executor, spawn_process_executor,
    worktree_executor,
};

pub(super) async fn dispatch<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &mut Issue,
    effect: &Effect,
) -> Result<()>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    match effect {
        Effect::PostCompletedComment => comment_executor::post_completed(ctx, issue).await,
        Effect::PostCancelComment => comment_executor::post_cancel(ctx, issue).await,
        Effect::PostRetryExhaustedComment {
            consecutive_failures,
            process_type,
        } => {
            comment_executor::post_retry_exhausted(ctx, issue, *consecutive_failures, *process_type)
                .await
        }
        Effect::RejectUntrustedReadyIssue => comment_executor::reject_untrusted(ctx, issue).await,
        Effect::PostCiFixLimitComment => comment_executor::post_ci_fix_limit(ctx, issue).await,
        Effect::SpawnInit => spawn_init_executor::execute(ctx, issue).await,
        Effect::SpawnProcess {
            type_,
            causes,
            pending_run_id,
        } => spawn_process_executor::execute(ctx, issue, *type_, causes, *pending_run_id).await,
        Effect::SwitchToImplBranch => worktree_executor::switch_to_impl_branch(ctx, issue).await,
        Effect::CleanupWorktree => worktree_executor::cleanup(ctx, issue).await,
        Effect::CloseIssue => close_executor::close(ctx, issue).await,
    }
}
