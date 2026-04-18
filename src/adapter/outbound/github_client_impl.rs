use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;
use tracing::{error, warn};

use crate::application::port::github_client::{
    GitHubClient, GitHubIssueDetail, GitHubPr, GitHubPrDetails, OpenIssueInfo, PrObservation,
    RepositoryPermission, ReviewThread,
};

use super::github_api_error::GitHubApiError;
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

/// リトライポリシーを適用してクロージャを実行する汎用ヘルパー。
///
/// - `GitHubApiError::RateLimit`: `retry_after` または 60 秒デフォルトで待機 → 最大 3 回リトライ
/// - `GitHubApiError::ServerError`: 指数バックオフ (1s → 2s → 4s) → 最大 3 回リトライ
/// - `GitHubApiError::Unauthorized` / `Forbidden` / `NotFound` / `Other`: 即時伝播
/// - その他の `anyhow::Error` (非 `GitHubApiError`): 即時伝播
///
/// リトライ試行ごとに `tracing::warn!` でエラー種別・試行回数・待機時間をログ出力する。
/// リトライ枯渇時は `tracing::error!` でログ出力し最後のエラーを返す。
async fn with_retry<T, Fut>(op_name: &str, f: impl Fn() -> Fut) -> Result<T>
where
    Fut: Future<Output = Result<T>>,
{
    const MAX_RETRIES: u32 = 3;
    let mut attempt = 0u32;

    loop {
        match f().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                // GitHubApiError にダウンキャストしてリトライ待機時間を決定する。
                // downcast_ref のボローをスコープで終わらせてから err を移動する。
                let wait_opt: Option<Duration> =
                    if let Some(gh_err) = err.downcast_ref::<GitHubApiError>() {
                        match gh_err {
                            GitHubApiError::RateLimit { retry_after, .. } => {
                                Some(retry_after.unwrap_or(Duration::from_secs(60)))
                            }
                            GitHubApiError::ServerError { .. } => {
                                // 指数バックオフ: 1s, 2s, 4s (attempt = 0, 1, 2)
                                Some(Duration::from_secs(1u64 << attempt))
                            }
                            // Unauthorized / Forbidden / NotFound / Other は即時伝播
                            _ => None,
                        }
                    } else {
                        // 非 GitHubApiError は即時伝播
                        None
                    };

                match wait_opt {
                    Some(wait_dur) if attempt < MAX_RETRIES => {
                        warn!(
                            op_name,
                            attempt = attempt + 1,
                            wait_secs = wait_dur.as_secs(),
                            %err,
                            "retrying after error"
                        );
                        sleep(wait_dur).await;
                        attempt += 1;
                    }
                    Some(_) => {
                        error!(op_name, %err, "retry exhausted");
                        return Err(err.context(format!("{op_name}: retry exhausted")));
                    }
                    None => return Err(err.context(op_name.to_string())),
                }
            }
        }
    }
}

impl GitHubClient for GitHubClientImpl {
    async fn list_open_issues(&self) -> Result<Vec<OpenIssueInfo>> {
        self.rest.list_open_issues().await
    }

    async fn get_issue(&self, issue_number: u64) -> Result<GitHubIssueDetail> {
        self.rest.get_issue(issue_number).await
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

    async fn get_job_logs(&self, job_id: u64) -> Result<String> {
        with_retry("get_job_logs", || self.rest.get_job_logs(job_id)).await
    }

    async fn get_pr_details(&self, pr_number: u64) -> Result<GitHubPrDetails> {
        self.rest.get_pr_details(pr_number).await
    }

    async fn fetch_label_actor_login(
        &self,
        issue_number: u64,
        label_name: &str,
    ) -> Result<Option<String>> {
        with_retry("fetch_label_actor_login", || {
            self.rest.fetch_label_actor_login(issue_number, label_name)
        })
        .await
    }

    async fn fetch_user_permission(&self, username: &str) -> Result<RepositoryPermission> {
        with_retry("fetch_user_permission", || {
            self.rest.fetch_user_permission(username)
        })
        .await
    }

    async fn remove_label(&self, issue_number: u64, label_name: &str) -> Result<()> {
        with_retry("remove_label", || {
            self.rest.remove_label(issue_number, label_name)
        })
        .await
    }

    async fn observe_pr(&self, pr_number: u64) -> Result<Option<PrObservation>> {
        self.graphql.observe_pr(pr_number).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use reqwest::StatusCode;

    use super::*;

    // ============================================================
    // with_retry ユニットテスト (Task 4.2)
    // ============================================================

    #[tokio::test]
    async fn with_retry_returns_ok_immediately_on_success() {
        let result = with_retry("test", || async { Ok::<i32, anyhow::Error>(42) }).await;
        assert_eq!(result.expect("should succeed"), 42);
    }

    #[tokio::test(start_paused = true)]
    async fn with_retry_retries_on_rate_limit_and_succeeds() {
        let count = Arc::new(AtomicU32::new(0));
        let count_c = count.clone();

        let result = with_retry("test", move || {
            let cnt = count_c.clone();
            async move {
                let n = cnt.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(anyhow::Error::from(GitHubApiError::RateLimit {
                        retry_after: Some(Duration::from_secs(1)),
                        resource: "test".to_string(),
                    }))
                } else {
                    Ok(42i32)
                }
            }
        })
        .await;

        assert_eq!(result.expect("should succeed after retries"), 42);
        // 1 initial + 2 retries = 3 calls
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn with_retry_uses_default_wait_when_rate_limit_has_no_retry_after() {
        let count = Arc::new(AtomicU32::new(0));
        let count_c = count.clone();

        let result = with_retry("test", move || {
            let cnt = count_c.clone();
            async move {
                let n = cnt.fetch_add(1, Ordering::SeqCst);
                if n < 1 {
                    Err(anyhow::Error::from(GitHubApiError::RateLimit {
                        retry_after: None, // デフォルト 60s
                        resource: "test".to_string(),
                    }))
                } else {
                    Ok(0i32)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn with_retry_exhausts_retries_on_server_error() {
        let count = Arc::new(AtomicU32::new(0));
        let count_c = count.clone();

        let result = with_retry("test", move || {
            let cnt = count_c.clone();
            async move {
                cnt.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(anyhow::Error::from(GitHubApiError::ServerError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    resource: "test".to_string(),
                }))
            }
        })
        .await;

        assert!(result.is_err());
        // 1 initial + 3 retries = 4 calls
        assert_eq!(count.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn with_retry_returns_immediately_on_unauthorized() {
        let count = Arc::new(AtomicU32::new(0));
        let count_c = count.clone();

        let result = with_retry("test", move || {
            let cnt = count_c.clone();
            async move {
                cnt.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(anyhow::Error::from(GitHubApiError::Unauthorized {
                    resource: "test".to_string(),
                }))
            }
        })
        .await;

        assert!(result.is_err());
        // リトライなし → 1 回のみ
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn with_retry_returns_immediately_on_forbidden() {
        let count = Arc::new(AtomicU32::new(0));
        let count_c = count.clone();

        let result = with_retry("test", move || {
            let cnt = count_c.clone();
            async move {
                cnt.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(anyhow::Error::from(GitHubApiError::Forbidden {
                    body: "denied".to_string(),
                    resource: "test".to_string(),
                }))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn with_retry_returns_immediately_on_non_github_error() {
        let count = Arc::new(AtomicU32::new(0));
        let count_c = count.clone();

        let result = with_retry("test", move || {
            let cnt = count_c.clone();
            async move {
                cnt.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(anyhow::anyhow!("plain anyhow error (no GitHubApiError)"))
            }
        })
        .await;

        assert!(result.is_err());
        // 非 GitHubApiError はリトライなし
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
