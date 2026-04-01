# 実装タスク

## タスク一覧

- [x] 1. キャッチコピーとヘッダーを更新する
- [x] 1.1 冒頭のキャッチコピーを新しい文言に変更する
  - タイトル `# Cupola` 直下の説明文を "Issue-driven local agent control plane for spec-driven development." に変更する
  - `[日本語](./README.ja.md)` リンクはそのまま維持する
  - キャッチコピーはリンク行の直後に配置する
  - _Requirements: 1.1, 1.2_

- [x] 2. CLI コマンドリファレンスを全面更新する
- [x] 2.1 `cupola start` コマンドのドキュメントを更新する
  - `cupola run` のセクションを削除し、`cupola start` に置換する
  - オプションテーブル（`--polling-interval-secs`, `--log-level`, `--config`, `-d`/`--daemon`）を記載する
  - コマンド使用例（通常起動・デーモン起動）を記載する
  - _Requirements: 2.1, 2.2_

- [x] 2.2 `cupola stop` コマンドのセクションを追加する
  - `cupola stop` サブコマンドのセクションを新規作成する
  - `--config` オプションを記載する
  - バックグラウンドデーモンを停止するコマンドであることを説明する
  - _Requirements: 2.3_

- [x] 2.3 `cupola doctor` コマンドのセクションを追加する
  - `cupola doctor` サブコマンドのセクションを新規作成する
  - `--config` オプションを記載する
  - 前提条件チェックを実行するコマンドであることを説明する
  - _Requirements: 2.4_

- [x] 2.4 `cupola --version` / `-V` フラグのドキュメントを追加する
  - `--version` と `-V` でバージョンを表示できることを記載する
  - 使用例を追記する
  - _Requirements: 2.5_

- [x] 2.5 `cupola init` と `cupola status` のドキュメントを確認・維持する
  - `cupola init`（SQLite スキーマ初期化）の記載が存在することを確認する
  - `cupola status`（Issue 処理状態一覧）の記載が存在することを確認する
  - `cupola run` が README のいかなる箇所にも残っていないことを確認する
  - _Requirements: 2.6, 2.7, 2.8_

- [x] 3. Configuration Reference を更新する
- [x] 3.1 `max_concurrent_sessions` と `model` を設定項目テーブルに追加する
  - 設定項目テーブルに `max_concurrent_sessions`（型: オプション整数, デフォルト: 無制限, 説明: 同時実行セッション数の上限）を追記する
  - 設定項目テーブルに `model`（型: String, デフォルト: `"sonnet"`, 説明: デフォルト Claude モデル）を追記する
  - 既存の全設定項目（`owner`, `repo`, `default_branch`, `language`, `polling_interval_secs`, `max_retries`, `stall_timeout_secs`, `[log] level`, `[log] dir`）が維持されていることを確認する
  - _Requirements: 3.1, 3.2, 3.4_

- [x] 3.2 設定ファイル例のコードブロックを更新する
  - `max_concurrent_sessions` と `model` をフル設定例の `toml` コードブロックに追加する
  - `max_concurrent_sessions` には「省略時は無制限」のコメントを付記する
  - _Requirements: 3.3_

- [x] 4. 機能紹介を追加する
- [x] 4.1 主要機能の説明を Project Overview に追加する
  - CI 失敗の自動検知と修正機能を説明する箇条書きを追加する
  - マージコンフリクトの自動検知と修正機能を説明する箇条書きを追加する
  - Issue ラベル（`model:opus` 等）によるモデル上書き機能を説明する箇条書きを追加する
  - `max_concurrent_sessions` による同時実行数制限機能を説明する箇条書きを追加する
  - `cupola doctor` による前提条件チェック機能を説明する箇条書きを追加する
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [x] 5. Limitations セクションを新設する
- [x] 5.1 既知の制限事項を記載する独立セクションを作成する
  - `## Limitations` セクションを新規作成する
  - review thread のみ対応（PR レベルのレビューコメントは未対応）の制限を記載する
  - 品質チェックコマンドを `AGENTS.md` または `CLAUDE.md` に定義する必要があることを記載する
  - 目次に `- [Limitations](#limitations)` を追加する
  - _Requirements: 5.1, 5.2, 5.3_

- [x] 6. Architecture Overview のファイルツリーを現状に同期する
- [x] 6.1 ファイルツリーを実際の `src/` 構造で全面置換する
  - `domain/` 配下に `check_result.rs` と `fixing_problem_kind.rs` を追加する
  - `application/` 配下に `doctor_use_case.rs`, `init_use_case.rs`, `stop_use_case.rs` を追加する
  - `application/port/` 配下に `command_runner.rs`, `config_loader.rs`, `pid_file.rs` を追加する
  - `adapter/outbound/` 配下に `init_file_generator.rs`, `pid_file_manager.rs`, `process_command_runner.rs` を追加する
  - `bootstrap/` 配下に `toml_config_loader.rs` を追加する
  - README 記載のファイルツリーと実ファイル一覧（`mod.rs` 除く）が一致することを確認する
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [x] 7. Installation セクションを更新する
- [x] 7.1 リポジトリ URL と手順内のコマンドを修正する
  - `git clone` の URL を `https://github.com/kyuki3rain/cupola.git` に変更する
  - `cd <repo>` を `cd cupola` に変更する
  - 手順内の `cupola run` を `cupola start` に変更する
  - _Requirements: 7.1, 7.2, 7.3_

- [x] 8. README.md の全体整合性を検証する
- [x] 8.1 全要件の受け入れ条件を目視確認する
  - キャッチコピーがタイトル直下に表示されることを確認する（要件 1）
  - 全 CLI コマンド（`start`, `stop`, `doctor`, `init`, `status`, `--version`）が記載され、`cupola run` が存在しないことを確認する（要件 2）
  - 設定テーブルと設定例に `max_concurrent_sessions` と `model` が含まれることを確認する（要件 3）
  - 機能紹介の5項目が記載されていることを確認する（要件 4）
  - `Limitations` セクションが目次と本文に存在することを確認する（要件 5）
  - ファイルツリーが実構造と一致することを確認する（要件 6）
  - インストール URL が `kyuki3rain/cupola` になっていることを確認する（要件 7）
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 6.1, 6.2, 6.3, 6.4, 6.5, 7.1, 7.2, 7.3_
