use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::application::port::file_generator::FileGenerator;

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

    pub fn install_claude_code_assets(&self) -> Result<bool> {
        let mut wrote_any = false;

        for (relative_path, content) in CLAUDE_CODE_ASSETS {
            let path = self.base_dir.join(relative_path);
            if path.exists() {
                continue;
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
}

impl FileGenerator for InitFileGenerator {
    fn generate_toml_template(&self) -> Result<bool> {
        InitFileGenerator::generate_toml_template(self)
    }

    fn install_claude_code_assets(&self) -> Result<bool> {
        InitFileGenerator::install_claude_code_assets(self)
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
        let result = generator.install_claude_code_assets().expect("install");
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

        let result = generator.install_claude_code_assets().expect("install");
        assert!(result);
        assert_eq!(fs::read_to_string(existing).expect("read"), "existing");
    }

    #[test]
    fn install_assets_returns_false_when_everything_exists() {
        let (_, generator) = setup();
        generator.install_claude_code_assets().expect("first");
        let result = generator.install_claude_code_assets().expect("second");
        assert!(!result);
    }

    #[test]
    fn gitignore_appends_entries_when_absent() {
        let (tmp, generator) = setup();
        let result = generator.append_gitignore_entries().expect("append");
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

        let result = generator.append_gitignore_entries().expect("append");
        assert!(result);
        assert!(gitignore_path.exists());
    }

    #[test]
    fn gitignore_skips_when_marker_already_present() {
        let (tmp, generator) = setup();
        let gitignore_path = tmp.path().join(".gitignore");
        fs::write(&gitignore_path, "# cupola\n.cupola/cupola.db\n").expect("write");

        let result = generator.append_gitignore_entries().expect("append");
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

        let result = generator.append_gitignore_entries().expect("append");
        assert!(result);

        let content = fs::read_to_string(&gitignore_path).expect("read");
        assert!(content.contains("node_modules/"));
        assert!(content.contains(GITIGNORE_MARKER));
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
}
