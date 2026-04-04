use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde_json::{Value, json};

use crate::application::port::github_client::{ReviewComment, ReviewThread};
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

pub struct GraphQLClient {
    client: Client,
    token: String,
    owner: String,
    repo: String,
}

impl GraphQLClient {
    pub fn new(token: String, owner: String, repo: String) -> Self {
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
                thread_cursor = threads_data["pageInfo"]["endCursor"]
                    .as_str()
                    .map(String::from);
            } else {
                break;
            }
        }

        Ok(all_threads)
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
            .header("Authorization", format!("bearer {}", self.token))
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
}
