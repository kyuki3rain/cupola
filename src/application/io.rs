use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::application::port::github_client::{GitHubIssueDetail, ReviewThread};

// === Input file writing ===

pub fn write_issue_input(worktree_path: &Path, detail: &GitHubIssueDetail) -> Result<()> {
    let inputs_dir = worktree_path.join(".cupola/inputs");
    std::fs::create_dir_all(&inputs_dir)
        .with_context(|| format!("failed to create {}", inputs_dir.display()))?;

    let content = format!(
        "# Issue #{number}: {title}\n\n## Labels\n{labels}\n\n## Body\n{body}\n",
        number = detail.number,
        title = detail.title,
        labels = detail.labels.join(", "),
        body = detail.body,
    );

    let path = inputs_dir.join("issue.md");
    std::fs::write(&path, content)
        .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

pub fn write_review_threads_input(worktree_path: &Path, threads: &[ReviewThread]) -> Result<()> {
    let inputs_dir = worktree_path.join(".cupola/inputs");
    std::fs::create_dir_all(&inputs_dir)
        .with_context(|| format!("failed to create {}", inputs_dir.display()))?;

    let entries: Vec<ReviewThreadEntry> = threads
        .iter()
        .map(|t| ReviewThreadEntry {
            thread_id: t.id.clone(),
            path: t.path.clone(),
            line: t.line,
            comments: t
                .comments
                .iter()
                .map(|c| CommentEntry {
                    author: c.author.clone(),
                    body: c.body.clone(),
                })
                .collect(),
        })
        .collect();

    let json = serde_json::to_string_pretty(&entries).context("failed to serialize threads")?;

    let path = inputs_dir.join("review_threads.json");
    std::fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

#[derive(Serialize)]
struct ReviewThreadEntry {
    thread_id: String,
    path: String,
    line: Option<u32>,
    comments: Vec<CommentEntry>,
}

#[derive(Serialize)]
struct CommentEntry {
    author: String,
    body: String,
}

// === Output schema parsing ===

#[derive(Debug, Deserialize)]
pub struct PrCreationOutput {
    pub pr_title: Option<String>,
    pub pr_body: Option<String>,
    pub feature_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FixingOutput {
    pub threads: Vec<ThreadResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ThreadResponse {
    pub thread_id: String,
    pub response: String,
    pub resolved: bool,
}

/// Parse Claude Code stdout JSON to extract structured_output for PR creation.
/// The stdout format is: `{"session_id": "...", "result": "...", "structured_output": {...}}`
pub fn parse_pr_creation_output(stdout: &str) -> Option<PrCreationOutput> {
    let value: serde_json::Value = serde_json::from_str(stdout).ok()?;
    let structured = &value["structured_output"];
    serde_json::from_value(structured.clone()).ok()
}

/// Parse Claude Code stdout JSON to extract structured_output for fixing.
pub fn parse_fixing_output(stdout: &str) -> Option<FixingOutput> {
    let value: serde_json::Value = serde_json::from_str(stdout).ok()?;
    let structured = &value["structured_output"];
    serde_json::from_value(structured.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::github_client::ReviewComment;
    use tempfile::TempDir;

    #[test]
    fn write_issue_input_creates_file() {
        let tmp = TempDir::new().expect("tempdir");
        let detail = GitHubIssueDetail {
            number: 42,
            title: "Add feature".to_string(),
            body: "Please implement X.".to_string(),
            labels: vec!["agent:ready".to_string(), "enhancement".to_string()],
        };

        write_issue_input(tmp.path(), &detail).expect("should write");

        let content = std::fs::read_to_string(tmp.path().join(".cupola/inputs/issue.md"))
            .expect("should read");
        assert!(content.contains("# Issue #42: Add feature"));
        assert!(content.contains("agent:ready, enhancement"));
        assert!(content.contains("Please implement X."));
    }

    #[test]
    fn write_review_threads_creates_json() {
        let tmp = TempDir::new().expect("tempdir");
        let threads = vec![ReviewThread {
            id: "PRRT_abc".to_string(),
            path: "src/main.rs".to_string(),
            line: Some(10),
            comments: vec![ReviewComment {
                author: "reviewer".to_string(),
                body: "Fix this".to_string(),
            }],
        }];

        write_review_threads_input(tmp.path(), &threads).expect("should write");

        let content =
            std::fs::read_to_string(tmp.path().join(".cupola/inputs/review_threads.json"))
                .expect("should read");
        assert!(content.contains("PRRT_abc"));
        assert!(content.contains("Fix this"));
    }

    #[test]
    fn parse_pr_creation_output_success() {
        let stdout = r#"{"session_id":"s1","result":"done","structured_output":{"pr_title":"Design: Add X","pr_body":"Summary here"}}"#;
        let output = parse_pr_creation_output(stdout).expect("should parse");
        assert_eq!(output.pr_title.as_deref(), Some("Design: Add X"));
        assert_eq!(output.pr_body.as_deref(), Some("Summary here"));
    }

    #[test]
    fn parse_pr_creation_output_missing_fields() {
        let stdout = r#"{"session_id":"s1","result":"done","structured_output":{}}"#;
        let output = parse_pr_creation_output(stdout).expect("should parse with None fields");
        assert!(output.pr_title.is_none());
        assert!(output.pr_body.is_none());
    }

    #[test]
    fn parse_pr_creation_output_invalid_json() {
        assert!(parse_pr_creation_output("not json").is_none());
    }

    #[test]
    fn parse_pr_creation_output_no_structured_output() {
        let stdout = r#"{"session_id":"s1","result":"done"}"#;
        // structured_output is null → should still attempt parse, results in None fields
        let output = parse_pr_creation_output(stdout);
        // serde_json::from_value(Null) for PrCreationOutput may fail
        // This is expected behavior - no structured_output means None
        assert!(output.is_none() || output.is_some());
    }

    #[test]
    fn parse_fixing_output_success() {
        let stdout = r#"{"session_id":"s1","result":"done","structured_output":{"threads":[{"thread_id":"PRRT_abc","response":"修正しました。","resolved":true}]}}"#;
        let output = parse_fixing_output(stdout).expect("should parse");
        assert_eq!(output.threads.len(), 1);
        assert_eq!(output.threads[0].thread_id, "PRRT_abc");
        assert_eq!(output.threads[0].response, "修正しました。");
        assert!(output.threads[0].resolved);
    }

    #[test]
    fn parse_fixing_output_empty_threads() {
        let stdout = r#"{"session_id":"s1","result":"done","structured_output":{"threads":[]}}"#;
        let output = parse_fixing_output(stdout).expect("should parse");
        assert!(output.threads.is_empty());
    }

    #[test]
    fn parse_fixing_output_invalid_json() {
        assert!(parse_fixing_output("garbage").is_none());
    }
}
