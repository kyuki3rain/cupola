use anyhow::{Result, anyhow};

use crate::application::port::github_client::GitHubClient;
use crate::domain::author_association::{AuthorAssociation, TrustedAssociations};

/// association チェックの結果。
#[derive(Debug, Clone)]
pub enum AssociationCheckResult {
    /// ラベル付与者は信頼済み association を持つ。
    Trusted,
    /// ラベル付与者は信頼されていない。ラベル削除とコメント通知が実行済み。
    Rejected {
        actor_login: String,
        association: AuthorAssociation,
    },
}

/// `agent:ready` ラベルを付与した actor の association チェックを行う。
///
/// `TrustedAssociations::All` の場合は API 呼び出しなしで即座に `Trusted` を返す。
/// それ以外の場合は Timeline API で actor login を取得し、Permission API で association を判定する。
/// 信頼されていない場合はラベル削除とコメント通知を実行する。
///
/// # Errors
/// - Timeline API または Permission API の呼び出しが失敗した場合
/// - ラベル付与者の login が取得できない場合（ラベル付与イベントが存在しない）
pub async fn check_label_actor<G: GitHubClient>(
    github: &G,
    issue_number: u64,
    label_name: &str,
    trusted: &TrustedAssociations,
    repository_owner: &str,
) -> Result<AssociationCheckResult> {
    // All の場合は即座に Trusted
    if matches!(trusted, TrustedAssociations::All) {
        tracing::info!(
            issue_number,
            label_name,
            "association check skipped (trusted_associations = all)"
        );
        return Ok(AssociationCheckResult::Trusted);
    }

    // Timeline API でラベル付与者の login を取得
    let actor_login = github
        .fetch_label_actor_login(issue_number, label_name)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "no labeled event found for label '{}' on issue #{}",
                label_name,
                issue_number
            )
        })?;

    // Permission API で association を取得
    let permission = github.fetch_user_permission(&actor_login).await?;
    let association =
        permission.to_author_association_for_actor(Some(&actor_login), Some(repository_owner));

    if trusted.is_trusted(&association) {
        tracing::info!(
            issue_number,
            actor_login,
            association = association.as_str(),
            "association check passed"
        );
        Ok(AssociationCheckResult::Trusted)
    } else {
        tracing::info!(
            issue_number,
            actor_login,
            association = association.as_str(),
            "association check rejected: removing label and notifying"
        );

        // ラベルを削除する
        if let Err(e) = github.remove_label(issue_number, label_name).await {
            tracing::error!(
                issue_number,
                label_name,
                error = %e,
                "failed to remove label"
            );
        }

        // コメントで通知する
        let trusted_list = match trusted {
            TrustedAssociations::All => unreachable!("All branch handled above"),
            TrustedAssociations::Specific(list) => list
                .iter()
                .map(AuthorAssociation::as_str)
                .collect::<Vec<_>>()
                .join(", "),
        };
        let comment = format!(
            "The `{label_name}` label was removed because the user `{actor_login}` \
             does not have a trusted author association.\n\n\
             Current association: `{assoc}`\n\
             Trusted associations: {trusted_list}\n\n\
             Only users with the following associations can trigger the agent: {trusted_list}.",
            assoc = association.as_str(),
        );
        if let Err(e) = github.comment_on_issue(issue_number, &comment).await {
            tracing::error!(
                issue_number,
                error = %e,
                "failed to post rejection comment"
            );
        }

        Ok(AssociationCheckResult::Rejected {
            actor_login,
            association,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::github_client::{
        GitHubIssueDetail, GitHubPr, GitHubPrDetails, RepositoryPermission, ReviewThread,
    };
    use std::sync::{Arc, Mutex};

    // --- MockGitHubClient ---

    struct MockGitHubClient {
        label_actor: Option<String>,
        permission: RepositoryPermission,
        removed_labels: Arc<Mutex<Vec<String>>>,
        comments_posted: Arc<Mutex<Vec<String>>>,
        fail_label_actor: bool,
        fail_permission: bool,
    }

    impl MockGitHubClient {
        fn new(actor: Option<&str>, permission: RepositoryPermission) -> Self {
            Self {
                label_actor: actor.map(String::from),
                permission,
                removed_labels: Arc::new(Mutex::new(vec![])),
                comments_posted: Arc::new(Mutex::new(vec![])),
                fail_label_actor: false,
                fail_permission: false,
            }
        }

        fn with_fail_label_actor(mut self) -> Self {
            self.fail_label_actor = true;
            self
        }

        fn with_fail_permission(mut self) -> Self {
            self.fail_permission = true;
            self
        }
    }

    impl GitHubClient for MockGitHubClient {
        async fn list_open_issues(
            &self,
        ) -> anyhow::Result<Vec<crate::application::port::github_client::OpenIssueInfo>> {
            Ok(vec![])
        }

        async fn get_issue(&self, _: u64) -> anyhow::Result<GitHubIssueDetail> {
            unimplemented!()
        }

        async fn find_pr_by_branches(&self, _: &str, _: &str) -> anyhow::Result<Option<GitHubPr>> {
            Ok(None)
        }

        async fn is_pr_merged(&self, _: u64) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn list_unresolved_threads(&self, _: u64) -> anyhow::Result<Vec<ReviewThread>> {
            Ok(vec![])
        }

        async fn create_pr(&self, _: &str, _: &str, _: &str, _: &str) -> anyhow::Result<u64> {
            Ok(0)
        }

        async fn reply_to_thread(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn resolve_thread(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn comment_on_issue(&self, _: u64, body: &str) -> anyhow::Result<()> {
            self.comments_posted
                .lock()
                .expect("lock comments")
                .push(body.to_string());
            Ok(())
        }

        async fn close_issue(&self, _: u64) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_job_logs(&self, _: u64) -> anyhow::Result<String> {
            Ok(String::new())
        }

        async fn get_pr_details(&self, _: u64) -> anyhow::Result<GitHubPrDetails> {
            unimplemented!()
        }

        async fn fetch_label_actor_login(&self, _: u64, _: &str) -> anyhow::Result<Option<String>> {
            if self.fail_label_actor {
                return Err(anyhow::anyhow!("timeline API error"));
            }
            Ok(self.label_actor.clone())
        }

        async fn fetch_user_permission(&self, _: &str) -> anyhow::Result<RepositoryPermission> {
            if self.fail_permission {
                return Err(anyhow::anyhow!("permission API error"));
            }
            Ok(self.permission.clone())
        }

        async fn remove_label(&self, _: u64, label: &str) -> anyhow::Result<()> {
            self.removed_labels
                .lock()
                .expect("lock removed_labels")
                .push(label.to_string());
            Ok(())
        }

        async fn observe_pr(
            &self,
            _: u64,
        ) -> anyhow::Result<Option<crate::application::port::github_client::PrObservation>>
        {
            Ok(None)
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn trusted_associations_all_returns_trusted_without_api_calls() {
        let mock = MockGitHubClient::new(None, RepositoryPermission::Read);
        let result = check_label_actor(&mock, 1, "agent:ready", &TrustedAssociations::All, "owner")
            .await
            .expect("should succeed");
        assert!(matches!(result, AssociationCheckResult::Trusted));
    }

    #[tokio::test]
    async fn trusted_actor_returns_trusted() {
        let mock = MockGitHubClient::new(Some("owner-user"), RepositoryPermission::Admin);
        let trusted = TrustedAssociations::Specific(vec![
            AuthorAssociation::Owner,
            AuthorAssociation::Collaborator,
        ]);
        let result = check_label_actor(&mock, 42, "agent:ready", &trusted, "owner-user")
            .await
            .expect("should succeed");
        assert!(matches!(result, AssociationCheckResult::Trusted));
        // ラベル削除やコメントは行われない
        assert!(mock.removed_labels.lock().expect("lock").is_empty());
        assert!(mock.comments_posted.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn untrusted_actor_returns_rejected_and_removes_label() {
        let mock = MockGitHubClient::new(Some("external-user"), RepositoryPermission::Read);
        let trusted = TrustedAssociations::Specific(vec![
            AuthorAssociation::Owner,
            AuthorAssociation::Member,
            AuthorAssociation::Collaborator,
        ]);
        let result = check_label_actor(&mock, 10, "agent:ready", &trusted, "test-owner")
            .await
            .expect("should succeed");

        assert!(matches!(result, AssociationCheckResult::Rejected { .. }));

        // ラベルが削除される
        let removed = mock.removed_labels.lock().expect("lock");
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "agent:ready");

        // コメントが投稿される
        let comments = mock.comments_posted.lock().expect("lock");
        assert_eq!(comments.len(), 1);
        assert!(comments[0].contains("external-user"));
        assert!(comments[0].contains("NONE"));
    }

    #[tokio::test]
    async fn missing_label_event_returns_error() {
        // label_actor が None の場合（ラベル付与イベントが存在しない）
        let mock = MockGitHubClient::new(None, RepositoryPermission::Admin);
        let trusted = TrustedAssociations::default();
        let result = check_label_actor(&mock, 1, "agent:ready", &trusted, "test-owner").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn timeline_api_failure_returns_error() {
        let mock = MockGitHubClient::new(Some("user"), RepositoryPermission::Admin)
            .with_fail_label_actor();
        let trusted = TrustedAssociations::default();
        let result = check_label_actor(&mock, 1, "agent:ready", &trusted, "test-owner").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn permission_api_failure_returns_error() {
        let mock =
            MockGitHubClient::new(Some("user"), RepositoryPermission::Admin).with_fail_permission();
        let trusted = TrustedAssociations::default();
        let result = check_label_actor(&mock, 1, "agent:ready", &trusted, "test-owner").await;
        assert!(result.is_err());
    }
}
