use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::application::port::db_initializer::DbInitializer;
use crate::application::port::file_generator::FileGenerator;

pub struct InitReport {
    pub db_initialized: bool,
    pub toml_created: bool,
    pub steering_copied: bool,
    pub gitignore_updated: bool,
}

pub struct InitUseCase<D: DbInitializer, F: FileGenerator> {
    base_dir: PathBuf,
    db_init: D,
    file_gen: F,
}

impl<D: DbInitializer, F: FileGenerator> InitUseCase<D, F> {
    pub fn new(base_dir: PathBuf, db_init: D, file_gen: F) -> Self {
        Self {
            base_dir,
            db_init,
            file_gen,
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
        let db_path = cupola_dir.join("cupola.db");
        let db_existed = db_path.exists();
        self.db_init
            .init_schema()
            .context("failed to initialize SQLite schema")?;
        let db_initialized = !db_existed;
        tracing::info!(path = %db_path.display(), "SQLite schema initialized");

        // Step 3-5: ファイル生成
        let toml_created = self
            .file_gen
            .generate_toml_template()
            .context("failed to generate cupola.toml template")?;

        let steering_copied = self
            .file_gen
            .copy_steering_templates()
            .context("failed to copy steering templates")?;

        let gitignore_updated = self
            .file_gen
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

    use crate::adapter::outbound::init_file_generator::InitFileGenerator;
    use crate::adapter::outbound::sqlite_connection::SqliteConnection;

    // === InitUseCase の統合テスト ===

    #[test]
    fn init_use_case_creates_all_files_in_empty_dir() {
        let tmp = TempDir::new().expect("temp dir");
        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        let uc = InitUseCase::new(tmp.path().to_path_buf(), db, file_gen);
        let report = uc.run().expect("run");

        assert!(report.db_initialized, "db should be initialized");
        assert!(report.toml_created, "toml should be created");
        assert!(
            !report.steering_copied,
            "steering should be skipped (no template)"
        );
        assert!(report.gitignore_updated, "gitignore should be updated");

        // ファイルの存在確認
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

        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        let uc = InitUseCase::new(tmp.path().to_path_buf(), db, file_gen);
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
        let cupola_dir = tmp.path().join(".cupola");
        fs::create_dir_all(&cupola_dir).expect("create .cupola");
        let db_path = cupola_dir.join("cupola.db");

        // 1回目の実行（ファイルベース SQLite を使用）
        let db1 = SqliteConnection::open(&db_path).expect("open db");
        let file_gen1 = InitFileGenerator::new(tmp.path().to_path_buf());
        let uc1 = InitUseCase::new(tmp.path().to_path_buf(), db1, file_gen1);
        uc1.run().expect("first run");

        let toml_content_after_first =
            fs::read_to_string(tmp.path().join(".cupola").join("cupola.toml")).expect("read toml");
        let gitignore_content_after_first =
            fs::read_to_string(tmp.path().join(".gitignore")).expect("read gitignore");

        // 2回目の実行（db ファイルが既に存在するため db_initialized=false）
        let db2 = SqliteConnection::open(&db_path).expect("open db");
        let file_gen2 = InitFileGenerator::new(tmp.path().to_path_buf());
        let uc2 = InitUseCase::new(tmp.path().to_path_buf(), db2, file_gen2);
        let report = uc2.run().expect("second run");

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

        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        let uc = InitUseCase::new(tmp.path().to_path_buf(), db, file_gen);
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
        let db = SqliteConnection::open_in_memory().expect("open db");
        let file_gen = InitFileGenerator::new(tmp.path().to_path_buf());
        // テンプレートディレクトリなし
        let uc = InitUseCase::new(tmp.path().to_path_buf(), db, file_gen);
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
