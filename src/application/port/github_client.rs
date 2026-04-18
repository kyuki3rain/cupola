use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::domain::author_association::AuthorAssociation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrStatus {
    Open,
    Closed,
    Merged,
}

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

/// Lightweight issue info returned by `list_open_issues`.
/// Contains only the fields needed for collect-phase discovery and observation.
#[derive(Debug, Clone)]
pub struct OpenIssueInfo {
    pub number: u64,
    pub labels: Vec<String>,
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
    pub author_association: AuthorAssociation,
}

/// PRレベルレビューの状態（COMMENTED / CHANGES_REQUESTED のみ）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrReviewState {
    Commented,
    ChangesRequested,
}

/// PRレベルレビュー1件を表す値オブジェクト（コード行に紐づかないレビュー）
#[derive(Debug, Clone)]
pub struct PrLevelReview {
    pub id: String,
    pub submitted_at: DateTime<Utc>,
    pub body: String,
    pub state: PrReviewState,
    pub author: String,
    pub author_association: AuthorAssociation,
}

/// Unified PR observation returned by a single GraphQL query.
/// Replaces separate calls to get_pr_status, list_unresolved_threads,
/// get_ci_check_runs, and get_pr_mergeable.
#[derive(Debug, Clone)]
pub struct PrObservation {
    pub state: PrStatus,
    pub mergeable: Option<bool>,
    pub unresolved_threads: Vec<ReviewThread>,
    pub check_runs: Vec<GitHubCheckRun>,
    pub pr_level_reviews: Vec<PrLevelReview>,
}

/// GitHub のリポジトリ permission level。
/// `GET /repos/{owner}/{repo}/collaborators/{username}/permission` の `permission` フィールドに対応。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryPermission {
    Admin,
    Maintain,
    Write,
    Triage,
    Read,
}

impl RepositoryPermission {
    /// `RepositoryPermission` を `AuthorAssociation` に変換する。
    ///
    /// このメソッドは permission のみから保守的に変換するフォールバックであり、
    /// `Admin` であっても `OWNER` 判定は行わない。
    /// `trusted_associations` で `OWNER` を扱う必要がある場合は、
    /// `to_author_association_for_actor` を使って actor login と repository owner を渡すこと。
    ///
    /// - Admin → Collaborator（owner 情報なしでは owner 判定不可）
    /// - Maintain → Collaborator
    /// - Write → Collaborator
    /// - Triage / Read → None
    pub fn to_author_association(&self) -> AuthorAssociation {
        self.to_author_association_for_actor(None, None)
    }

    /// `RepositoryPermission` を、actor と repository owner の情報を加味して
    /// `AuthorAssociation` に変換する。
    ///
    /// `Admin` は permission 単体では owner と区別できないため、
    /// `actor_login == repository_owner` のときにのみ `Owner` へ昇格させる。
    ///
    /// - Admin + actor_login == repository_owner → Owner
    /// - Admin（それ以外）→ Collaborator
    /// - Maintain / Write → Collaborator
    /// - Triage / Read → None
    pub fn to_author_association_for_actor(
        &self,
        actor_login: Option<&str>,
        repository_owner: Option<&str>,
    ) -> AuthorAssociation {
        match self {
            Self::Admin => {
                if matches!(
                    (actor_login, repository_owner),
                    (Some(actor), Some(owner)) if actor == owner
                ) {
                    AuthorAssociation::Owner
                } else {
                    AuthorAssociation::Collaborator
                }
            }
            Self::Maintain | Self::Write => AuthorAssociation::Collaborator,
            Self::Triage | Self::Read => AuthorAssociation::None,
        }
    }
}

pub trait GitHubClient: Send + Sync {
    /// Fetch all open issues (not PRs) with full pagination.
    /// Returns issue number + labels for each open issue.
    fn list_open_issues(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<OpenIssueInfo>>> + Send;

    fn get_issue(
        &self,
        issue_number: u64,
    ) -> impl std::future::Future<Output = Result<GitHubIssueDetail>> + Send;

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

    /// CI job のログを取得する。check-run ID を job ID として使用。
    fn get_job_logs(&self, job_id: u64)
    -> impl std::future::Future<Output = Result<String>> + Send;

    /// PR の merged / mergeable を 1 回の API 呼び出しで取得する。
    fn get_pr_details(
        &self,
        pr_number: u64,
    ) -> impl std::future::Future<Output = Result<GitHubPrDetails>> + Send;

    /// Issue の timeline から、指定ラベルを最後に付与した actor の login を返す。
    ///
    /// ラベル付与イベントが見つからない場合は `Ok(None)` を返す。
    fn fetch_label_actor_login(
        &self,
        issue_number: u64,
        label_name: &str,
    ) -> impl std::future::Future<Output = Result<Option<String>>> + Send;

    /// ユーザーのリポジトリに対する permission level を返す。
    ///
    /// コラボレーターでない場合は `RepositoryPermission::Read` を返す。
    fn fetch_user_permission(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Result<RepositoryPermission>> + Send;

    /// Issue からラベルを削除する。
    fn remove_label(
        &self,
        issue_number: u64,
        label_name: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Unified PR observation via a single GraphQL query.
    /// Returns state, mergeable, unresolved review threads, and CI check runs.
    /// Returns `Ok(None)` if the PR does not exist (GraphQL returns `pullRequest: null`).
    fn observe_pr(
        &self,
        pr_number: u64,
    ) -> impl std::future::Future<Output = Result<Option<PrObservation>>> + Send;
}
