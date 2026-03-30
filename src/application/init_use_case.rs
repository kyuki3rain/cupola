use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::adapter::outbound::init_file_generator::InitFileGenerator;
use crate::adapter::outbound::sqlite_connection::SqliteConnection;

pub struct InitReport {
    pub db_initialized: bool,
    pub toml_created: bool,
    pub steering_copied: bool,
    pub gitignore_updated: bool,
}

pub struct InitUseCase {
    base_dir: PathBuf,
}

impl InitUseCase {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
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
        let db_path = cupola_dir.join("cupola.db");
        let db_existed = db_path.exists();
        let db = SqliteConnection::open(&db_path)?;
        db.init_schema()?;
        let db_initialized = !db_existed;
        tracing::info!(path = %db_path.display(), "SQLite schema initialized");

        // Step 3-5: ファイル生成
        let file_gen = InitFileGenerator::new(self.base_dir.clone());

        let toml_created = file_gen
            .generate_toml_template()
            .context("failed to generate cupola.toml template")?;

        let steering_copied = file_gen
            .copy_steering_templates()
            .context("failed to copy steering templates")?;

        let gitignore_updated = file_gen
            .append_gitignore_entries()
            .context("failed to append .gitignore entries")?;

        Ok(InitReport {
            db_initialized,
            toml_created,
            steering_copied,
            gitignore_updated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // === Task 3.2: InitUseCase の統合テスト ===

    #[test]
    fn init_use_case_creates_all_files_in_empty_dir() {
        let tmp = TempDir::new().expect("temp dir");
        let uc = InitUseCase::new(tmp.path().to_path_buf());
        let report = uc.run().expect("run");

        assert!(report.db_initialized, "db should be initialized");
        assert!(report.toml_created, "toml should be created");
        assert!(
            !report.steering_copied,
            "steering should be skipped (no template)"
        );
        assert!(report.gitignore_updated, "gitignore should be updated");

        // ファイルの存在確認
        assert!(tmp.path().join(".cupola").join("cupola.db").exists());
        assert!(tmp.path().join(".cupola").join("cupola.toml").exists());
        assert!(tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn init_use_case_skips_all_when_files_exist() {
        let tmp = TempDir::new().expect("temp dir");

        // 事前にファイルを作成
        let cupola_dir = tmp.path().join(".cupola");
        fs::create_dir_all(&cupola_dir).expect("create .cupola");
        fs::write(cupola_dir.join("cupola.toml"), "existing").expect("write toml");
        fs::write(tmp.path().join(".gitignore"), "# cupola\n").expect("write gitignore");

        let uc = InitUseCase::new(tmp.path().to_path_buf());
        let report = uc.run().expect("run");

        assert!(report.db_initialized, "db init is idempotent");
        assert!(!report.toml_created, "toml should be skipped");
        assert!(!report.steering_copied, "steering should be skipped");
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
        let uc = InitUseCase::new(tmp.path().to_path_buf());

        // 1回目の実行
        uc.run().expect("first run");

        let toml_content_after_first =
            fs::read_to_string(tmp.path().join(".cupola").join("cupola.toml")).expect("read toml");
        let gitignore_content_after_first =
            fs::read_to_string(tmp.path().join(".gitignore")).expect("read gitignore");

        // 2回目の実行
        let report = uc.run().expect("second run");

        assert!(
            !report.db_initialized,
            "db already existed, should be skipped on 2nd run"
        );
        assert!(!report.toml_created, "toml should be skipped on 2nd run");
        assert!(
            !report.steering_copied,
            "steering should be skipped on 2nd run"
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
    fn init_use_case_with_steering_template_dir() {
        let tmp = TempDir::new().expect("temp dir");

        // steering テンプレートディレクトリを作成
        let template_dir = tmp
            .path()
            .join(".cupola")
            .join("settings")
            .join("templates")
            .join("steering");
        fs::create_dir_all(&template_dir).expect("create template dir");
        fs::write(template_dir.join("product.md"), "# Product").expect("write template");

        let uc = InitUseCase::new(tmp.path().to_path_buf());
        let report = uc.run().expect("run");

        assert!(
            report.steering_copied,
            "steering should be copied when template exists"
        );

        let dest = tmp
            .path()
            .join(".cupola")
            .join("steering")
            .join("product.md");
        assert!(dest.exists(), "template file should be copied");
    }

    #[test]
    fn init_use_case_without_template_skips_steering_runs_rest() {
        let tmp = TempDir::new().expect("temp dir");
        // テンプレートディレクトリなし
        let uc = InitUseCase::new(tmp.path().to_path_buf());
        let report = uc.run().expect("run");

        assert!(report.db_initialized);
        assert!(report.toml_created);
        assert!(
            !report.steering_copied,
            "should skip steering without template"
        );
        assert!(report.gitignore_updated);
    }
}
