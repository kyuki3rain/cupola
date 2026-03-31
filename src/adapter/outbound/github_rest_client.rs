use anyhow::{Context, Result};
use octocrab::Octocrab;

use crate::application::port::github_client::{GitHubIssue, GitHubIssueDetail, GitHubPr};

pub struct OctocrabRestClient {
    octocrab: Octocrab,
    owner: String,
    repo: String,
}

impl OctocrabRestClient {
    pub fn new(token: String, owner: String, repo: String) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token)
            .build()
            .context("failed to build octocrab client")?;
        Ok(Self {
            octocrab,
            owner,
            repo,
        })
    }

    pub async fn list_ready_issues(&self) -> Result<Vec<GitHubIssue>> {
        let page = self
            .octocrab
            .issues(&self.owner, &self.repo)
            .list()
            .labels(&[String::from("agent:ready")])
            .state(octocrab::params::State::Open)
            .per_page(100)
            .send()
            .await
            .context("failed to list issues with agent:ready label")?;

        Ok(page
            .items
            .into_iter()
            .map(|i| GitHubIssue {
                number: i.number,
                title: i.title,
            })
            .collect())
    }

    pub async fn get_issue(&self, issue_number: u64) -> Result<GitHubIssueDetail> {
        let issue = self
            .octocrab
            .issues(&self.owner, &self.repo)
            .get(issue_number)
            .await
            .with_context(|| format!("failed to get issue #{issue_number}"))?;

        Ok(GitHubIssueDetail {
            number: issue.number,
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            labels: issue.labels.into_iter().map(|l| l.name).collect(),
        })
    }

    pub async fn is_issue_open(&self, issue_number: u64) -> Result<bool> {
        let issue = self
            .octocrab
            .issues(&self.owner, &self.repo)
            .get(issue_number)
            .await
            .with_context(|| format!("failed to get issue #{issue_number} for state check"))?;

        Ok(issue.state == octocrab::models::IssueState::Open)
    }

    pub async fn find_pr_by_branches(&self, head: &str, base: &str) -> Result<Option<GitHubPr>> {
        let full_head = format!("{}:{head}", self.owner);
        let page = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .list()
            .head(full_head)
            .base(base)
            .state(octocrab::params::State::Open)
            .per_page(1)
            .send()
            .await
            .context("failed to search PRs by branches")?;

        Ok(page.items.into_iter().next().map(|pr| GitHubPr {
            number: pr.number,
            merged: pr.merged_at.is_some(),
        }))
    }

    pub async fn is_pr_merged(&self, pr_number: u64) -> Result<bool> {
        let pr = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .get(pr_number)
            .await
            .with_context(|| format!("failed to get PR #{pr_number}"))?;

        Ok(pr.merged_at.is_some())
    }

    pub async fn create_pr(&self, head: &str, base: &str, title: &str, body: &str) -> Result<u64> {
        let pr = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .create(title, head, base)
            .body(body)
            .send()
            .await
            .context("failed to create PR")?;

        Ok(pr.number)
    }

    pub async fn comment_on_issue(&self, issue_number: u64, body: &str) -> Result<()> {
        self.octocrab
            .issues(&self.owner, &self.repo)
            .create_comment(issue_number, body)
            .await
            .with_context(|| format!("failed to comment on issue #{issue_number}"))?;

        Ok(())
    }

    pub async fn get_issue_labels(&self, issue_number: u64) -> Result<Vec<String>> {
        let page = self
            .octocrab
            .issues(&self.owner, &self.repo)
            .list_labels_for_issue(issue_number)
            .per_page(100)
            .send()
            .await
            .with_context(|| format!("failed to get labels for issue #{issue_number}"))?;

        Ok(page.items.into_iter().map(|l| l.name).collect())
    }

    pub async fn close_issue(&self, issue_number: u64) -> Result<()> {
        self.octocrab
            .issues(&self.owner, &self.repo)
            .update(issue_number)
            .state(octocrab::models::IssueState::Closed)
            .send()
            .await
            .with_context(|| format!("failed to close issue #{issue_number}"))?;

        Ok(())
    }
}
