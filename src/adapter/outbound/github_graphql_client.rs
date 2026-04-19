use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

use crate::application::port::github_client::{
    GitHubCheckRun, PrLevelReview, PrObservation, PrReviewState, PrStatus, ReviewComment,
    ReviewThread,
};
use crate::domain::author_association::AuthorAssociation;

const LIST_THREADS_QUERY: &str = r#"query($owner: String!, $repo: String!, $pr: Int!, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $pr) {
      reviewThreads(first: 100, after: $after) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          isResolved
          path
          line
          comments(first: 100) {
            nodes {
              body
              author { login }
              authorAssociation
            }
          }
        }
      }
    }
  }
}"#;

const OBSERVE_PR_QUERY: &str = r#"query($owner: String!, $repo: String!, $pr: Int!, $threadAfter: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $pr) {
      state
      mergeable
      reviewThreads(first: 100, after: $threadAfter) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          isResolved
          path
          line
          comments(first: 100) {
            nodes {
              body
              author { login }
              authorAssociation
            }
          }
        }
      }
      reviews(last: 100, states: [COMMENTED, CHANGES_REQUESTED]) {
        nodes {
          id
          submittedAt
          body
          state
          author { login }
          authorAssociation
        }
      }
      commits(last: 1) {
        nodes {
          commit {
            checkSuites(first: 10) {
              nodes {
                checkRuns(first: 100) {
                  nodes { databaseId name status conclusion }
                }
              }
            }
          }
        }
      }
    }
  }
}"#;

pub struct GraphQLClient {
    client: Client,
    token: SecretString,
    owner: String,
    repo: String,
}

impl GraphQLClient {
    pub fn new(token: SecretString, owner: String, repo: String) -> Self {
        Self {
            client: Client::new(),
            token,
            owner,
            repo,
        }
    }

    pub async fn list_unresolved_threads(&self, pr_number: u64) -> Result<Vec<ReviewThread>> {
        let mut all_threads = Vec::new();
        let mut thread_cursor: Option<String> = None;

        loop {
            let payload = build_list_threads_payload(
                &self.owner,
                &self.repo,
                pr_number,
                thread_cursor.as_deref(),
            );

            let resp = self.execute_raw(&payload).await?;
            check_graphql_errors(&resp)?;
            let threads_data = &resp["data"]["repository"]["pullRequest"]["reviewThreads"];

            if let Some(nodes) = threads_data["nodes"].as_array() {
                for node in nodes {
                    if node["isResolved"].as_bool() == Some(true) {
                        continue;
                    }
                    if let Some(thread) = parse_review_thread(node) {
                        all_threads.push(thread);
                    }
                }
            }

            let has_next = threads_data["pageInfo"]["hasNextPage"]
                .as_bool()
                .unwrap_or(false);
            if has_next {
                match threads_data["pageInfo"]["endCursor"].as_str() {
                    Some(cursor) => thread_cursor = Some(cursor.to_string()),
                    None => {
                        tracing::warn!(
                            pr_number,
                            "GraphQL hasNextPage=true but endCursor is null, stopping pagination"
                        );
                        break;
                    }
                }
            } else {
                break;
            }
        }

        Ok(all_threads)
    }

    /// Unified PR observation: state, mergeable, unresolved threads, and CI check runs
    /// in a single GraphQL call. Returns `Ok(None)` if the PR does not exist.
    ///
    /// Review threads are paginated (cursor-based). State, mergeable, and check runs
    /// are parsed from the first page only.
    pub async fn observe_pr(&self, pr_number: u64) -> Result<Option<PrObservation>> {
        let mut accumulated_threads = Vec::new();
        let mut thread_cursor: Option<String> = None;
        let mut base_observation: Option<PrObservation> = None;

        loop {
            let after_value = match &thread_cursor {
                Some(cursor) => Value::String(cursor.clone()),
                None => Value::Null,
            };
            let variables = json!({
                "owner": self.owner,
                "repo": self.repo,
                "pr": pr_number,
                "threadAfter": after_value,
            });
            let payload = json!({
                "query": OBSERVE_PR_QUERY,
                "variables": variables,
            });

            let resp = self.execute_raw(&payload).await?;
            check_graphql_errors(&resp)?;

            let pr_data = &resp["data"]["repository"]["pullRequest"];

            if pr_data.is_null() {
                return Ok(None);
            }

            // Parse everything on first page; only threads on subsequent pages
            if base_observation.is_none() {
                let mut obs = match parse_pr_observation(pr_data) {
                    Some(obs) => obs,
                    None => return Ok(None),
                };
                // Threads from first page are already in obs; drain them to accumulated
                accumulated_threads.append(&mut obs.unresolved_threads);
                base_observation = Some(obs);
            } else {
                // Subsequent pages: only parse additional threads
                if let Some(nodes) = pr_data["reviewThreads"]["nodes"].as_array() {
                    for node in nodes {
                        if node["isResolved"].as_bool() == Some(true) {
                            continue;
                        }
                        if let Some(thread) = parse_review_thread(node) {
                            accumulated_threads.push(thread);
                        }
                    }
                }
            }

            let threads_data = &pr_data["reviewThreads"];
            let has_next = threads_data["pageInfo"]["hasNextPage"]
                .as_bool()
                .unwrap_or(false);
            if has_next {
                match threads_data["pageInfo"]["endCursor"].as_str() {
                    Some(cursor) => thread_cursor = Some(cursor.to_string()),
                    None => {
                        tracing::warn!(
                            pr_number,
                            "GraphQL hasNextPage=true but endCursor is null, stopping pagination"
                        );
                        break;
                    }
                }
            } else {
                break;
            }
        }

        Ok(base_observation.map(|mut obs| {
            obs.unresolved_threads = accumulated_threads;
            obs
        }))
    }

    pub async fn reply_to_thread(&self, thread_id: &str, body: &str) -> Result<()> {
        let query = r#"mutation($threadId: ID!, $body: String!) {
  addPullRequestReviewThreadReply(input: {
    pullRequestReviewThreadId: $threadId,
    body: $body
  }) {
    comment { id }
  }
}"#;

        let variables = json!({
            "threadId": thread_id,
            "body": body,
        });

        let payload = json!({
            "query": query,
            "variables": variables,
        });

        let resp = self.execute_raw(&payload).await?;
        check_graphql_errors(&resp)?;

        Ok(())
    }

    pub async fn resolve_thread(&self, thread_id: &str) -> Result<()> {
        let query = r#"mutation($threadId: ID!) {
  resolveReviewThread(input: { threadId: $threadId }) {
    thread { id isResolved }
  }
}"#;

        let variables = json!({ "threadId": thread_id });
        let payload = json!({ "query": query, "variables": variables });

        let resp = self.execute_raw(&payload).await?;
        check_graphql_errors(&resp)?;

        Ok(())
    }

    async fn execute_raw(&self, payload: &Value) -> Result<Value> {
        let response = self
            .client
            .post("https://api.github.com/graphql")
            .header(
                "Authorization",
                format!("bearer {}", self.token.expose_secret()),
            )
            .header("User-Agent", "cupola")
            .json(payload)
            .send()
            .await
            .context("failed to send GraphQL request")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read GraphQL response body")?;

        if !status.is_success() {
            return Err(anyhow!(
                "GraphQL request failed with status {status}: {body}"
            ));
        }

        serde_json::from_str(&body).context("failed to parse GraphQL response as JSON")
    }
}

fn build_list_threads_payload(
    owner: &str,
    repo: &str,
    pr_number: u64,
    after: Option<&str>,
) -> Value {
    let after_value = match after {
        Some(cursor) => Value::String(cursor.to_owned()),
        None => Value::Null,
    };
    let variables = json!({
        "owner": owner,
        "repo": repo,
        "pr": pr_number,
        "after": after_value,
    });
    json!({
        "query": LIST_THREADS_QUERY,
        "variables": variables,
    })
}

/// Parse a PR observation from a GraphQL pullRequest node.
/// Returns `None` if `pr_data` is null (PR does not exist).
fn parse_pr_observation(pr_data: &Value) -> Option<PrObservation> {
    if pr_data.is_null() {
        return None;
    }

    let state = match pr_data["state"].as_str() {
        Some("MERGED") => PrStatus::Merged,
        Some("CLOSED") => PrStatus::Closed,
        _ => PrStatus::Open,
    };

    let mergeable = match pr_data["mergeable"].as_str() {
        Some("MERGEABLE") => Some(true),
        Some("CONFLICTING") => Some(false),
        _ => None,
    };

    // Parse check runs from last commit
    let mut check_runs = Vec::new();
    if let Some(commits) = pr_data["commits"]["nodes"].as_array()
        && let Some(commit_node) = commits.first()
        && let Some(suites) = commit_node["commit"]["checkSuites"]["nodes"].as_array()
    {
        for suite in suites {
            if let Some(runs) = suite["checkRuns"]["nodes"].as_array() {
                for run in runs {
                    let run_status = run["status"].as_str().unwrap_or("").to_string();
                    // Include ALL runs (not just completed) so that
                    // derive_ci_status can detect in-progress runs and
                    // return Unknown instead of falsely reporting Ok.
                    let conclusion = run["conclusion"].as_str().map(|s| s.to_lowercase());
                    check_runs.push(GitHubCheckRun {
                        id: run["databaseId"].as_u64().unwrap_or(0),
                        name: run["name"].as_str().unwrap_or("").to_string(),
                        status: run_status.to_lowercase(),
                        conclusion,
                        output_summary: None,
                        output_text: None,
                    });
                }
            }
        }
    }

    // Parse unresolved review threads
    let mut threads = Vec::new();
    if let Some(nodes) = pr_data["reviewThreads"]["nodes"].as_array() {
        for node in nodes {
            if node["isResolved"].as_bool() == Some(true) {
                continue;
            }
            if let Some(thread) = parse_review_thread(node) {
                threads.push(thread);
            }
        }
    }

    // Parse PR-level reviews (COMMENTED / CHANGES_REQUESTED with non-empty body)
    let pr_level_reviews = parse_pr_level_reviews(pr_data);

    Some(PrObservation {
        state,
        mergeable,
        unresolved_threads: threads,
        check_runs,
        pr_level_reviews,
    })
}

/// GraphQL レスポンスの reviews ノードを走査し、PRレベルレビューのリストを返す。
///
/// - `body` が空文字・空白のみのノードはスキップ
/// - `submittedAt` のパース失敗はスキップしログ出力
/// - `reviews` フィールド自体が存在しない場合は空リストを返し警告ログを出力
fn parse_pr_level_reviews(pr_data: &Value) -> Vec<PrLevelReview> {
    let reviews = &pr_data["reviews"];
    if reviews.is_null() {
        tracing::warn!(
            "GraphQL response missing 'reviews' field; skipping PR-level review detection"
        );
        return Vec::new();
    }

    let nodes = match reviews["nodes"].as_array() {
        Some(n) => n,
        None => {
            tracing::warn!(
                "GraphQL 'reviews.nodes' is not an array; skipping PR-level review detection"
            );
            return Vec::new();
        }
    };

    let mut result = Vec::new();
    for node in nodes {
        // body が空文字・空白のみのノードをスキップ
        let body = node["body"].as_str().unwrap_or("").to_string();
        if body.trim().is_empty() {
            continue;
        }

        // state フィルタ（クエリ側で COMMENTED / CHANGES_REQUESTED に絞っているが念のため）
        let state = match node["state"].as_str() {
            Some("COMMENTED") => PrReviewState::Commented,
            Some("CHANGES_REQUESTED") => PrReviewState::ChangesRequested,
            other => {
                tracing::debug!(
                    "Skipping PR-level review with unexpected state: {:?}",
                    other
                );
                continue;
            }
        };

        // submittedAt パース
        let submitted_at_str = match node["submittedAt"].as_str() {
            Some(s) => s,
            None => {
                tracing::warn!("PR-level review node missing 'submittedAt', skipping");
                continue;
            }
        };
        let submitted_at: DateTime<Utc> = match DateTime::parse_from_rfc3339(submitted_at_str) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(e) => {
                tracing::warn!(
                    "Failed to parse PR-level review submittedAt '{}': {}; skipping",
                    submitted_at_str,
                    e
                );
                continue;
            }
        };

        let id = node["id"].as_str().unwrap_or("").to_string();
        let author = match node["author"]["login"].as_str() {
            Some(login) => login.to_string(),
            None => {
                tracing::warn!(
                    "PR-level review '{}' has no author login (deleted user?); skipping",
                    id
                );
                continue;
            }
        };
        let author_association = node["authorAssociation"]
            .as_str()
            .and_then(|s| s.parse::<AuthorAssociation>().ok())
            .unwrap_or(AuthorAssociation::None);

        result.push(PrLevelReview {
            id,
            submitted_at,
            body,
            state,
            author,
            author_association,
        });
    }

    result
}

fn check_graphql_errors(resp: &Value) -> Result<()> {
    if let Some(errors) = resp["errors"].as_array()
        && !errors.is_empty()
    {
        let messages: Vec<&str> = errors
            .iter()
            .filter_map(|e| e["message"].as_str())
            .collect();
        return Err(anyhow!("GraphQL errors: {}", messages.join("; ")));
    }
    Ok(())
}

fn parse_review_thread(node: &Value) -> Option<ReviewThread> {
    let id = node["id"].as_str()?.to_string();
    let path = node["path"].as_str().unwrap_or("").to_string();
    let line = node["line"].as_u64().map(|l| l as u32);

    let comments = node["comments"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let author_association = c["authorAssociation"]
                        .as_str()
                        .and_then(|s| s.parse::<AuthorAssociation>().ok())
                        .unwrap_or(AuthorAssociation::None);
                    Some(ReviewComment {
                        author: c["author"]["login"].as_str()?.to_string(),
                        body: c["body"].as_str().unwrap_or("").to_string(),
                        author_association,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(ReviewThread {
        id,
        path,
        line,
        comments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_review_thread_from_json() {
        use crate::domain::author_association::AuthorAssociation;

        let json = json!({
            "id": "PRRT_abc123",
            "isResolved": false,
            "path": "src/main.rs",
            "line": 42,
            "comments": {
                "nodes": [
                    {
                        "body": "Fix this",
                        "author": { "login": "reviewer" },
                        "authorAssociation": "COLLABORATOR"
                    },
                    {
                        "body": "Done",
                        "author": { "login": "bot" },
                        "authorAssociation": "NONE"
                    }
                ]
            }
        });

        let thread = parse_review_thread(&json).expect("should parse");
        assert_eq!(thread.id, "PRRT_abc123");
        assert_eq!(thread.path, "src/main.rs");
        assert_eq!(thread.line, Some(42));
        assert_eq!(thread.comments.len(), 2);
        assert_eq!(thread.comments[0].author, "reviewer");
        assert_eq!(thread.comments[0].body, "Fix this");
        assert_eq!(
            thread.comments[0].author_association,
            AuthorAssociation::Collaborator
        );
        assert_eq!(thread.comments[1].author, "bot");
        assert_eq!(
            thread.comments[1].author_association,
            AuthorAssociation::None
        );
    }

    #[test]
    fn parse_review_thread_unknown_association_defaults_to_none() {
        use crate::domain::author_association::AuthorAssociation;

        let json = json!({
            "id": "PRRT_xyz",
            "isResolved": false,
            "path": "src/main.rs",
            "line": 1,
            "comments": {
                "nodes": [
                    {
                        "body": "comment",
                        "author": { "login": "user" },
                        "authorAssociation": "UNKNOWN_VALUE"
                    }
                ]
            }
        });

        let thread = parse_review_thread(&json).expect("should parse");
        assert_eq!(
            thread.comments[0].author_association,
            AuthorAssociation::None
        );
    }

    #[test]
    fn parse_review_thread_without_line() {
        let json = json!({
            "id": "PRRT_xyz",
            "isResolved": false,
            "path": "README.md",
            "line": null,
            "comments": { "nodes": [] }
        });

        let thread = parse_review_thread(&json).expect("should parse");
        assert!(thread.line.is_none());
        assert!(thread.comments.is_empty());
    }

    #[test]
    fn check_graphql_errors_passes_when_no_errors() {
        let resp = json!({"data": {}});
        assert!(check_graphql_errors(&resp).is_ok());
    }

    #[test]
    fn check_graphql_errors_fails_with_errors() {
        let resp = json!({
            "errors": [
                { "message": "Not found" },
                { "message": "Forbidden" }
            ]
        });
        let err = check_graphql_errors(&resp).unwrap_err();
        assert!(err.to_string().contains("Not found"));
        assert!(err.to_string().contains("Forbidden"));
    }

    #[test]
    fn check_graphql_errors_passes_with_empty_errors_array() {
        let resp = json!({"errors": [], "data": {}});
        assert!(check_graphql_errors(&resp).is_ok());
    }

    #[test]
    fn list_threads_payload_contains_variables_with_owner_repo_pr() {
        let payload = build_list_threads_payload("myowner", "myrepo", 42, None);
        let vars = &payload["variables"];
        assert_eq!(vars["owner"], "myowner");
        assert_eq!(vars["repo"], "myrepo");
        assert_eq!(vars["pr"], 42);
        assert!(vars["after"].is_null());
    }

    #[test]
    fn list_threads_query_does_not_contain_literal_owner_repo_pr() {
        // クエリ文字列に owner/repo/pr_number のリテラル値が含まれていないことを確認
        let payload = build_list_threads_payload("secretowner", "secretrepo", 99999, None);
        let query_str = payload["query"].as_str().expect("query should be a string");
        assert!(
            !query_str.contains("secretowner"),
            "query must not contain owner literal"
        );
        assert!(
            !query_str.contains("secretrepo"),
            "query must not contain repo literal"
        );
        assert!(
            !query_str.contains("99999"),
            "query must not contain pr_number literal"
        );
    }

    #[test]
    fn list_threads_payload_passes_cursor_as_string_in_variables() {
        let payload = build_list_threads_payload("owner", "repo", 1, Some("cursor_abc"));
        let vars = &payload["variables"];
        assert_eq!(vars["after"], "cursor_abc");
    }

    #[test]
    fn list_threads_payload_passes_null_cursor_when_none() {
        let payload = build_list_threads_payload("owner", "repo", 1, None);
        assert!(payload["variables"]["after"].is_null());
    }

    // ── parse_pr_observation tests ──────────────────────────────────────

    fn full_pr_data() -> Value {
        json!({
            "state": "OPEN",
            "mergeable": "MERGEABLE",
            "reviewThreads": {
                "pageInfo": { "hasNextPage": false, "endCursor": null },
                "nodes": [
                    {
                        "id": "T1",
                        "isResolved": false,
                        "path": "src/main.rs",
                        "line": 10,
                        "comments": {
                            "nodes": [{
                                "body": "Fix this",
                                "author": { "login": "reviewer" },
                                "authorAssociation": "COLLABORATOR"
                            }]
                        }
                    },
                    {
                        "id": "T2",
                        "isResolved": true,
                        "path": "src/lib.rs",
                        "line": 5,
                        "comments": { "nodes": [] }
                    }
                ]
            },
            "commits": {
                "nodes": [{
                    "commit": {
                        "checkSuites": {
                            "nodes": [{
                                "checkRuns": {
                                    "nodes": [
                                        {
                                            "databaseId": 100,
                                            "name": "ci",
                                            "status": "COMPLETED",
                                            "conclusion": "SUCCESS"
                                        },
                                        {
                                            "databaseId": 101,
                                            "name": "lint",
                                            "status": "IN_PROGRESS",
                                            "conclusion": null
                                        }
                                    ]
                                }
                            }]
                        }
                    }
                }]
            }
        })
    }

    #[test]
    fn parse_pr_observation_full() {
        let data = full_pr_data();
        let obs = parse_pr_observation(&data).expect("should parse");
        assert_eq!(obs.state, PrStatus::Open);
        assert_eq!(obs.mergeable, Some(true));
        // Only unresolved thread (T1) should be included
        assert_eq!(obs.unresolved_threads.len(), 1);
        assert_eq!(obs.unresolved_threads[0].id, "T1");
        // Both check runs should be included (completed + in-progress)
        // so derive_ci_status can detect in-progress and return Unknown
        assert_eq!(obs.check_runs.len(), 2);
        assert_eq!(obs.check_runs[0].id, 100);
        assert_eq!(obs.check_runs[0].name, "ci");
        assert_eq!(obs.check_runs[0].conclusion.as_deref(), Some("success"));
        assert_eq!(obs.check_runs[1].id, 101);
        assert_eq!(obs.check_runs[1].name, "lint");
        assert!(obs.check_runs[1].conclusion.is_none());
    }

    #[test]
    fn parse_pr_observation_null_returns_none() {
        assert!(parse_pr_observation(&Value::Null).is_none());
    }

    #[test]
    fn parse_pr_observation_merged() {
        let data = json!({
            "state": "MERGED",
            "mergeable": "UNKNOWN",
            "reviewThreads": { "pageInfo": { "hasNextPage": false }, "nodes": [] },
            "commits": { "nodes": [] }
        });
        let obs = parse_pr_observation(&data).expect("should parse");
        assert_eq!(obs.state, PrStatus::Merged);
        assert_eq!(obs.mergeable, None);
        assert!(obs.unresolved_threads.is_empty());
        assert!(obs.check_runs.is_empty());
    }

    #[test]
    fn parse_pr_observation_conflicting() {
        let data = json!({
            "state": "OPEN",
            "mergeable": "CONFLICTING",
            "reviewThreads": { "pageInfo": { "hasNextPage": false }, "nodes": [] },
            "commits": { "nodes": [] }
        });
        let obs = parse_pr_observation(&data).expect("should parse");
        assert_eq!(obs.mergeable, Some(false));
    }

    // ── parse_pr_level_reviews tests ─────────────────────────────────────

    #[test]
    fn parse_pr_level_reviews_normal_commented() {
        let data = json!({
            "reviews": {
                "nodes": [{
                    "id": "PRR_1",
                    "submittedAt": "2026-01-01T10:00:00Z",
                    "body": "Please fix this",
                    "state": "COMMENTED",
                    "author": { "login": "reviewer" },
                    "authorAssociation": "COLLABORATOR"
                }]
            }
        });
        let reviews = parse_pr_level_reviews(&data);
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].id, "PRR_1");
        assert_eq!(reviews[0].body, "Please fix this");
        assert_eq!(reviews[0].state, PrReviewState::Commented);
        assert_eq!(reviews[0].author, "reviewer");
        assert_eq!(
            reviews[0].author_association,
            AuthorAssociation::Collaborator
        );
    }

    #[test]
    fn parse_pr_level_reviews_normal_changes_requested() {
        let data = json!({
            "reviews": {
                "nodes": [{
                    "id": "PRR_2",
                    "submittedAt": "2026-01-02T10:00:00Z",
                    "body": "Needs changes",
                    "state": "CHANGES_REQUESTED",
                    "author": { "login": "owner" },
                    "authorAssociation": "OWNER"
                }]
            }
        });
        let reviews = parse_pr_level_reviews(&data);
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].state, PrReviewState::ChangesRequested);
        assert_eq!(reviews[0].author_association, AuthorAssociation::Owner);
    }

    #[test]
    fn parse_pr_level_reviews_skips_empty_body() {
        let data = json!({
            "reviews": {
                "nodes": [
                    {
                        "id": "PRR_3",
                        "submittedAt": "2026-01-03T10:00:00Z",
                        "body": "",
                        "state": "COMMENTED",
                        "author": { "login": "user" },
                        "authorAssociation": "COLLABORATOR"
                    },
                    {
                        "id": "PRR_4",
                        "submittedAt": "2026-01-03T10:00:00Z",
                        "body": "   ",
                        "state": "COMMENTED",
                        "author": { "login": "user" },
                        "authorAssociation": "COLLABORATOR"
                    }
                ]
            }
        });
        let reviews = parse_pr_level_reviews(&data);
        assert!(
            reviews.is_empty(),
            "empty/whitespace-only body must be skipped"
        );
    }

    #[test]
    fn parse_pr_level_reviews_skips_invalid_timestamp() {
        let data = json!({
            "reviews": {
                "nodes": [{
                    "id": "PRR_5",
                    "submittedAt": "not-a-date",
                    "body": "Some review",
                    "state": "COMMENTED",
                    "author": { "login": "user" },
                    "authorAssociation": "NONE"
                }]
            }
        });
        let reviews = parse_pr_level_reviews(&data);
        assert!(reviews.is_empty(), "invalid timestamp must be skipped");
    }

    #[test]
    fn parse_pr_level_reviews_missing_reviews_field_returns_empty() {
        let data = json!({
            "state": "OPEN"
            // reviews フィールドなし
        });
        let reviews = parse_pr_level_reviews(&data);
        assert!(reviews.is_empty());
    }

    #[test]
    fn parse_pr_level_reviews_skips_null_author() {
        let data = json!({
            "reviews": {
                "nodes": [{
                    "id": "PRR_null_author",
                    "submittedAt": "2026-01-05T10:00:00Z",
                    "body": "Some review",
                    "state": "COMMENTED",
                    "author": null,
                    "authorAssociation": "COLLABORATOR"
                }]
            }
        });
        let reviews = parse_pr_level_reviews(&data);
        assert!(reviews.is_empty(), "null author must be skipped");
    }

    #[test]
    fn parse_pr_level_reviews_unknown_association_defaults_to_none() {
        let data = json!({
            "reviews": {
                "nodes": [{
                    "id": "PRR_6",
                    "submittedAt": "2026-01-06T10:00:00Z",
                    "body": "Review body",
                    "state": "COMMENTED",
                    "author": { "login": "user" },
                    "authorAssociation": "UNKNOWN_VALUE"
                }]
            }
        });
        let reviews = parse_pr_level_reviews(&data);
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].author_association, AuthorAssociation::None);
    }
}
