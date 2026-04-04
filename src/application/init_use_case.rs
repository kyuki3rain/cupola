use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::adapter::inbound::cli::InitAgent;
use crate::application::port::command_runner::CommandRunner;
use crate::application::port::db_initializer::DbInitializer;
use crate::application::port::file_generator::FileGenerator;

pub struct InitReport {
    pub db_initialized: bool,
    pub toml_created: bool,
    pub agent_assets_installed: bool,
    pub gitignore_updated: bool,
    pub steering_bootstrap_message: Option<String>,
}

pub struct InitUseCase<D: DbInitializer, F: FileGenerator, R: CommandRunner> {
    base_dir: PathBuf,
    /// DB ファイルが open 前から存在していたかどうか。
    /// `SqliteConnection::open()` はファイルを生成するため、open 後に存在チェックすると
    /// 常に true になってしまう。呼び出し元が open 前に判定して渡す。
    db_existed: bool,
    db_init: D,
    file_gen: F,
    command_runner: R,
    init_agent: InitAgent,
}

impl<D: DbInitializer, F: FileGenerator, R: CommandRunner> InitUseCase<D, F, R> {
    pub fn new(
        base_dir: PathBuf,
        db_existed: bool,
        db_init: D,
        file_gen: F,
        command_runner: R,
        init_agent: InitAgent,
    ) -> Self {
        Self {
            base_dir,
            db_existed,
            db_init,
            file_gen,
            command_runner,
            init_agent,
        }
    }

    pub fn run(&self) -> Result<InitReport> {
        // Step 1: .cupola/ ディレクトリ作成
        let cupola_dir = self.base_dir.join(".cupola");
        std::fs::create_dir_all(&cupola_dir).with_context(|| {
            format!(
                "failed to create .cupola directory at {}",
                cupola_dir.display()
            )
        })?;

        // Step 2: SQLite スキーマ初期化
        self.db_init
            .init_schema()
            .context("failed to initialize SQLite schema")?;
        let db_initialized = !self.db_existed;
        tracing::info!("SQLite schema initialized");

        // Step 3-5: ファイル生成
        let toml_created = self
            .file_gen
            .generate_toml_template()
            .context("failed to generate cupola.toml template")?;

        let agent_assets_installed = self
            .file_gen
            .install_claude_code_assets()
            .context("failed to install Cupola agent assets")?;

        let gitignore_updated = self
            .file_gen
            .append_gitignore_entries()
            .context("failed to append .gitignore entries")?;

        let steering_bootstrap_message = self.bootstrap_steering()?;

        Ok(InitReport {
            db_initialized,
            toml_created,
            agent_assets_installed,
            gitignore_updated,
            steering_bootstrap_message,
        })
    }

    fn bootstrap_steering(&self) -> Result<Option<String>> {
        match self.init_agent {
            InitAgent::ClaudeCode => {}
        }

        let output = self.command_runner.run_in_dir(
            "claude",
            &["-p", "/cupola:steering", "--dangerously-skip-permissions"],
            &self.base_dir,
        )?;

        if output.success {
            return Ok(None);
        }

        let stderr = output.stderr.trim();
        let message = if stderr.contains("command not found") {
            "skipped (claude not installed)".to_string()
        } else if stderr.is_empty() {
            "skipped (claude bootstrap failed)".to_string()
        } else {
            format!("skipped ({stderr})")
        };

        Ok(Some(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    use crate::adapter::outbound::init_file_generator::InitFileGenerator;
    use crate::adapter::outbound::sqlite_connection::SqliteConnection;
    use crate::application::port::command_runner::test_support::MockCommandRunner;

    // === InitUseCase の統合テスト ===

    #[test]
    fn init_use_case_creates_all_files_in_empty_dir() {
        let tmp = TempDir::new().expect("temp dir");
        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        let runner = MockCommandRunner::new().with_failure(
            "claude",
            &["-p", "/cupola:steering", "--dangerously-skip-permissions"],
        );
        let uc = InitUseCase::new(
            tmp.path().to_path_buf(),
            false,
            db,
            file_gen,
            runner,
            InitAgent::ClaudeCode,
        );
        let report = uc.run().expect("run");

        assert!(report.db_initialized, "db should be initialized");
        assert!(report.toml_created, "toml should be created");
        assert!(report.agent_assets_installed, "assets should be installed");
        assert!(report.gitignore_updated, "gitignore should be updated");
        assert!(
            report.steering_bootstrap_message.is_some(),
            "bootstrap should warn when claude is unavailable"
        );

        // ファイルの存在確認
        assert!(tmp.path().join(".cupola").join("cupola.toml").exists());
        assert!(tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn init_use_case_skips_all_when_files_exist() {
        let tmp = TempDir::new().expect("temp dir");

        // 事前にファイルと assets を作成
        let cupola_dir = tmp.path().join(".cupola");
        fs::create_dir_all(&cupola_dir).expect("create .cupola");
        fs::write(cupola_dir.join("cupola.toml"), "existing").expect("write toml");
        fs::write(tmp.path().join(".gitignore"), "# cupola\n").expect("write gitignore");
        InitFileGenerator::new(tmp.path().to_path_buf())
            .install_claude_code_assets()
            .expect("install assets");

        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        let runner = MockCommandRunner::new().with_failure(
            "claude",
            &["-p", "/cupola:steering", "--dangerously-skip-permissions"],
        );
        let uc = InitUseCase::new(
            tmp.path().to_path_buf(),
            false,
            db,
            file_gen,
            runner,
            InitAgent::ClaudeCode,
        );
        let report = uc.run().expect("run");

        assert!(report.db_initialized, "db init is idempotent");
        assert!(!report.toml_created, "toml should be skipped");
        assert!(!report.agent_assets_installed, "assets should be skipped");
        assert!(!report.gitignore_updated, "gitignore should be skipped");

        // 既存ファイルが変更されていないこと
        assert_eq!(
            fs::read_to_string(cupola_dir.join("cupola.toml")).expect("read"),
            "existing"
        );
    }

    #[test]
    fn init_use_case_second_run_skips_everything() {
        let tmp = TempDir::new().expect("temp dir");
        let cupola_dir = tmp.path().join(".cupola");
        fs::create_dir_all(&cupola_dir).expect("create .cupola");
        let db_path = cupola_dir.join("cupola.db");

        // 1回目の実行（ファイルベース SQLite を使用）
        let db_existed_before_first = db_path.exists();
        let db1 = SqliteConnection::open(&db_path).expect("open db");
        let file_gen1 = InitFileGenerator::new(tmp.path().to_path_buf());
        let runner1 = MockCommandRunner::new().with_failure(
            "claude",
            &["-p", "/cupola:steering", "--dangerously-skip-permissions"],
        );
        let uc1 = InitUseCase::new(
            tmp.path().to_path_buf(),
            db_existed_before_first,
            db1,
            file_gen1,
            runner1,
            InitAgent::ClaudeCode,
        );
        uc1.run().expect("first run");

        let toml_content_after_first =
            fs::read_to_string(tmp.path().join(".cupola").join("cupola.toml")).expect("read toml");
        let gitignore_content_after_first =
            fs::read_to_string(tmp.path().join(".gitignore")).expect("read gitignore");

        // 2回目の実行（db ファイルが既に存在するため db_initialized=false）
        let db_existed_before_second = db_path.exists();
        let db2 = SqliteConnection::open(&db_path).expect("open db");
        let file_gen2 = InitFileGenerator::new(tmp.path().to_path_buf());
        let runner2 = MockCommandRunner::new().with_failure(
            "claude",
            &["-p", "/cupola:steering", "--dangerously-skip-permissions"],
        );
        let uc2 = InitUseCase::new(
            tmp.path().to_path_buf(),
            db_existed_before_second,
            db2,
            file_gen2,
            runner2,
            InitAgent::ClaudeCode,
        );
        let report = uc2.run().expect("second run");

        assert!(
            !report.db_initialized,
            "db already existed, should be skipped on 2nd run"
        );
        assert!(!report.toml_created, "toml should be skipped on 2nd run");
        assert!(
            !report.agent_assets_installed,
            "assets should be skipped on 2nd run"
        );
        assert!(
            !report.gitignore_updated,
            "gitignore should be skipped on 2nd run"
        );

        // ファイル内容が変わっていないこと
        assert_eq!(
            fs::read_to_string(tmp.path().join(".cupola").join("cupola.toml")).expect("read"),
            toml_content_after_first
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join(".gitignore")).expect("read"),
            gitignore_content_after_first
        );
    }

    #[test]
    fn init_use_case_reports_successful_steering_bootstrap() {
        let tmp = TempDir::new().expect("temp dir");
        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        let runner = MockCommandRunner::new().with_success(
            "claude",
            &["-p", "/cupola:steering", "--dangerously-skip-permissions"],
            "",
        );
        let uc = InitUseCase::new(
            tmp.path().to_path_buf(),
            false,
            db,
            file_gen,
            runner,
            InitAgent::ClaudeCode,
        );
        let report = uc.run().expect("run");

        assert!(
            report.steering_bootstrap_message.is_none(),
            "bootstrap should report success"
        );
    }

    #[test]
    fn init_use_case_without_claude_skips_bootstrap_runs_rest() {
        let tmp = TempDir::new().expect("temp dir");
        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        let runner = MockCommandRunner::new();
        let uc = InitUseCase::new(
            tmp.path().to_path_buf(),
            false,
            db,
            file_gen,
            runner,
            InitAgent::ClaudeCode,
        );
        let report = uc.run().expect("run");

        assert!(report.db_initialized);
        assert!(report.toml_created);
        assert!(
            report.steering_bootstrap_message.is_some(),
            "should skip bootstrap without claude"
        );
        assert!(report.gitignore_updated);
    }
}
