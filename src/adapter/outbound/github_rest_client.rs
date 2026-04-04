use anyhow::{Context, Result, anyhow};
use octocrab::Octocrab;

use crate::application::port::github_client::{
    GitHubCheckRun, GitHubIssue, GitHubIssueDetail, GitHubPr, GitHubPrDetails, PrStatus,
    RepositoryPermission,
};

pub struct OctocrabRestClient {
    octocrab: Octocrab,
    owner: String,
    repo: String,
    token: String,
    http_client: reqwest::Client,
}

impl OctocrabRestClient {
    pub fn new(token: String, owner: String, repo: String) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token.clone())
            .build()
            .context("failed to build octocrab client")?;
        let http_client = reqwest::Client::builder()
            .user_agent("cupola/1.0")
            .build()
            .context("failed to build reqwest client")?;
        Ok(Self {
            octocrab,
            owner,
            repo,
            token,
            http_client,
        })
    }

    pub async fn get_ci_check_runs(&self, pr_number: u64) -> Result<Vec<GitHubCheckRun>> {
        // Step 1: Get PR head SHA
        let pr = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .get(pr_number)
            .await
            .with_context(|| format!("failed to get PR #{pr_number} for CI check-runs"))?;

        let sha = pr.head.sha.clone();

        // Step 2: GET /repos/{owner}/{repo}/commits/{sha}/check-runs?per_page=100
        let url = format!(
            "https://api.github.com/repos/{}/{}/commits/{}/check-runs?per_page=100",
            self.owner, self.repo, sha
        );

        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .with_context(|| format!("failed to call check-runs API for SHA {sha}"))?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "check-runs API returned {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .context("failed to parse check-runs response")?;

        let runs = body["check_runs"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let status = r["status"].as_str()?.to_string();
                        if status != "completed" {
                            return None;
                        }
                        Some(GitHubCheckRun {
                            id: r["id"].as_u64().unwrap_or(0),
                            name: r["name"].as_str().unwrap_or("").to_string(),
                            status,
                            conclusion: r["conclusion"].as_str().map(str::to_string),
                            output_summary: r["output"]["summary"]
                                .as_str()
                                .filter(|s| !s.is_empty())
                                .map(str::to_string),
                            output_text: r["output"]["text"]
                                .as_str()
                                .filter(|s| !s.is_empty())
                                .map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(runs)
    }

    pub async fn get_job_logs(&self, job_id: u64) -> Result<String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/jobs/{}/logs",
            self.owner, self.repo, job_id
        );

        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .with_context(|| format!("failed to get logs for job {job_id}"))?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "job logs API returned {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        resp.text().await.context("failed to read job logs body")
    }

    pub async fn get_pr_mergeable(&self, pr_number: u64) -> Result<Option<bool>> {
        let pr = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .get(pr_number)
            .await
            .with_context(|| format!("failed to get PR #{pr_number} for mergeable check"))?;

        Ok(pr.mergeable)
    }

    pub async fn get_pr_details(&self, pr_number: u64) -> Result<GitHubPrDetails> {
        let pr = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .get(pr_number)
            .await
            .with_context(|| format!("failed to get PR #{pr_number} details"))?;

        Ok(GitHubPrDetails {
            merged: pr.merged_at.is_some(),
            mergeable: pr.mergeable,
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

    pub async fn get_pr_status(&self, pr_number: u64) -> Result<PrStatus> {
        let pr = self
            .octocrab
            .pulls(&self.owner, &self.repo)
            .get(pr_number)
            .await
            .with_context(|| format!("failed to get PR #{pr_number} status"))?;

        if pr.merged_at.is_some() {
            Ok(PrStatus::Merged)
        } else if pr.state == Some(octocrab::models::IssueState::Open) {
            Ok(PrStatus::Open)
        } else {
            Ok(PrStatus::Closed)
        }
    }

    /// Issue の timeline から、指定ラベルを最後に付与した actor の login を返す。
    ///
    /// `GET /repos/{owner}/{repo}/issues/{issue_number}/timeline` を呼び出し、
    /// `event == "labeled"` かつ `label.name == label_name` のイベントを逆順で検索する。
    pub async fn fetch_label_actor_login(
        &self,
        issue_number: u64,
        label_name: &str,
    ) -> Result<Option<String>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/timeline?per_page=100",
            self.owner, self.repo, issue_number
        );

        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .with_context(|| {
                format!("failed to fetch timeline for issue #{issue_number}")
            })?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "timeline API returned {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        let events: serde_json::Value = resp
            .json()
            .await
            .context("failed to parse timeline response")?;

        // 逆順で最新の labeled イベントを検索
        let login = events
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|e| {
                e["event"].as_str() == Some("labeled")
                    && e["label"]["name"].as_str() == Some(label_name)
            })
            .and_then(|e| e["actor"]["login"].as_str().map(String::from));

        Ok(login)
    }

    /// ユーザーのリポジトリに対する permission level を返す。
    ///
    /// `GET /repos/{owner}/{repo}/collaborators/{username}/permission` を呼び出す。
    /// 404 の場合は `RepositoryPermission::Read` を返す。
    pub async fn fetch_user_permission(&self, username: &str) -> Result<RepositoryPermission> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/collaborators/{}/permission",
            self.owner, self.repo, username
        );

        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .with_context(|| format!("failed to fetch permission for user {username}"))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(RepositoryPermission::Read);
        }

        if !resp.status().is_success() {
            return Err(anyhow!(
                "permission API returned {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .context("failed to parse permission response")?;

        let permission = match body["permission"].as_str().unwrap_or("none") {
            "admin" => RepositoryPermission::Admin,
            "maintain" => RepositoryPermission::Maintain,
            "write" => RepositoryPermission::Write,
            "triage" => RepositoryPermission::Triage,
            _ => RepositoryPermission::Read,
        };

        Ok(permission)
    }

    /// Issue からラベルを削除する。
    pub async fn remove_label(&self, issue_number: u64, label_name: &str) -> Result<()> {
        // ラベル名の URL エンコード（コロンなどが含まれる "agent:ready" 等に対応）
        let encoded_label = label_name.replace(' ', "%20").replace('#', "%23");
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/labels/{}",
            self.owner, self.repo, issue_number, encoded_label
        );

        let resp = self
            .http_client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .with_context(|| {
                format!("failed to remove label '{label_name}' from issue #{issue_number}")
            })?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "remove label API returned {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        Ok(())
    }
}
