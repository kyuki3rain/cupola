use anyhow::Result;

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

    fn get_issue_labels(
        &self,
        issue_number: u64,
    ) -> impl std::future::Future<Output = Result<Vec<String>>> + Send;
}
