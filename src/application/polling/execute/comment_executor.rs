use anyhow::Result;

use crate::domain::issue::Issue;
use crate::domain::metadata_update::MetadataUpdates;
use crate::domain::process_run::ProcessRunType;

use super::context::ExecuteContext;
use super::shared;

pub(super) async fn post_completed<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &Issue,
) -> Result<()>
where
    G: crate::application::port::github_client::GitHubClient,
    I: crate::application::port::issue_repository::IssueRepository,
    P: crate::application::port::process_run_repository::ProcessRunRepository,
    C: crate::application::port::claude_code_runner::ClaudeCodeRunner,
    W: super::SpawnableGitWorktree,
    F: crate::application::port::file_generator::FileGenerator + Clone + 'static,
{
    let n = issue.github_issue_number;
    let lang = &ctx.config.language;
    let msg = rust_i18n::t!("issue_comment.all_completed", locale = lang);
    ctx.github.comment_on_issue(n, &msg).await?;
    Ok(())
}

pub(super) async fn post_cancel<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &Issue,
) -> Result<()>
where
    G: crate::application::port::github_client::GitHubClient,
    I: crate::application::port::issue_repository::IssueRepository,
    P: crate::application::port::process_run_repository::ProcessRunRepository,
    C: crate::application::port::claude_code_runner::ClaudeCodeRunner,
    W: super::SpawnableGitWorktree,
    F: crate::application::port::file_generator::FileGenerator + Clone + 'static,
{
    let n = issue.github_issue_number;
    let lang = &ctx.config.language;
    let msg = rust_i18n::t!("issue_comment.cleanup_done", locale = lang);
    ctx.github.comment_on_issue(n, &msg).await?;
    Ok(())
}

pub(super) async fn post_retry_exhausted<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &Issue,
    consecutive_failures: u32,
    process_type: ProcessRunType,
) -> Result<()>
where
    G: crate::application::port::github_client::GitHubClient,
    I: crate::application::port::issue_repository::IssueRepository,
    P: crate::application::port::process_run_repository::ProcessRunRepository,
    C: crate::application::port::claude_code_runner::ClaudeCodeRunner,
    W: super::SpawnableGitWorktree,
    F: crate::application::port::file_generator::FileGenerator + Clone + 'static,
{
    let n = issue.github_issue_number;
    let lang = &ctx.config.language;
    let unknown = rust_i18n::t!("issue_comment.unknown_error", locale = lang);
    let last_error = shared::find_last_error(ctx.process_repo, issue.id, process_type).await;
    let error_str = last_error.as_deref().unwrap_or(&unknown);
    let count = consecutive_failures;
    let msg = rust_i18n::t!(
        "issue_comment.retry_exhausted",
        locale = lang,
        count = count,
        error = error_str
    );
    ctx.github.comment_on_issue(n, &msg).await?;
    Ok(())
}

pub(super) async fn post_ci_fix_limit<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &mut Issue,
) -> Result<()>
where
    G: crate::application::port::github_client::GitHubClient,
    I: crate::application::port::issue_repository::IssueRepository,
    P: crate::application::port::process_run_repository::ProcessRunRepository,
    C: crate::application::port::claude_code_runner::ClaudeCodeRunner,
    W: super::SpawnableGitWorktree,
    F: crate::application::port::file_generator::FileGenerator + Clone + 'static,
{
    let n = issue.github_issue_number;
    let msg = format!(
        "CI fix limit reached ({} cycles). Automatic fixing has stopped.",
        ctx.config.max_ci_fix_cycles
    );
    match ctx.github.comment_on_issue(n, &msg).await {
        Ok(()) => {
            ctx.issue_repo
                .update_state_and_metadata(
                    issue.id,
                    &MetadataUpdates {
                        ci_fix_limit_notified: Some(true),
                        ..Default::default()
                    },
                )
                .await?;
        }
        Err(e) => {
            tracing::warn!(
                issue_number = n,
                error = %e,
                "failed to post ci-fix-limit comment; will retry next cycle"
            );
        }
    }
    Ok(())
}

pub(super) async fn reject_untrusted<G, I, P, C, W, F>(
    ctx: &mut ExecuteContext<'_, G, I, P, C, W, F>,
    issue: &Issue,
) -> Result<()>
where
    G: crate::application::port::github_client::GitHubClient,
    I: crate::application::port::issue_repository::IssueRepository,
    P: crate::application::port::process_run_repository::ProcessRunRepository,
    C: crate::application::port::claude_code_runner::ClaudeCodeRunner,
    W: super::SpawnableGitWorktree,
    F: crate::application::port::file_generator::FileGenerator + Clone + 'static,
{
    let n = issue.github_issue_number;
    match ctx.github.remove_label(n, "agent:ready").await {
        Ok(()) => {
            let msg = "This issue was labeled `agent:ready` by an untrusted actor. \
                 Only trusted collaborators may trigger automatic processing."
                .to_string();
            let _ = ctx.github.comment_on_issue(n, &msg).await;
        }
        Err(e) => {
            tracing::warn!(issue_number = n, error = %e, "failed to remove agent:ready label");
        }
    }
    Ok(())
}
