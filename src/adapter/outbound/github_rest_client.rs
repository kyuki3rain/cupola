use anyhow::{Context, Result, anyhow};
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

/// Retrieve a GitHub token from `gh auth token` or the `GITHUB_TOKEN` env var.
pub fn resolve_github_token() -> Result<String> {
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        return Ok(token);
    }

    let output = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .map_err(|e| anyhow!("failed to run `gh auth token`: {e}"))?;

    if !output.status.success() {
        return Err(anyhow!(
            "`gh auth token` failed. Set GITHUB_TOKEN or run `gh auth login`."
        ));
    }

    let token = String::from_utf8(output.stdout)
        .context("`gh auth token` output is not valid UTF-8")?
        .trim()
        .to_string();

    if token.is_empty() {
        return Err(anyhow!(
            "empty token from `gh auth token`. Set GITHUB_TOKEN or run `gh auth login`."
        ));
    }

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_token_from_env() {
        // Save and restore env
        let original = std::env::var("GITHUB_TOKEN").ok();
        // SAFETY: test runs sequentially with --test-threads=1 for env var safety
        unsafe {
            std::env::set_var("GITHUB_TOKEN", "test-token-123");
        }

        let result = resolve_github_token();
        assert_eq!(result.unwrap(), "test-token-123");

        // Restore
        unsafe {
            match original {
                Some(v) => std::env::set_var("GITHUB_TOKEN", v),
                None => std::env::remove_var("GITHUB_TOKEN"),
            }
        }
    }
}
