use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::application::port::file_generator::FileGenerator;

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

pub struct InitFileGenerator {
    base_dir: PathBuf,
}

impl InitFileGenerator {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// cupola.toml 雛形を生成する。既に存在する場合はスキップ。
    /// 戻り値: true = 生成した, false = スキップ
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

    /// steering テンプレートをコピーする。
    /// テンプレート不在時・既にファイルがある場合はスキップ。
    /// 戻り値: true = コピーした, false = スキップ
    pub fn copy_steering_templates(&self) -> Result<bool> {
        let template_dir = self
            .base_dir
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("steering");

        if !template_dir.exists() {
            tracing::info!(
                path = %template_dir.display(),
                "steering template directory not found (cc-sdd not installed?), skipping"
            );
            return Ok(false);
        }

        let steering_dir = self.base_dir.join(".cupola").join("steering");
        std::fs::create_dir_all(&steering_dir).with_context(|| {
            format!(
                "failed to create steering directory at {}",
                steering_dir.display()
            )
        })?;

        // steering/ が空かどうかを確認
        let is_empty = std::fs::read_dir(&steering_dir)
            .with_context(|| {
                format!(
                    "failed to read steering directory at {}",
                    steering_dir.display()
                )
            })?
            .next()
            .is_none();

        if !is_empty {
            tracing::info!(
                path = %steering_dir.display(),
                "steering directory already has files, skipping"
            );
            return Ok(false);
        }

        // テンプレートファイルをコピー
        let entries = std::fs::read_dir(&template_dir).with_context(|| {
            format!(
                "failed to read template directory at {}",
                template_dir.display()
            )
        })?;

        for entry in entries {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read entry in template directory {}",
                    template_dir.display()
                )
            })?;
            let file_name = entry.file_name();
            let dest = steering_dir.join(&file_name);
            std::fs::copy(entry.path(), &dest).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    entry.path().display(),
                    dest.display()
                )
            })?;
            tracing::info!(
                src = %entry.path().display(),
                dst = %dest.display(),
                "steering template copied"
            );
        }

        Ok(true)
    }

    /// .gitignore に cupola 用エントリを追記する。
    /// マーカーが既に存在する場合はスキップ。
    /// 戻り値: true = 追記した, false = スキップ
    pub fn append_gitignore_entries(&self) -> Result<bool> {
        let gitignore_path = self.base_dir.join(".gitignore");

        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path).with_context(|| {
                format!("failed to read .gitignore at {}", gitignore_path.display())
            })?;
            if content.contains(GITIGNORE_MARKER) || content.contains(GITIGNORE_CONTENT_CHECK) {
                tracing::info!(
                    path = %gitignore_path.display(),
                    "cupola entries already present in .gitignore, skipping"
                );
                return Ok(false);
            }
            // 既存の行末スタイルを検出し、同じ改行コードで追記する
            let line_ending = if content.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let separator = if content.ends_with('\n') {
                ""
            } else {
                line_ending
            };
            let entries = GITIGNORE_ENTRIES.replace('\n', line_ending);
            let new_content = format!("{content}{separator}{entries}");
            std::fs::write(&gitignore_path, new_content).with_context(|| {
                format!(
                    "failed to append to .gitignore at {}",
                    gitignore_path.display()
                )
            })?;
        } else {
            // 新規作成
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
}

impl InitFileGenerator {
    /// spec ディレクトリ（spec.json + requirements.md）を生成する。
    /// 既に存在する場合はスキップ。
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

        // Read templates and substitute placeholders
        let template_dir = self
            .base_dir
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("specs");

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        // spec.json from template
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

        // requirements.md from template
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
}

impl FileGenerator for InitFileGenerator {
    fn generate_toml_template(&self) -> Result<bool> {
        InitFileGenerator::generate_toml_template(self)
    }

    fn copy_steering_templates(&self) -> Result<bool> {
        InitFileGenerator::copy_steering_templates(self)
    }

    fn append_gitignore_entries(&self) -> Result<bool> {
        InitFileGenerator::append_gitignore_entries(self)
    }

    fn generate_spec_directory(
        &self,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool> {
        InitFileGenerator::generate_spec_directory(self, issue_number, issue_body, language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, InitFileGenerator) {
        let tmp = TempDir::new().expect("temp dir");
        // .cupola/ ディレクトリを事前作成
        fs::create_dir_all(tmp.path().join(".cupola")).expect("create .cupola");
        let generator = InitFileGenerator::new(tmp.path().to_path_buf());
        (tmp, generator)
    }

    // === generate_toml_template ===

    #[test]
    fn toml_template_creates_file_when_absent() {
        let (tmp, generator) = setup();
        let result = generator.generate_toml_template().expect("generate");
        assert!(result, "should return true when file is created");

        let toml_path = tmp.path().join(".cupola").join("cupola.toml");
        assert!(toml_path.exists(), "cupola.toml should exist");

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
        assert!(!result, "should return false when file already exists");

        // 内容が変更されていないことを確認
        let content = fs::read_to_string(&toml_path).expect("read");
        assert_eq!(content, "existing content");
    }

    // === copy_steering_templates ===

    #[test]
    fn steering_copy_skips_when_template_dir_absent() {
        let (_, generator) = setup();
        // テンプレートディレクトリを作成しない
        let result = generator.copy_steering_templates().expect("copy");
        assert!(!result, "should return false when template dir is absent");
    }

    #[test]
    fn steering_copy_succeeds_when_steering_is_empty() {
        let (tmp, generator) = setup();
        // テンプレートディレクトリを作成してファイルを配置
        let template_dir = tmp
            .path()
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("steering");
        fs::create_dir_all(&template_dir).expect("create template dir");
        fs::write(template_dir.join("product.md"), "# Product").expect("write template");

        // steering/ ディレクトリは空のまま（setup() では作成していない）
        let result = generator.copy_steering_templates().expect("copy");
        assert!(result, "should return true when copy is performed");

        let dest = tmp
            .path()
            .join(".cupola")
            .join("steering")
            .join("product.md");
        assert!(dest.exists(), "product.md should be copied");
        let content = fs::read_to_string(&dest).expect("read");
        assert_eq!(content, "# Product");
    }

    #[test]
    fn steering_copy_skips_when_steering_has_files() {
        let (tmp, generator) = setup();
        // テンプレートディレクトリを作成
        let template_dir = tmp
            .path()
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("steering");
        fs::create_dir_all(&template_dir).expect("create template dir");
        fs::write(template_dir.join("product.md"), "# Product").expect("write template");

        // steering/ に既存ファイルを作成
        let steering_dir = tmp.path().join(".cupola").join("steering");
        fs::create_dir_all(&steering_dir).expect("create steering dir");
        fs::write(steering_dir.join("existing.md"), "existing").expect("write existing");

        let result = generator.copy_steering_templates().expect("copy");
        assert!(
            !result,
            "should return false when steering has existing files"
        );

        // 既存ファイルが残っていること
        assert!(steering_dir.join("existing.md").exists());
        // テンプレートがコピーされていないこと
        assert!(!steering_dir.join("product.md").exists());
    }

    // === append_gitignore_entries ===

    #[test]
    fn gitignore_appends_entries_when_absent() {
        let (tmp, generator) = setup();
        let result = generator.append_gitignore_entries().expect("append");
        assert!(result, "should return true when entries are appended");

        let gitignore_path = tmp.path().join(".gitignore");
        assert!(gitignore_path.exists(), ".gitignore should exist");
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

        let result = generator.append_gitignore_entries().expect("append");
        assert!(result, "should return true when file is created");
        assert!(gitignore_path.exists(), ".gitignore should be created");
    }

    #[test]
    fn gitignore_skips_when_marker_already_present() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(&gitignore_path, "# cupola\n.cupola/cupola.db\n").expect("write");

        let result = generator.append_gitignore_entries().expect("append");
        assert!(!result, "should return false when marker already exists");

        // 内容が変わっていないこと
        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert_eq!(content, "# cupola\n.cupola/cupola.db\n");
    }

    // === generate_spec_directory ===

    #[test]
    fn spec_directory_creates_files_when_absent() {
        let (tmp, generator) = setup();
        // Create settings/templates/specs with init.json and requirements-init.md
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
        assert!(result, "should return true when directory is created");

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
        assert!(!result, "should return false when directory exists");
    }

    #[test]
    fn spec_directory_works_without_templates() {
        let (tmp, generator) = setup();
        // No template files - should use inline fallback
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
    fn gitignore_appends_to_existing_file_without_marker() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(&gitignore_path, "node_modules/\n").expect("write");

        let result = generator.append_gitignore_entries().expect("append");
        assert!(result, "should return true when entries are appended");

        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert!(
            content.contains("node_modules/"),
            "existing content preserved"
        );
        assert!(content.contains(GITIGNORE_MARKER), "cupola marker added");
    }
}
