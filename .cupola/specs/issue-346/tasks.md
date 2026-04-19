# 実装タスクリスト: issue-346

## タスク概要

- [ ] 1. `--dangerously-skip-permissions` フラグの削除
- [ ] 2. Claude Settings ドメインモデルの追加
- [ ] 3. Permission テンプレートファイルの追加
- [ ] 4. TemplateManager の実装
- [ ] 5. InitFileGenerator への settings.json 生成・マージ機能追加
- [ ] 6. CLI `--template` オプションと InitUseCase の拡張
- [ ] 7. Permission Denied エラーハンドリングの改善
- [ ] 8. ドキュメント更新

---

- [ ] 1. `--dangerously-skip-permissions` フラグの削除

- [ ] 1.1 `ClaudeCodeProcess::build_command` からフラグを削除する
  - `src/adapter/outbound/claude_code_process.rs` の `build_command` メソッドから `.arg("--dangerously-skip-permissions")` の行を削除する
  - 既存のユニットテストを更新し、フラグが含まれていないことをアサーションで確認する
  - _Requirements: 1.1_

- [ ] 1.2 steering bootstrap の呼び出しからもフラグを削除する
  - `src/application/init_use_case.rs` 内の steering bootstrap 処理で Claude Code を呼び出す箇所から `--dangerously-skip-permissions` フラグを削除する
  - _Requirements: 1.2_

- [ ] 2. Claude Settings ドメインモデルの追加

- [ ] 2.1 (P) `ClaudeSettings` 値オブジェクトを domain 層に追加する
  - `src/domain/claude_settings.rs` を新規作成し、`ClaudeSettings` と `ClaudePermissions` 構造体を定義する
  - `serde::Serialize` / `serde::Deserialize` を derive し、`Default` も `ClaudePermissions` に derive する
  - `#[serde(default)]` を `allow`/`deny` フィールドに付与し、フィールド欠落時は空配列になるようにする
  - `src/domain/mod.rs` に `pub mod claude_settings;` を追加する
  - _Requirements: 2.4_

- [ ] 3. Permission テンプレートファイルの追加

- [ ] 3.1 (P) `base.json` テンプレートを作成する
  - `assets/claude-settings/base.json` を作成する
  - `permissions.allow` に Read, Write, Edit, Glob, Grep, Bash(git status), Bash(git diff*), Bash(git add*), Bash(git commit*), Bash(git log*), Bash(git show*) を含める
  - `permissions.deny` に Bash(rm -rf*), Bash(curl*), Bash(wget*), Bash(ssh*), Bash(git push*), Bash(gh *), WebFetch, WebSearch を含める
  - _Requirements: 2.1_

- [ ] 3.2 (P) スタック別テンプレートを作成する
  - `assets/claude-settings/rust.json` を作成し、Bash(cargo build*), Bash(cargo test*), Bash(cargo clippy*), Bash(cargo fmt*), Bash(cargo check*), Bash(rustup*) を allow に含める
  - `assets/claude-settings/typescript.json` を作成し、Bash(npm *), Bash(pnpm *), Bash(yarn *), Bash(npx *), Bash(tsc*), Bash(node *) 等を allow に含める
  - `assets/claude-settings/python.json` を作成し、Bash(python*), Bash(pip *), Bash(poetry *), Bash(pytest*), Bash(uv *) 等を allow に含める
  - `assets/claude-settings/go.json` を作成し、Bash(go test*), Bash(go build*), Bash(go fmt*), Bash(go vet*), Bash(go mod *), Bash(go run *) 等を allow に含める
  - _Requirements: 2.2_

- [ ] 4. TemplateManager の実装

- [ ] 4.1 テンプレートのコンパイル時埋め込みと TemplateManager 骨格を作成する
  - `src/application/template_manager.rs` を新規作成する
  - `const TEMPLATES: &[(&str, &str)]` に `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/claude-settings/<key>.json"))` で各テンプレートを埋め込む
  - `TemplateError` 列挙体 (`UnknownTemplate`, `ParseError`) を `thiserror` で定義する
  - `src/application/mod.rs` に `pub mod template_manager;` を追加する
  - _Requirements: 2.3, 3.4_

- [ ] 4.2 テンプレートのロード・マージロジックを実装する
  - `TemplateManager::build_settings(templates: &[&str]) -> Result<ClaudeSettings, TemplateError>` を実装する
  - `base` を常に先頭に適用し、`templates` に `base` が含まれる場合は重複を排除する
  - 各テンプレートを順番に `allow`/`deny` の union でオーバーレイする
  - `TemplateManager::list_available() -> &'static [&'static str]` を実装する (エラーメッセージ用)
  - ユニットテストを作成する: base のみ / 複数テンプレート / 未知キー / base 重複入力 / 空スライス
  - _Requirements: 2.1, 2.2, 3.1, 3.2, 3.3, 3.5_

- [ ] 5. InitFileGenerator への settings.json 生成・マージ機能追加

- [ ] 5.1 `FileGenerator` トレイトに `write_claude_settings` メソッドを追加する
  - `src/application/port/file_generator.rs` の `FileGenerator` トレイトに `write_claude_settings(&self, settings: &ClaudeSettings, upgrade: bool) -> Result<bool, anyhow::Error>` を追加する
  - `ClaudeSettings` 型を domain からインポートする
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ] 5.2 `InitFileGenerator` に settings.json 書き込みと deep merge を実装する
  - `src/adapter/outbound/init_file_generator.rs` に `write_claude_settings` を実装する
  - 対象ディレクトリの `.claude/` サブディレクトリが存在しない場合は `std::fs::create_dir_all` で作成する
  - 既存 `.claude/settings.json` が存在する場合は読み込んで `deep_merge_json` を適用する
  - `deep_merge_json(existing: Value, managed: Value) -> Value` を実装する: `permissions.allow`/`deny` は `HashSet` で union、スカラーキーは `existing` 優先、ネストオブジェクトは再帰マージ
  - マージ済み JSON を `serde_json::to_string_pretty` で書き込む
  - ユニットテストを作成する: 新規ファイル / 既存 allow への union / スカラー既存優先 / --upgrade でユーザー設定保持
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ] 6. CLI `--template` オプションと InitUseCase の拡張

- [ ] 6.1 CLI `Init` サブコマンドに `--template` オプションを追加する
  - `src/adapter/inbound/cli.rs` の `Init` バリアントに `#[arg(long, value_delimiter = ',')] template: Vec<String>` フィールドを追加する
  - `--template` 未指定時は空の `Vec<String>` となるようにする (`default_value` は設定しない)
  - コマンド実行箇所で `init_use_case.run(&cmd.template, cmd.upgrade)` のように渡す
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 6.2 `InitUseCase::run` に templates パラメータを追加して統合する
  - `InitUseCase::run` のシグネチャに `templates: &[String]` パラメータを追加する
  - `TemplateManager::build_settings` を呼び出し `ClaudeSettings` を生成し、未知テンプレートキーエラー時は早期リターンする
  - `FileGenerator::write_claude_settings(settings, upgrade)` を呼び出す
  - `InitReport` 構造体に `settings_json_written: bool` フィールドを追加する
  - `bootstrap` 層の `InitUseCase` 呼び出し箇所を新しいシグネチャに合わせて更新する
  - 統合テストを追加する: base のみ / --template rust / 既存 settings.json あり / --upgrade
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.3, 4.4_

- [ ] 7. Permission Denied エラーハンドリングの改善

- [ ] 7.1 Permission Denied レスポンスの検知とエラーログを実装する
  - Claude Code の JSON レスポンスにおける permission denied パターンを実際に確認し、既存の JSON パーサーに対応するケースを追加する
  - permission denied 検出時に `tracing::error!` でツール名と allow 追加ヒントを出力する
  - セッションを失敗状態に遷移させる既存のエラーハンドリングフローと接続する
  - _Requirements: 5.1, 5.2, 5.3_

- [ ] 8. ドキュメント更新

- [ ] 8.1 (P) `SECURITY.md` を更新する
  - Prompt Injection Risk セクションに `--dangerously-skip-permissions` 廃止と permission 機構採用を記載する
  - `cupola init --template <key>` による安全デフォルト設定の取得方法を記載する
  - `permissions.allow` を緩めることが攻撃面の拡大につながることを明記する
  - _Requirements: 6.1, 6.3_

- [ ] 8.2 (P) `CONTRIBUTING.md` を更新する
  - `assets/claude-settings/<key>.json` を追加することでテンプレートをコントリビュートできる手順を記載する
  - テンプレートキーの命名規則 (言語: rust/typescript/python/go、フレームワーク: nextjs/django/axum、インフラ: docker/terraform/kubernetes) を記載する
  - _Requirements: 6.2_
