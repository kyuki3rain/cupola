use anyhow::Result;
use std::path::Path;

use crate::domain::claude_settings::ClaudeSettings;

/// ファイル生成操作を抽象化する outbound ポート。
/// 各メソッドは `true` を返す（操作を実行した）または `false` を返す（既存のためスキップ）。
pub trait FileGenerator: Send + Sync {
    /// cupola.toml テンプレートを生成する（冪等）。
    fn generate_toml_template(&self) -> Result<bool>;
    /// Claude Code 向けの Cupola assets を導入する（冪等）。
    /// `upgrade=true` の場合、既存の Cupola 管理ファイルを最新版で上書きする。
    fn install_claude_code_assets(&self, upgrade: bool) -> Result<bool>;
    /// .gitignore に cupola エントリを追記する（冪等）。
    /// `upgrade=true` の場合、既存の cupola セクションを最新版で置き換える。
    fn append_gitignore_entries(&self, upgrade: bool) -> Result<bool>;
    /// spec ディレクトリ（spec.json + requirements.md）を生成する（冪等）。
    fn generate_spec_directory(
        &self,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool>;

    /// spec ディレクトリを指定ベース配下に生成する（冪等）。
    /// 既定の base_dir に依存する `generate_spec_directory` と異なり、
    /// 呼び出し側が任意のディレクトリ（典型的には worktree ルート）を指定できる。
    fn generate_spec_directory_at(
        &self,
        base_dir: &Path,
        issue_number: u64,
        issue_body: &str,
        language: &str,
    ) -> Result<bool>;

    /// `.claude/settings.json` に `ClaudeSettings` を書き込む（既存ファイルとディープマージ）。
    ///
    /// - `Ok(true)`: ファイルを新規作成または更新した
    /// - `Ok(false)`: 変更なし
    /// - `Err(...)`: 既存ファイルの JSON パース失敗等
    fn write_claude_settings(&self, settings: &ClaudeSettings) -> Result<bool>;
}
