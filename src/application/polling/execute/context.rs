use crate::application::init_task_manager::InitTaskManager;
use crate::application::port::claude_code_runner::ClaudeCodeRunner;
use crate::application::port::file_generator::FileGenerator;
use crate::application::port::github_client::GitHubClient;
use crate::application::port::issue_repository::IssueRepository;
use crate::application::port::process_run_repository::ProcessRunRepository;
use crate::application::session_manager::SessionManager;
use crate::domain::config::Config;

use super::SpawnableGitWorktree;

pub struct ExecuteContext<'a, G, I, P, C, W, F>
where
    G: GitHubClient,
    I: IssueRepository,
    P: ProcessRunRepository,
    C: ClaudeCodeRunner,
    W: SpawnableGitWorktree,
    F: FileGenerator + Clone + 'static,
{
    pub github: &'a G,
    pub issue_repo: &'a I,
    pub process_repo: &'a P,
    pub claude_runner: &'a C,
    pub worktree: &'a W,
    pub file_gen: &'a F,
    pub session_mgr: &'a mut SessionManager,
    pub init_mgr: &'a mut InitTaskManager,
    pub config: &'a Config,
}
