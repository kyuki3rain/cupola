use anyhow::Result;
use std::path::Path;

/// ファイル生成操作を抽象化する outbound ポート。
/// 各メソッドは `true` を返す（操作を実行した）または `false` を返す（既存のためスキップ）。
pub trait FileGenerator: Send + Sync {
    /// cupola.toml テンプレートを生成する（冪等）。
    fn generate_toml_template(&self) -> Result<bool>;
    /// Claude Code 向けの Cupola assets を導入する（冪等）。
    fn install_claude_code_assets(&self) -> Result<bool>;
    /// .gitignore に cupola エントリを追記する（冪等）。
    fn append_gitignore_entries(&self) -> Result<bool>;
    /// spec ディレクトリ（spec.json + requirements.md）を生成する（冪等）。
    fn generate_spec_directory(
        &self,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool>;

    /// spec ディレクトリを指定ベース配下に生成する。
    fn generate_spec_directory_at(
        &self,
        base_dir: &Path,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool> {
        let _ = base_dir;
        self.generate_spec_directory(issue_number, issue_body, language)
    }
}
