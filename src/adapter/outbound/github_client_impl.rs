use anyhow::Result;

use crate::application::port::github_client::{
    GitHubCheckRun, GitHubClient, GitHubIssue, GitHubIssueDetail, GitHubPr, GitHubPrDetails,
    PrStatus, ReviewThread,
};

use super::github_graphql_client::GraphQLClient;
use super::github_rest_client::OctocrabRestClient;

pub struct GitHubClientImpl {
    rest: OctocrabRestClient,
    graphql: GraphQLClient,
}

impl GitHubClientImpl {
    pub fn new(rest: OctocrabRestClient, graphql: GraphQLClient) -> Self {
        Self { rest, graphql }
    }
}

impl GitHubClient for GitHubClientImpl {
    async fn list_ready_issues(&self) -> Result<Vec<GitHubIssue>> {
        self.rest.list_ready_issues().await
    }

    async fn get_issue(&self, issue_number: u64) -> Result<GitHubIssueDetail> {
        self.rest.get_issue(issue_number).await
    }

    async fn is_issue_open(&self, issue_number: u64) -> Result<bool> {
        self.rest.is_issue_open(issue_number).await
    }

    async fn find_pr_by_branches(&self, head: &str, base: &str) -> Result<Option<GitHubPr>> {
        self.rest.find_pr_by_branches(head, base).await
    }

    async fn is_pr_merged(&self, pr_number: u64) -> Result<bool> {
        self.rest.is_pr_merged(pr_number).await
    }

    async fn list_unresolved_threads(&self, pr_number: u64) -> Result<Vec<ReviewThread>> {
        self.graphql.list_unresolved_threads(pr_number).await
    }

    async fn create_pr(&self, head: &str, base: &str, title: &str, body: &str) -> Result<u64> {
        self.rest.create_pr(head, base, title, body).await
    }

    async fn reply_to_thread(&self, thread_id: &str, body: &str) -> Result<()> {
        self.graphql.reply_to_thread(thread_id, body).await
    }

    async fn resolve_thread(&self, thread_id: &str) -> Result<()> {
        self.graphql.resolve_thread(thread_id).await
    }

    async fn comment_on_issue(&self, issue_number: u64, body: &str) -> Result<()> {
        self.rest.comment_on_issue(issue_number, body).await
    }

    async fn close_issue(&self, issue_number: u64) -> Result<()> {
        self.rest.close_issue(issue_number).await
    }

    async fn get_ci_check_runs(&self, pr_number: u64) -> Result<Vec<GitHubCheckRun>> {
        self.rest.get_ci_check_runs(pr_number).await
    }

    async fn get_job_logs(&self, job_id: u64) -> Result<String> {
        self.rest.get_job_logs(job_id).await
    }

    async fn get_pr_mergeable(&self, pr_number: u64) -> Result<Option<bool>> {
        self.rest.get_pr_mergeable(pr_number).await
    }

    async fn get_pr_details(&self, pr_number: u64) -> Result<GitHubPrDetails> {
        self.rest.get_pr_details(pr_number).await
    }

    async fn get_pr_status(&self, pr_number: u64) -> Result<PrStatus> {
        self.rest.get_pr_status(pr_number).await
    }
}
