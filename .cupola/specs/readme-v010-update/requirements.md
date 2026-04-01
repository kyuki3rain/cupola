# 要件定義書

## プロジェクト概要（入力）
README.md の全面更新（v0.1.0 リリース向け）。キャッチコピー変更、CLIコマンド更新（start/stop/doctor/--version追加）、設定項目追加（max_concurrent_sessions/model）、機能紹介追加、制限事項セクション追加、ファイルツリー更新を行う。

## はじめに

本仕様は、Cupola v0.1.0 のリリースに向けた README.md の全面更新を対象とする。現行の README は初期実装時の内容を反映しており、その後追加された機能（daemon起動、stop コマンド、doctor コマンド等）や変更されたコマンド名（run → start）が反映されていない。OSS として公開するにあたり、正確かつ魅力的なドキュメントへの更新が必要である。

対象ファイル: `README.md`（英語版）

---

## 要件

### 要件 1: キャッチコピーの更新

**目的:** OSS ユーザーとして、プロジェクトの目的を一目で理解したい。そのため、冒頭のキャッチコピーが製品の本質を正確に表現している必要がある。

#### 受け入れ条件

1. The README shall display "Issue-driven local agent control plane for spec-driven development." as the tagline immediately following the Japanese version link line (i.e., after the `[日本語](./README.ja.md)` link that appears directly below the project title).
2. When a user reads the top of README.md, the README shall present the Japanese version link immediately after the project title, followed by the tagline before any other description.

---

### 要件 2: CLI コマンドリファレンスの更新

**目的:** ユーザーとして、現在利用可能なすべての CLI コマンドを README から確認したい。そのため、廃止されたコマンド（`cupola run`）が削除され、新コマンドが網羅されている必要がある。

#### 受け入れ条件

1. The README shall document `cupola start` as the command to start the polling loop, replacing the removed `cupola run` command.
2. The README shall document `cupola start -d` (or `--daemon`) as the option to start the agent as a background daemon process.
3. The README shall document `cupola stop` as the command to stop the background daemon.
4. The README shall document `cupola doctor` as the command to run environment prerequisite checks.
5. The README shall document `cupola --version` and `cupola -V` as the flags to display the installed version.
6. The README shall document `cupola init` as the command to initialize the SQLite schema.
7. The README shall document `cupola status` as the command to list the processing status of all Issues.
8. If `cupola run` appears anywhere in README.md, the README shall not include it (the command has been renamed to `start`).

---

### 要件 3: 設定項目リファレンスの更新

**目的:** ユーザーとして、すべての設定項目とそのデフォルト値を Configuration Reference から確認したい。

#### 受け入れ条件

1. The README shall document `max_concurrent_sessions` configuration key with its type, default value (unlimited / no limit), and description (maximum number of concurrent agent sessions).
2. The README shall document `model` configuration key with its type, default value (`"sonnet"`), and description (default Claude model used for agent sessions).
3. The README shall include `max_concurrent_sessions` and `model` in the full configuration example block.
4. The README shall retain all existing configuration keys (`owner`, `repo`, `default_branch`, `language`, `polling_interval_secs`, `max_retries`, `stall_timeout_secs`, `[log] level`, `[log] dir`) in the Configuration Reference table.

---

### 要件 4: 機能紹介セクションの追加・更新

**目的:** 潜在的なユーザーとして、Cupola が提供する主要機能の概要を把握したい。そのため、機能紹介として自動化シナリオが明示されている必要がある。

#### 受け入れ条件

1. The README shall describe the CI failure auto-detection and fixing capability (Cupola detects CI failures and automatically attempts to fix them).
2. The README shall describe the merge conflict auto-detection and fixing capability.
3. The README shall describe the Issue label-based model override feature (e.g., `model:opus` label overrides the default model for a specific Issue).
4. The README shall describe the concurrent session limit feature controlled by `max_concurrent_sessions`.
5. The README shall describe the `cupola doctor` prerequisite check feature and what it verifies.

---

### 要件 5: 制限事項セクションの追加

**目的:** ユーザーとして、Cupola の既知の制限事項を事前に把握したい。そのため、Limitations または Known Limitations セクションが存在する必要がある。

#### 受け入れ条件

1. The README shall include a "Limitations" (or equivalent) section listing known constraints.
2. The README shall state that only review thread comments are supported; PR-level review comments (top-level PR review without thread) are not handled.
3. The README shall state that quality check commands must be defined in the repository's `AGENTS.md` or `CLAUDE.md` file for Cupola to execute them.

---

### 要件 6: ファイルツリーの更新

**目的:** 開発者として、現在のソースコード構造を README から把握したい。そのため、ファイルツリーが実際の `src/` ディレクトリ構造と一致している必要がある。

#### 受け入れ条件

1. The README shall reflect the current `src/` directory structure, including files added after the initial implementation such as `doctor_use_case.rs`, `init_use_case.rs`, `stop_use_case.rs`, `fixing_problem_kind.rs`, `check_result.rs`, and `pid_file_manager.rs`.
2. The README shall include `application/port/command_runner.rs`, `application/port/config_loader.rs`, and `application/port/pid_file.rs` in the file tree.
3. The README shall include `adapter/outbound/init_file_generator.rs`, `adapter/outbound/pid_file_manager.rs`, and `adapter/outbound/process_command_runner.rs` in the file tree.
4. The README shall include `bootstrap/toml_config_loader.rs` in the file tree.
5. If the file tree contains files that no longer exist in `src/` (e.g., previous names or removed files), the README shall not include them.

---

### 要件 7: インストール手順の更新

**目的:** ユーザーとして、実際の GitHub リポジトリ URL でクローン手順を確認したい。

#### 受け入れ条件

1. The README shall replace `<owner>/<repo>` placeholders in the Installation section with the actual repository path `kyuki3rain/cupola`.
2. The README shall retain the link to the Japanese version (`README.ja.md`) at the top of the document.
3. When a user follows the Installation steps, the README shall provide the correct `git clone` URL using `kyuki3rain/cupola`.
