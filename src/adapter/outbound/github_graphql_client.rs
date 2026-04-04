use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde_json::{Value, json};

use crate::application::port::github_client::{ReviewComment, ReviewThread};
use crate::domain::author_association::AuthorAssociation;

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
            let after_clause = thread_cursor
                .as_deref()
                .map(|c| format!(r#", after: "{c}""#))
                .unwrap_or_default();

            let query = format!(
                r#"query {{
  repository(owner: "{owner}", name: "{repo}") {{
    pullRequest(number: {pr_number}) {{
      reviewThreads(first: 100{after_clause}) {{
        pageInfo {{ hasNextPage endCursor }}
        nodes {{
          id
          isResolved
          path
          line
          comments(first: 100) {{
            nodes {{
              body
              author {{ login }}
              authorAssociation
            }}
          }}
        }}
      }}
    }}
  }}
}}"#,
                owner = self.owner,
                repo = self.repo,
            );

            let resp = self.execute_query(&query).await?;
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

    async fn execute_query(&self, query: &str) -> Result<Value> {
        let payload = json!({ "query": query });
        let resp = self.execute_raw(&payload).await?;
        check_graphql_errors(&resp)?;
        Ok(resp)
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
}
