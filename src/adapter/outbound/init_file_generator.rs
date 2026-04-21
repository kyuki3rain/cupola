use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::application::port::file_generator::FileGenerator;
use crate::domain::claude_settings::ClaudeSettings;

const CLAUDE_CODE_ASSETS: &[(&str, &str)] = &[
    (
        ".claude/commands/cupola/spec-compress.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.claude/commands/cupola/spec-compress.md"
        )),
    ),
    (
        ".claude/commands/cupola/spec-design.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.claude/commands/cupola/spec-design.md"
        )),
    ),
    (
        ".claude/commands/cupola/spec-init.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.claude/commands/cupola/spec-init.md"
        )),
    ),
    (
        ".claude/commands/cupola/spec-impl.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.claude/commands/cupola/spec-impl.md"
        )),
    ),
    (
        ".claude/commands/cupola/steering.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.claude/commands/cupola/steering.md"
        )),
    ),
    (
        ".cupola/settings/rules/design-discovery-full.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/design-discovery-full.md"
        )),
    ),
    (
        ".cupola/settings/rules/design-discovery-light.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/design-discovery-light.md"
        )),
    ),
    (
        ".cupola/settings/rules/design-principles.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/design-principles.md"
        )),
    ),
    (
        ".cupola/settings/rules/design-review.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/design-review.md"
        )),
    ),
    (
        ".cupola/settings/rules/ears-format.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/ears-format.md"
        )),
    ),
    (
        ".cupola/settings/rules/gap-analysis.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/gap-analysis.md"
        )),
    ),
    (
        ".cupola/settings/rules/steering-principles.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/steering-principles.md"
        )),
    ),
    (
        ".cupola/settings/rules/tasks-generation.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/tasks-generation.md"
        )),
    ),
    (
        ".cupola/settings/rules/tasks-parallel-analysis.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/rules/tasks-parallel-analysis.md"
        )),
    ),
    (
        ".cupola/settings/templates/specs/design.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/specs/design.md"
        )),
    ),
    (
        ".cupola/settings/templates/specs/init.json",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/specs/init.json"
        )),
    ),
    (
        ".cupola/settings/templates/specs/requirements-init.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/specs/requirements-init.md"
        )),
    ),
    (
        ".cupola/settings/templates/specs/requirements.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/specs/requirements.md"
        )),
    ),
    (
        ".cupola/settings/templates/specs/research.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/specs/research.md"
        )),
    ),
    (
        ".cupola/settings/templates/specs/tasks.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/specs/tasks.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering/product.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering/product.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering/structure.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering/structure.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering/tech.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering/tech.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering-custom/api-standards.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering-custom/api-standards.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering-custom/authentication.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering-custom/authentication.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering-custom/database.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering-custom/database.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering-custom/deployment.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering-custom/deployment.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering-custom/error-handling.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering-custom/error-handling.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering-custom/security.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering-custom/security.md"
        )),
    ),
    (
        ".cupola/settings/templates/steering-custom/testing.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/.cupola/settings/templates/steering-custom/testing.md"
        )),
    ),
];

const CUPOLA_TOML_TEMPLATE: &str = r#"owner = ""
repo = ""
default_branch = ""

# language = "ja"
# polling_interval_secs = 60
# max_retries = 3
# stall_timeout_secs = 1800
# max_concurrent_sessions = 3

# [log]
# level = "info"
# dir = ".cupola/logs"

# モデルを weight 別に指定する例:
# [models]
# light = "haiku"
# heavy = "opus"
#
# フェーズ別に細かく指定する例:
# [models.heavy]
# design = "opus"
# implementation = "opus"
# # design_fix / implementation_fix は上記にフォールバックされます
"#;

const GITIGNORE_MARKER: &str = "# cupola";
const GITIGNORE_CONTENT_CHECK: &str = ".cupola/cupola.db";

const GITIGNORE_ENTRIES: &str = r#"# cupola
.cupola/cupola.db
.cupola/cupola.db-wal
.cupola/cupola.db-shm
.cupola/cupola.pid
.cupola/logs/
.cupola/worktrees/
.cupola/inputs/
"#;

/// `.gitignore` 内の Cupola 管理ブロックを `new_entries` で置き換える。
///
/// ブロックはマーカー行から始まり、最初の空行（またはファイル末尾）で終わる。
/// マーカーより前の行およびブロック終端以降の行はそのまま保持される。
fn replace_cupola_block(content: &str, new_entries: &str, line_ending: &str) -> String {
    let ends_with_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();

    let Some(marker_idx) = lines.iter().position(|l| *l == GITIGNORE_MARKER) else {
        // マーカーが見つからない場合は末尾に追記する（呼び出し元の事前条件違反への安全なフォールバック）
        let separator = if content.ends_with('\n') {
            ""
        } else {
            line_ending
        };
        return format!("{content}{separator}{new_entries}");
    };

    // ブロック終端: マーカーより後の最初の空行、または EOF
    let block_end_idx = lines[marker_idx + 1..]
        .iter()
        .position(|l| l.is_empty())
        .map(|rel| marker_idx + 1 + rel)
        .unwrap_or(lines.len());

    let before_lines = &lines[..marker_idx];
    let after_lines = &lines[block_end_idx..]; // 空行セパレータも含む

    let mut result = String::new();

    if !before_lines.is_empty() {
        result.push_str(&before_lines.join(line_ending));
        result.push_str(line_ending);
    }
    result.push_str(new_entries); // new_entries は line_ending で終わることが保証されている

    if !after_lines.is_empty() {
        result.push_str(&after_lines.join(line_ending));
        if ends_with_newline {
            result.push_str(line_ending);
        }
    }

    result
}

#[derive(Clone)]
pub struct InitFileGenerator {
    base_dir: PathBuf,
}

impl InitFileGenerator {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn generate_toml_template(&self) -> Result<bool> {
        let toml_path = self.base_dir.join(".cupola").join("cupola.toml");
        if toml_path.exists() {
            tracing::info!(
                path = %toml_path.display(),
                "cupola.toml already exists, skipping"
            );
            return Ok(false);
        }
        std::fs::write(&toml_path, CUPOLA_TOML_TEMPLATE)
            .with_context(|| format!("failed to write cupola.toml at {}", toml_path.display()))?;
        tracing::info!(path = %toml_path.display(), "cupola.toml template generated");
        Ok(true)
    }

    pub fn install_claude_code_assets(&self, upgrade: bool) -> Result<bool> {
        let mut wrote_any = false;

        for (relative_path, content) in CLAUDE_CODE_ASSETS {
            let path = self.base_dir.join(relative_path);
            if path.exists() {
                if !upgrade {
                    continue;
                }
                // upgrade=true: skip if content is already identical
                if let Ok(existing) = std::fs::read(&path)
                    && existing == content.as_bytes()
                {
                    continue;
                }
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create parent directory at {}", parent.display())
                })?;
            }

            std::fs::write(&path, content)
                .with_context(|| format!("failed to write asset at {}", path.display()))?;
            tracing::info!(path = %path.display(), "installed Cupola asset");
            wrote_any = true;
        }

        let steering_dir = self.base_dir.join(".cupola").join("steering");
        if !steering_dir.exists() {
            wrote_any = true;
        }
        std::fs::create_dir_all(&steering_dir).with_context(|| {
            format!(
                "failed to create steering directory at {}",
                steering_dir.display()
            )
        })?;

        Ok(wrote_any)
    }

    pub fn append_gitignore_entries(&self, upgrade: bool) -> Result<bool> {
        let gitignore_path = self.base_dir.join(".gitignore");

        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path).with_context(|| {
                format!("failed to read .gitignore at {}", gitignore_path.display())
            })?;
            // Use line-based matching to avoid false positives from substring matches
            // (e.g. a comment like "# see .cupola/cupola.db" should not trigger detection)
            let has_marker = content.lines().any(|l| l == GITIGNORE_MARKER);
            let has_cupola = has_marker || content.lines().any(|l| l == GITIGNORE_CONTENT_CHECK);
            if has_cupola && !upgrade {
                tracing::info!(
                    path = %gitignore_path.display(),
                    "cupola entries already present in .gitignore, skipping"
                );
                return Ok(false);
            }
            let line_ending = if content.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let entries = GITIGNORE_ENTRIES.replace('\n', line_ending);
            let new_content = if has_marker && upgrade {
                // upgrade=true かつマーカーあり: マーカー行から最初の空行（またはEOF）までの
                // Cupola 管理ブロックのみを新しいエントリで置き換え、それ以外は保持する
                replace_cupola_block(&content, &entries, line_ending)
            } else {
                // マーカーなし（または upgrade=false）: 末尾に追記
                let separator = if content.ends_with('\n') {
                    ""
                } else {
                    line_ending
                };
                format!("{content}{separator}{entries}")
            };
            if new_content == content {
                tracing::info!(
                    path = %gitignore_path.display(),
                    "cupola entries already up to date in .gitignore, skipping"
                );
                return Ok(false);
            }
            std::fs::write(&gitignore_path, new_content).with_context(|| {
                format!("failed to write .gitignore at {}", gitignore_path.display())
            })?;
        } else {
            std::fs::write(&gitignore_path, GITIGNORE_ENTRIES).with_context(|| {
                format!(
                    "failed to create .gitignore at {}",
                    gitignore_path.display()
                )
            })?;
        }

        tracing::info!(path = %gitignore_path.display(), "cupola entries appended to .gitignore");
        Ok(true)
    }

    pub fn generate_spec_directory(
        &self,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool> {
        let feature_name = format!("issue-{issue_number}");
        let spec_dir = self
            .base_dir
            .join(".cupola")
            .join("specs")
            .join(&feature_name);

        if spec_dir.exists() {
            tracing::info!(
                path = %spec_dir.display(),
                "spec directory already exists, skipping"
            );
            return Ok(false);
        }

        std::fs::create_dir_all(&spec_dir).with_context(|| {
            format!("failed to create spec directory at {}", spec_dir.display())
        })?;

        let template_dir = self
            .base_dir
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("specs");

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let spec_json_path = spec_dir.join("spec.json");
        let spec_json_template = template_dir.join("init.json");
        let spec_json_content = if spec_json_template.exists() {
            let tmpl = std::fs::read_to_string(&spec_json_template)
                .with_context(|| format!("failed to read {}", spec_json_template.display()))?;
            tmpl.replace("{{FEATURE_NAME}}", &feature_name)
                .replace("{{TIMESTAMP}}", &now)
                .replace("{{LANGUAGE}}", language)
        } else {
            format!(
                r#"{{"feature_name":"{feature_name}","created_at":"{now}","updated_at":"{now}","language":"{language}","phase":"initialized","approvals":{{"requirements":{{"generated":false,"approved":false}},"design":{{"generated":false,"approved":false}},"tasks":{{"generated":false,"approved":false}}}},"ready_for_implementation":false}}"#
            )
        };
        std::fs::write(&spec_json_path, spec_json_content).with_context(|| {
            format!("failed to write spec.json at {}", spec_json_path.display())
        })?;

        let req_path = spec_dir.join("requirements.md");
        let req_template = template_dir.join("requirements-init.md");
        let req_content = if req_template.exists() {
            let tmpl = std::fs::read_to_string(&req_template)
                .with_context(|| format!("failed to read {}", req_template.display()))?;
            tmpl.replace("{{PROJECT_DESCRIPTION}}", issue_body)
        } else {
            format!(
                "# Requirements Document\n\n## Project Description (Input)\n{issue_body}\n\n## Requirements\n<!-- Will be generated by /cupola:spec-design -->\n"
            )
        };
        std::fs::write(&req_path, req_content).with_context(|| {
            format!("failed to write requirements.md at {}", req_path.display())
        })?;

        tracing::info!(
            path = %spec_dir.display(),
            feature_name = %feature_name,
            "spec directory created with spec.json and requirements.md"
        );
        Ok(true)
    }

    pub fn write_claude_settings(&self, settings: &ClaudeSettings) -> Result<bool> {
        let claude_dir = self.base_dir.join(".claude");
        std::fs::create_dir_all(&claude_dir).with_context(|| {
            format!(
                "failed to create .claude directory at {}",
                claude_dir.display()
            )
        })?;

        let settings_path = claude_dir.join("settings.json");
        let managed =
            serde_json::to_value(settings).context("failed to serialize ClaudeSettings")?;

        let existing = if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)
                .with_context(|| format!("failed to read {}", settings_path.display()))?;
            Some(
                serde_json::from_str::<serde_json::Value>(&content).with_context(|| {
                    format!(
                        "failed to parse existing {}: please fix the JSON manually",
                        settings_path.display()
                    )
                })?,
            )
        } else {
            None
        };

        let merged = if let Some(existing_val) = existing.as_ref() {
            deep_merge_json(existing_val.clone(), managed)
        } else {
            managed
        };

        if existing
            .as_ref()
            .is_some_and(|existing_val| existing_val == &merged)
        {
            return Ok(false);
        }

        let new_content =
            serde_json::to_string_pretty(&merged).context("failed to serialize settings")?;

        std::fs::write(&settings_path, &new_content)
            .with_context(|| format!("failed to write {}", settings_path.display()))?;
        tracing::info!(path = %settings_path.display(), "wrote .claude/settings.json");
        Ok(true)
    }
}

/// 2 つの JSON Value をディープマージする。
///
/// ルール:
/// - `permissions.allow` / `permissions.deny` 配列は union マージ（重複排除）
/// - スカラーキー: existing 優先 (existing に値があれば managed は無視)
/// - ネストオブジェクト: 再帰的に同ルールを適用
/// - 配列（上記 allow/deny 以外）: existing 優先
fn deep_merge_json(existing: serde_json::Value, managed: serde_json::Value) -> serde_json::Value {
    deep_merge_json_inner(existing, managed, false, false)
}

fn deep_merge_json_inner(
    existing: serde_json::Value,
    managed: serde_json::Value,
    in_permissions: bool,
    union_merge: bool,
) -> serde_json::Value {
    use serde_json::Value;
    match (existing, managed) {
        (Value::Object(mut existing_map), Value::Object(managed_map)) => {
            for (key, managed_val) in managed_map {
                let child_in_permissions = key == "permissions";
                let child_union_merge = in_permissions && (key == "allow" || key == "deny");
                if let Some(existing_val) = existing_map.remove(&key) {
                    let merged = deep_merge_json_inner(
                        existing_val,
                        managed_val,
                        child_in_permissions,
                        child_union_merge,
                    );
                    existing_map.insert(key, merged);
                } else {
                    existing_map.insert(key, managed_val);
                }
            }
            Value::Object(existing_map)
        }
        // allow/deny: union merge
        (Value::Array(existing_arr), Value::Array(managed_arr)) if union_merge => {
            let mut seen: HashSet<String> = HashSet::new();
            let mut result: Vec<Value> = Vec::new();
            for v in existing_arr.iter().chain(managed_arr.iter()) {
                if let Some(s) = v.as_str() {
                    if seen.insert(s.to_string()) {
                        result.push(v.clone());
                    }
                } else {
                    result.push(v.clone());
                }
            }
            Value::Array(result)
        }
        // その他: existing 優先
        (existing, _managed) => existing,
    }
}

impl FileGenerator for InitFileGenerator {
    fn generate_toml_template(&self) -> Result<bool> {
        InitFileGenerator::generate_toml_template(self)
    }

    fn install_claude_code_assets(&self, upgrade: bool) -> Result<bool> {
        InitFileGenerator::install_claude_code_assets(self, upgrade)
    }

    fn append_gitignore_entries(&self, upgrade: bool) -> Result<bool> {
        InitFileGenerator::append_gitignore_entries(self, upgrade)
    }

    fn generate_spec_directory(
        &self,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool> {
        InitFileGenerator::generate_spec_directory(self, issue_number, issue_body, language)
    }

    fn generate_spec_directory_at(
        &self,
        base_dir: &Path,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool> {
        InitFileGenerator::new(base_dir.to_path_buf()).generate_spec_directory(
            issue_number,
            issue_body,
            language,
        )
    }

    fn write_claude_settings(&self, settings: &ClaudeSettings) -> Result<bool> {
        InitFileGenerator::write_claude_settings(self, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, InitFileGenerator) {
        let tmp = TempDir::new().expect("temp dir");
        fs::create_dir_all(tmp.path().join(".cupola")).expect("create .cupola");
        let generator = InitFileGenerator::new(tmp.path().to_path_buf());
        (tmp, generator)
    }

    #[test]
    fn toml_template_creates_file_when_absent() {
        let (tmp, generator) = setup();
        let result = generator.generate_toml_template().expect("generate");
        assert!(result);

        let toml_path = tmp.path().join(".cupola").join("cupola.toml");
        assert!(toml_path.exists());

        let content = fs::read_to_string(&toml_path).expect("read");
        assert!(content.contains("owner = \"\""));
        assert!(content.contains("repo = \"\""));
        assert!(content.contains("default_branch = \"\""));
        assert!(content.contains("# language = \"ja\""));
    }

    #[test]
    fn toml_template_skips_when_file_exists() {
        let (tmp, generator) = setup();
        let toml_path = tmp.path().join(".cupola").join("cupola.toml");
        fs::write(&toml_path, "existing content").expect("write");

        let result = generator.generate_toml_template().expect("generate");
        assert!(!result);
        assert_eq!(
            fs::read_to_string(&toml_path).expect("read"),
            "existing content"
        );
    }

    #[test]
    fn install_assets_writes_embedded_files() {
        let (tmp, generator) = setup();
        let result = generator
            .install_claude_code_assets(false)
            .expect("install");
        assert!(result);

        assert!(
            tmp.path()
                .join(".claude")
                .join("commands")
                .join("cupola")
                .join("spec-design.md")
                .exists()
        );
        assert!(
            tmp.path()
                .join(".cupola")
                .join("settings")
                .join("templates")
                .join("steering")
                .join("product.md")
                .exists()
        );
        assert!(tmp.path().join(".cupola").join("steering").exists());
    }

    #[test]
    fn install_assets_preserves_existing_file() {
        let (tmp, generator) = setup();
        let existing = tmp
            .path()
            .join(".claude")
            .join("commands")
            .join("cupola")
            .join("spec-design.md");
        fs::create_dir_all(existing.parent().expect("parent")).expect("create parent");
        fs::write(&existing, "existing").expect("write");

        let result = generator
            .install_claude_code_assets(false)
            .expect("install");
        assert!(result);
        assert_eq!(fs::read_to_string(existing).expect("read"), "existing");
    }

    #[test]
    fn install_assets_returns_false_when_everything_exists() {
        let (_, generator) = setup();
        generator.install_claude_code_assets(false).expect("first");
        let result = generator.install_claude_code_assets(false).expect("second");
        assert!(!result);
    }

    #[test]
    fn install_assets_upgrade_overwrites_existing_managed_files() {
        let (tmp, generator) = setup();
        // 1回目: 通常インストール
        generator
            .install_claude_code_assets(false)
            .expect("first install");

        // Cupola 管理ファイルを書き換え
        let spec_design = tmp
            .path()
            .join(".claude")
            .join("commands")
            .join("cupola")
            .join("spec-design.md");
        fs::write(&spec_design, "old content").expect("write old content");

        // upgrade=true: 管理ファイルが上書きされる
        let result = generator.install_claude_code_assets(true).expect("upgrade");
        assert!(result, "upgrade should report files were written");
        assert_ne!(
            fs::read_to_string(&spec_design).expect("read"),
            "old content",
            "managed file should be overwritten on upgrade"
        );
    }

    #[test]
    fn install_assets_upgrade_returns_false_when_all_content_identical() {
        let (_tmp, generator) = setup();
        // 1回目: 通常インストール（全ファイルを埋め込みコンテンツで書き込む）
        generator
            .install_claude_code_assets(false)
            .expect("first install");
        // 2回目: upgrade=true だが内容が同一なのでスキップ
        let result = generator
            .install_claude_code_assets(true)
            .expect("upgrade no-op");
        assert!(
            !result,
            "upgrade should return false when all embedded content is already identical to disk"
        );
    }

    #[test]
    fn gitignore_appends_entries_when_absent() {
        let (tmp, generator) = setup();
        let result = generator.append_gitignore_entries(false).expect("append");
        assert!(result);

        let gitignore_path = tmp.path().join(".gitignore");
        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert!(content.contains(GITIGNORE_MARKER));
        assert!(content.contains(".cupola/cupola.db"));
        assert!(content.contains(".cupola/worktrees/"));
    }

    #[test]
    fn gitignore_creates_new_file_when_not_exists() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        assert!(!gitignore_path.exists());

        let result = generator.append_gitignore_entries(false).expect("append");
        assert!(result);
        assert!(gitignore_path.exists());
    }

    #[test]
    fn gitignore_skips_when_marker_already_present() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(&gitignore_path, "# cupola\n.cupola/cupola.db\n").expect("write");

        let result = generator.append_gitignore_entries(false).expect("append");
        assert!(!result);
        assert_eq!(
            fs::read_to_string(&gitignore_path).expect("read"),
            "# cupola\n.cupola/cupola.db\n"
        );
    }

    #[test]
    fn gitignore_appends_to_existing_file_without_marker() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(&gitignore_path, "node_modules/\n").expect("write");

        let result = generator.append_gitignore_entries(false).expect("append");
        assert!(result);

        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert!(content.contains("node_modules/"));
        assert!(content.contains(GITIGNORE_MARKER));
    }

    #[test]
    fn gitignore_upgrade_replaces_existing_cupola_section() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(&gitignore_path, "node_modules/\n# cupola\nold-entry\n").expect("write");

        let result = generator.append_gitignore_entries(true).expect("upgrade");
        assert!(result, "upgrade should report the file was written");

        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert!(
            content.contains("node_modules/"),
            "non-cupola entries preserved"
        );
        assert!(!content.contains("old-entry"), "old cupola entries removed");
        assert!(
            content.contains(".cupola/cupola.db"),
            "new cupola entries present"
        );
    }

    #[test]
    fn gitignore_upgrade_preserves_content_after_cupola_section() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(
            &gitignore_path,
            "node_modules/\n# cupola\n.cupola/cupola.db\nold-entry\n\ndist/\nuser-rule/\n",
        )
        .expect("write");

        let result = generator.append_gitignore_entries(true).expect("upgrade");
        assert!(result);

        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert!(
            content.contains("node_modules/"),
            "entries before cupola preserved"
        );
        assert!(!content.contains("old-entry"), "old cupola entries removed");
        assert!(
            content.contains(".cupola/cupola.db"),
            "new cupola entries present"
        );
        assert!(
            content.contains("dist/"),
            "entries after cupola block preserved"
        );
        assert!(
            content.contains("user-rule/"),
            "entries after cupola block preserved"
        );
    }

    #[test]
    fn gitignore_upgrade_adds_section_when_absent() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(&gitignore_path, "node_modules/\n").expect("write");

        let result = generator.append_gitignore_entries(true).expect("upgrade");
        assert!(result);

        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".cupola/cupola.db"));
    }

    #[test]
    fn spec_directory_creates_files_when_absent() {
        let (tmp, generator) = setup();
        let tmpl_dir = tmp
            .path()
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("specs");
        fs::create_dir_all(&tmpl_dir).expect("create template dir");
        fs::write(
            tmpl_dir.join("init.json"),
            r#"{"feature_name":"{{FEATURE_NAME}}","created_at":"{{TIMESTAMP}}","updated_at":"{{TIMESTAMP}}","language":"ja","phase":"initialized"}"#,
        )
        .expect("write init.json");
        fs::write(
            tmpl_dir.join("requirements-init.md"),
            "# Requirements\n\n## Project Description\n{{PROJECT_DESCRIPTION}}\n",
        )
        .expect("write requirements-init.md");

        let result = generator
            .generate_spec_directory(42, "Fix the bug", "ja")
            .expect("generate");
        assert!(result);

        let spec_dir = tmp.path().join(".cupola").join("specs").join("issue-42");
        assert!(spec_dir.join("spec.json").exists());
        assert!(spec_dir.join("requirements.md").exists());

        let spec_json = fs::read_to_string(spec_dir.join("spec.json")).expect("read");
        assert!(spec_json.contains("issue-42"));
        assert!(!spec_json.contains("{{FEATURE_NAME}}"));

        let req = fs::read_to_string(spec_dir.join("requirements.md")).expect("read");
        assert!(req.contains("Fix the bug"));
        assert!(!req.contains("{{PROJECT_DESCRIPTION}}"));
    }

    #[test]
    fn spec_directory_skips_when_exists() {
        let (tmp, generator) = setup();
        let spec_dir = tmp.path().join(".cupola").join("specs").join("issue-42");
        fs::create_dir_all(&spec_dir).expect("create spec dir");

        let result = generator
            .generate_spec_directory(42, "Fix the bug", "ja")
            .expect("generate");
        assert!(!result);
    }

    #[test]
    fn spec_directory_works_without_templates() {
        let (tmp, generator) = setup();
        let result = generator
            .generate_spec_directory(99, "New feature", "ja")
            .expect("generate");
        assert!(result);

        let spec_dir = tmp.path().join(".cupola").join("specs").join("issue-99");
        let spec_json = fs::read_to_string(spec_dir.join("spec.json")).expect("read");
        assert!(spec_json.contains("issue-99"));

        let req = fs::read_to_string(spec_dir.join("requirements.md")).expect("read");
        assert!(req.contains("New feature"));
    }

    #[test]
    fn spec_directory_at_writes_into_target_base_dir() {
        let (tmp, generator) = setup();
        let worktree_dir = tmp.path().join("worktree");
        fs::create_dir_all(worktree_dir.join(".cupola")).expect("create worktree .cupola");
        let tmpl_dir = worktree_dir
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("specs");
        fs::create_dir_all(&tmpl_dir).expect("create template dir");
        fs::write(
            tmpl_dir.join("init.json"),
            r#"{"feature_name":"{{FEATURE_NAME}}","created_at":"{{TIMESTAMP}}","updated_at":"{{TIMESTAMP}}","language":"{{LANGUAGE}}","phase":"initialized"}"#,
        )
        .expect("write init.json");

        let result = generator
            .generate_spec_directory_at(&worktree_dir, 193, "Issue body", "en")
            .expect("generate");
        assert!(result);

        let spec_dir = worktree_dir.join(".cupola").join("specs").join("issue-193");
        assert!(spec_dir.join("spec.json").exists());
        assert!(spec_dir.join("requirements.md").exists());
        assert!(
            !tmp.path()
                .join(".cupola")
                .join("specs")
                .join("issue-193")
                .exists()
        );
    }

    // --- write_claude_settings tests ---

    use crate::domain::claude_settings::{ClaudePermissions, ClaudeSettings};

    fn make_settings(allow: &[&str], deny: &[&str]) -> ClaudeSettings {
        ClaudeSettings {
            permissions: ClaudePermissions {
                allow: allow.iter().map(|s| s.to_string()).collect(),
                deny: deny.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    #[test]
    fn write_claude_settings_creates_new_file() {
        let (tmp, generator) = setup();
        let settings = make_settings(&["Read", "Bash(git status)"], &["Bash(rm -rf*)"]);

        let result = generator.write_claude_settings(&settings).expect("write");
        assert!(result, "should report file was written");

        let path = tmp.path().join(".claude").join("settings.json");
        assert!(path.exists());
        let content = fs::read_to_string(&path).expect("read");
        assert!(content.contains("Read"));
        assert!(content.contains("Bash(git status)"));
        assert!(content.contains("Bash(rm -rf*)"));
    }

    #[test]
    fn write_claude_settings_merges_with_existing_allow() {
        let (tmp, generator) = setup();
        let settings_path = tmp.path().join(".claude").join("settings.json");
        fs::create_dir_all(settings_path.parent().unwrap()).expect("create .claude");
        fs::write(
            &settings_path,
            r#"{"permissions":{"allow":["Bash(existing-cmd)"],"deny":[]}}"#,
        )
        .expect("write existing");

        let settings = make_settings(&["Read"], &[]);
        generator.write_claude_settings(&settings).expect("write");

        let content = fs::read_to_string(&settings_path).expect("read");
        assert!(content.contains("existing-cmd"), "existing allow preserved");
        assert!(content.contains("Read"), "new allow added");
    }

    #[test]
    fn write_claude_settings_scalar_existing_wins() {
        let (tmp, generator) = setup();
        let settings_path = tmp.path().join(".claude").join("settings.json");
        fs::create_dir_all(settings_path.parent().unwrap()).expect("create .claude");
        fs::write(
            &settings_path,
            r#"{"customKey":"user-value","permissions":{"allow":[],"deny":[]}}"#,
        )
        .expect("write existing");

        let settings = make_settings(&[], &[]);
        generator.write_claude_settings(&settings).expect("write");

        let content = fs::read_to_string(&settings_path).expect("read");
        assert!(
            content.contains("user-value"),
            "scalar from existing wins: {content}"
        );
    }

    #[test]
    fn write_claude_settings_no_change_returns_false() {
        let (tmp, generator) = setup();

        let settings = make_settings(&["Read"], &[]);
        generator
            .write_claude_settings(&settings)
            .expect("first write");

        let result = generator
            .write_claude_settings(&settings)
            .expect("second write");
        let _ = tmp;
        assert!(
            !result,
            "second write with same content should return false"
        );
    }

    #[test]
    fn write_claude_settings_union_merge_no_duplicates() {
        let (tmp, generator) = setup();
        let settings_path = tmp.path().join(".claude").join("settings.json");
        fs::create_dir_all(settings_path.parent().unwrap()).expect("create .claude");
        fs::write(
            &settings_path,
            r#"{"permissions":{"allow":["Read","Bash(git status)"],"deny":[]}}"#,
        )
        .expect("write existing");

        let settings = make_settings(&["Read", "Write"], &[]);
        generator.write_claude_settings(&settings).expect("write");

        let content = fs::read_to_string(&settings_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        let allow = parsed["permissions"]["allow"].as_array().expect("array");
        let read_count = allow.iter().filter(|v| v.as_str() == Some("Read")).count();
        assert_eq!(
            read_count, 1,
            "Read should appear only once after union merge"
        );
    }
}
