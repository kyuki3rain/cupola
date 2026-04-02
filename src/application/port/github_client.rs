use anyhow::Result;

#[derive(Debug, Clone)]
pub struct GitHubCheckRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    /// Corresponds to GitHub Checks API `output.summary`.
    pub output_summary: Option<String>,
    /// Corresponds to GitHub Checks API `output.text`.
    pub output_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitHubPrDetails {
    pub merged: bool,
    pub mergeable: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct GitHubIssueDetail {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GitHubPr {
    pub number: u64,
    pub merged: bool,
}

#[derive(Debug, Clone)]
pub struct ReviewThread {
    pub id: String,
    pub path: String,
    pub line: Option<u32>,
    pub comments: Vec<ReviewComment>,
}

#[derive(Debug, Clone)]
pub struct ReviewComment {
    pub author: String,
    pub body: String,
}

pub trait GitHubClient: Send + Sync {
    fn list_ready_issues(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<GitHubIssue>>> + Send;

    fn get_issue(
        &self,
        issue_number: u64,
    ) -> impl std::future::Future<Output = Result<GitHubIssueDetail>> + Send;

    fn is_issue_open(
        &self,
        issue_number: u64,
    ) -> impl std::future::Future<Output = Result<bool>> + Send;

    fn find_pr_by_branches(
        &self,
        head: &str,
        base: &str,
    ) -> impl std::future::Future<Output = Result<Option<GitHubPr>>> + Send;

    fn is_pr_merged(
        &self,
        pr_number: u64,
    ) -> impl std::future::Future<Output = Result<bool>> + Send;

    fn list_unresolved_threads(
        &self,
        pr_number: u64,
    ) -> impl std::future::Future<Output = Result<Vec<ReviewThread>>> + Send;

    fn create_pr(
        &self,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> impl std::future::Future<Output = Result<u64>> + Send;

    fn reply_to_thread(
        &self,
        thread_id: &str,
        body: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn resolve_thread(
        &self,
        thread_id: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn comment_on_issue(
        &self,
        issue_number: u64,
        body: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn close_issue(
        &self,
        issue_number: u64,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// PR の CI check-runs を取得する。status が "completed" のもののみ返す。
    fn get_ci_check_runs(
        &self,
        pr_number: u64,
    ) -> impl std::future::Future<Output = Result<Vec<GitHubCheckRun>>> + Send;

    /// CI job のログを取得する。check-run ID を job ID として使用。
    fn get_job_logs(&self, job_id: u64)
    -> impl std::future::Future<Output = Result<String>> + Send;

    /// PR の mergeable フィールドを取得する。None は GitHub が計算中を意味する。
    fn get_pr_mergeable(
        &self,
        pr_number: u64,
    ) -> impl std::future::Future<Output = Result<Option<bool>>> + Send;

    /// PR の merged / mergeable を 1 回の API 呼び出しで取得する。
    fn get_pr_details(
        &self,
        pr_number: u64,
    ) -> impl std::future::Future<Output = Result<GitHubPrDetails>> + Send;
}
