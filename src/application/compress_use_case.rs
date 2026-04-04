use std::path::Path;

use anyhow::{Context, Result};

pub struct CompressReport {
    pub compressed_count: usize,
    pub skipped_reason: Option<String>,
}

pub struct CompressUseCase {
    specs_dir: std::path::PathBuf,
}

impl CompressUseCase {
    pub fn new(specs_dir: std::path::PathBuf) -> Self {
        Self { specs_dir }
    }

    /// 完了 spec があるかチェックし、結果を返す。
    /// 実際の要約処理は Claude Code の `/cupola:spec-compress` skill が行うため、
    /// この use case は完了 spec の存在確認のみ。
    pub fn find_completed_specs(&self) -> Result<CompressReport> {
        if !self.specs_dir.exists() {
            return Ok(CompressReport {
                compressed_count: 0,
                skipped_reason: Some("specs ディレクトリが存在しません".to_string()),
            });
        }

        let entries = std::fs::read_dir(&self.specs_dir).with_context(|| {
            format!(
                "failed to read specs directory: {}",
                self.specs_dir.display()
            )
        })?;

        let mut completed_count = 0;
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let spec_json_path = entry.path().join("spec.json");
            if !spec_json_path.exists() {
                continue;
            }
            if is_completed_spec(&spec_json_path)? {
                completed_count += 1;
            }
        }

        if completed_count == 0 {
            Ok(CompressReport {
                compressed_count: 0,
                skipped_reason: Some("完了済みの spec が見つかりません".to_string()),
            })
        } else {
            Ok(CompressReport {
                compressed_count: completed_count,
                skipped_reason: None,
            })
        }
    }
}

fn is_completed_spec(spec_json_path: &Path) -> Result<bool> {
    let content = std::fs::read_to_string(spec_json_path)
        .with_context(|| format!("failed to read {}", spec_json_path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", spec_json_path.display()))?;
    let phase = json
        .get("phase")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    Ok(phase == "implementation-complete" || phase == "completed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_completed_specs_returns_zero_when_dir_missing() {
        let tmp = TempDir::new().expect("temp dir");
        let uc = CompressUseCase::new(tmp.path().join("nonexistent"));
        let report = uc.find_completed_specs().expect("find");
        assert_eq!(report.compressed_count, 0);
        assert!(report.skipped_reason.is_some());
    }

    #[test]
    fn find_completed_specs_returns_zero_when_no_completed() {
        let tmp = TempDir::new().expect("temp dir");
        let spec_dir = tmp.path().join("specs");
        fs::create_dir_all(spec_dir.join("issue-1")).expect("create");
        fs::write(
            spec_dir.join("issue-1").join("spec.json"),
            r#"{"phase":"tasks-generated"}"#,
        )
        .expect("write");

        let uc = CompressUseCase::new(spec_dir);
        let report = uc.find_completed_specs().expect("find");
        assert_eq!(report.compressed_count, 0);
        assert!(report.skipped_reason.is_some());
    }

    #[test]
    fn find_completed_specs_finds_completed() {
        let tmp = TempDir::new().expect("temp dir");
        let spec_dir = tmp.path().join("specs");
        fs::create_dir_all(spec_dir.join("issue-1")).expect("create");
        fs::write(
            spec_dir.join("issue-1").join("spec.json"),
            r#"{"phase":"implementation-complete"}"#,
        )
        .expect("write");

        let uc = CompressUseCase::new(spec_dir);
        let report = uc.find_completed_specs().expect("find");
        assert_eq!(report.compressed_count, 1);
        assert!(report.skipped_reason.is_none());
    }

    #[test]
    fn find_completed_specs_skips_archived() {
        let tmp = TempDir::new().expect("temp dir");
        let spec_dir = tmp.path().join("specs");
        fs::create_dir_all(spec_dir.join("issue-1")).expect("create");
        fs::write(
            spec_dir.join("issue-1").join("spec.json"),
            r#"{"phase":"archived"}"#,
        )
        .expect("write");

        let uc = CompressUseCase::new(spec_dir);
        let report = uc.find_completed_specs().expect("find");
        assert_eq!(report.compressed_count, 0);
    }
}
