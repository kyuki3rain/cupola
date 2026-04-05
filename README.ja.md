# Cupola

[![CI](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml/badge.svg)](https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/kyuki3rain/cupola/blob/main/LICENSE)

[English](./README.md)

Issue 駆動のローカル常駐エージェント。仕様駆動開発のコントロールプレーン。

## 目次

- [プロジェクト概要](#プロジェクト概要)
- [前提条件](#前提条件)
- [インストールとセットアップ](#インストールとセットアップ)
- [使い方](#使い方)
- [CLI コマンドリファレンス](#cli-コマンドリファレンス)
- [設定リファレンス](#設定リファレンス)
- [アーキテクチャ概要](#アーキテクチャ概要)
- [制限事項](#制限事項)
- [ライセンス](#ライセンス)

## プロジェクト概要

Cupola は、GitHub Issue と PR を唯一のインターフェースとして、Claude Code を使って設計から実装までを自動化するローカル常駐エージェントです。人間が行うのは Issue の作成、ラベルの付与、PR のレビューだけで、Cupola が設計ドキュメントの生成から実装、レビュー対応、完了後のクリーンアップまでをすべて処理します。GitHub の既存ワークフロー（Issue + PR + レビュー）を活用することで、専用 UI を必要とせずに品質保証と自動化の両立を実現しています。

**主な機能:**

- **設計の自動生成**: GitHub Issue を検知し、同梱した Cupola skills で要件・設計・タスクを自動生成する
- **PR の自動作成**: 設計 PR・実装 PR を手動操作なしに自動作成する
- **レビュースレッド対応**: PR のレビュースレッドへの修正・返信・resolve を自動処理する
- **CI 失敗の自動修正**: CI（GitHub Actions 等）の失敗を検知し、自動的に修正を試みる
- **コンフリクトの自動修正**: マージコンフリクトを検知し、自動的に解消を試みる
- **Issue ラベルによるモデル上書き**: Issue に `model:opus` 等のラベルを付与することで、そのIssueに使用するClaudeモデルをオーバーライドできる
- **同時実行数制限**: `max_concurrent_sessions` によりエージェントの同時実行数を制限できる
- **前提条件チェック**: `cupola doctor` を実行することで、`cupola.toml` の構成や Git / gh CLI / ラベル設定 / steering / DB など、Cupola の動作に必要な環境を検証できる

## 前提条件

> **対応プラットフォーム**: Unix (macOS / Linux) のみ対応。`nix` クレート (`cfg(unix)`) への依存により、Windows は非対応です。

| Tool | 用途 | 備考 |
|------|------|------|
| Rust stable | ビルド | devbox で管理 |
| Claude Code CLI | AI コード生成 | Anthropic 提供 |
| gh CLI | GitHub API 操作 | GitHub 公式 |
| Git | バージョン管理 | — |
| devbox | 開発環境管理 | Nix ベース |

Cupola は `cupola init` により、独自の rules / templates / Claude Code skills を対象リポジトリへ導入します。設計と実装は外部依存の skill ではなく、同梱した `/cupola:*` コマンドで実行されます。

devbox を使用する場合は、リポジトリルートで `devbox shell` を実行すると、必要なツール（Rust 等）を一括でセットアップできます。

## インストールとセットアップ

1. リポジトリをクローンする

   ```bash
   git clone https://github.com/kyuki3rain/cupola.git
   cd cupola
   ```

2. 開発環境に入る（devbox を使用する場合）

   ```bash
   devbox shell
   ```

   devbox を使用しない場合は、Rust stable を手動でインストールしてください。

3. ビルドとインストール

   ```bash
   cargo install --path .
   ```

   > `cargo install` は `cupola` バイナリを `~/.cargo/bin/` に配置します。`~/.cargo/bin` が PATH に含まれていることを確認してください。

4. リポジトリを Cupola で bootstrap する

   ```bash
   cupola init
   ```

   `.cupola/cupola.toml` の生成、bundled assets の導入、`.gitignore` 更新、SQLite 初期化、Claude Code による初期 steering 生成を試みます。

5. `.cupola/cupola.toml` を編集する

6. GitHub リポジトリに `agent:ready` ラベルを作成する

   ```bash
   gh label create "agent:ready" --description "Cupola automation trigger"
   ```

7. ポーリングを開始する

   ```bash
   cupola start
   ```

## 使い方

Issue の作成からマージまでのワークフロー：

1. **[Human]** GitHub Issue を作成し、要件を記述する
2. **[Human]** Issue に `agent:ready` ラベルを付与する — これが Cupola のトリガーになる
3. **[Cupola]** Issue を検知し、同梱した Cupola skills で設計ドキュメント（要件 / 設計 / タスク）を自動生成する
4. **[Cupola]** 設計 PR を作成する
5. **[Human]** 設計 PR をレビューし、承認する
6. **[Cupola]** タスクに基づいて実装を自動生成する
7. **[Cupola]** 実装 PR を作成する
8. **[Human]** 実装 PR をレビューし、承認・マージする
9. **[Cupola]** クリーンアップ（ラベルの削除など）を実行する

設計 PR と実装 PR の 2 段階レビューフローにより、人間のレビュー承認を唯一のゲートとして品質を担保しています。

## CLI コマンドリファレンス

### `cupola start`

ポーリングループを開始し、`agent:ready` ラベルが付いた Issue を監視します。

| Option | 説明 | デフォルト |
|--------|------|------------|
| `--polling-interval-secs <seconds>` | ポーリング間隔を上書きする（秒） | `cupola.toml` の値 |
| `--log-level <level>` | ログレベルを上書きする（trace / debug / info / warn / error） | `cupola.toml` の値 |
| `--config <path>` | 設定ファイルのパス | `.cupola/cupola.toml` |
| `-d`, `--daemon` | バックグラウンド（デーモン）として起動する | false |

```bash
# デフォルト設定で起動する
cupola start

# デーモンとして起動し、ポーリング間隔を指定する
cupola start --daemon --polling-interval-secs 30
```

### `cupola stop`

バックグラウンドで動作中のデーモンを停止します。

| Option | 説明 | デフォルト |
|--------|------|------------|
| `--config <path>` | 設定ファイルのパス | `.cupola/cupola.toml` |

```bash
cupola stop
```

### `cupola doctor`

必要なツールと設定がすべて揃っているかを確認します。

| Option | 説明 | デフォルト |
|--------|------|------------|
| `--config <path>` | 設定ファイルのパス | `.cupola/cupola.toml` |

```bash
cupola doctor
```

### `cupola init`

現在のリポジトリを Cupola 用に bootstrap します。

```bash
cupola init
```

### `cupola status`

全 Issue の処理状況を一覧表示します。

```bash
cupola status
```

### `cupola --version` / `-V`

インストール済みバージョンを表示します。

```bash
cupola --version
```

## 設定リファレンス

設定ファイルは `.cupola/cupola.toml` に配置します。

| Key | Type | デフォルト | 説明 |
|-----|------|------------|------|
| `owner` | String | —（必須） | GitHub リポジトリのオーナー |
| `repo` | String | —（必須） | GitHub リポジトリ名 |
| `default_branch` | String | —（必須） | デフォルトブランチ名 |
| `language` | String | `"ja"` | 生成ドキュメントの言語 |
| `polling_interval_secs` | u64 | `60` | ポーリング間隔（秒） |
| `max_retries` | u32 | `3` | 最大リトライ回数 |
| `stall_timeout_secs` | u64 | `1800` | 停滞検知タイムアウト（秒） |
| `max_concurrent_sessions` | u32 | `3` | 同時実行する Cupola セッション数の上限 |
| `model` | String | `"sonnet"` | エージェントセッションで使用するデフォルト Claude モデル |
| `[log] level` | String | `"info"` | ログレベル |
| `[log] dir` | String | —（任意） | ログ出力ディレクトリ |

設定ファイルの全体例：

```toml
owner = "your-github-username"
repo = "your-repo-name"
default_branch = "main"
language = "ja"
polling_interval_secs = 60
max_retries = 3
stall_timeout_secs = 1800
max_concurrent_sessions = 4  # デフォルト: 3
model = "sonnet"

[log]
level = "info"
dir = ".cupola/logs"
```

## アーキテクチャ概要

Cupola はクリーンアーキテクチャ（4 層）を採用しています。依存方向は内側に向かう一方向のみです。

| Layer | ディレクトリ | 責務 |
|-------|-------------|------|
| domain | `src/domain/` | 純粋なビジネスロジック。State、Event、StateMachine、Issue、Config。I/O 依存なし |
| application | `src/application/` | ユースケースとポート（トレイト）定義。外部依存はトレイトで抽象化 |
| adapter | `src/adapter/` | 外部接続の実装。inbound（CLI） / outbound（GitHub、SQLite、Claude Code、Git） |
| bootstrap | `src/bootstrap/` | DI ワイヤリング、設定読み込み、ランタイム起動 |

依存方向: `domain` ← `application` ← `adapter` ← `bootstrap`（内側方向のみ）

```
src/
├── main.rs
├── lib.rs
├── domain/
│   ├── check_result.rs
│   ├── config.rs
│   ├── event.rs
│   ├── execution_log.rs
│   ├── fixing_problem_kind.rs
│   ├── issue.rs
│   ├── state.rs
│   └── state_machine.rs
├── application/
│   ├── doctor_use_case.rs
│   ├── error.rs
│   ├── init_use_case.rs
│   ├── io.rs
│   ├── polling_use_case.rs
│   ├── prompt.rs
│   ├── retry_policy.rs
│   ├── session_manager.rs
│   ├── stop_use_case.rs
│   ├── transition_use_case.rs
│   └── port/
│       ├── claude_code_runner.rs
│       ├── command_runner.rs
│       ├── config_loader.rs
│       ├── execution_log_repository.rs
│       ├── git_worktree.rs
│       ├── github_client.rs
│       ├── issue_repository.rs
│       └── pid_file.rs
├── adapter/
│   ├── inbound/
│   │   └── cli.rs
│   └── outbound/
│       ├── claude_code_process.rs
│       ├── git_worktree_manager.rs
│       ├── github_client_impl.rs
│       ├── github_graphql_client.rs
│       ├── github_rest_client.rs
│       ├── init_file_generator.rs
│       ├── pid_file_manager.rs
│       ├── process_command_runner.rs
│       ├── sqlite_connection.rs
│       ├── sqlite_execution_log_repository.rs
│       └── sqlite_issue_repository.rs
└── bootstrap/
    ├── app.rs
    ├── config_loader.rs
    ├── logging.rs
    └── toml_config_loader.rs
```

設計ドキュメントの役割分担は
[`docs/architecture/README.md`](docs/architecture/README.md)
に定義しています。
システム全体の設計前提は
[`docs/architecture/system-overview.md`](docs/architecture/system-overview.md)
にまとめています。
ワークフローの中核は
[`docs/architecture/polling.md`](docs/architecture/polling.md),
[`docs/architecture/signals.md`](docs/architecture/signals.md),
[`docs/architecture/states.md`](docs/architecture/states.md),
[`docs/architecture/effects.md`](docs/architecture/effects.md),
[`docs/architecture/reduce.md`](docs/architecture/reduce.md)
に分割して定義しています。

## 制限事項

- **レビューコメントのサポート範囲**: PR のレビュースレッド（`review_thread`）のみ対応。PR レベルのレビューコメント（スレッドを持たないトップレベルの PR レビューコメント）は未対応。
- **品質チェックコマンドの要件**: Cupola が実行する品質チェックコマンドは、対象リポジトリの `AGENTS.md` または `CLAUDE.md` に記載する必要があります。

## ライセンス

このプロジェクトは Apache License 2.0 の下でライセンスされています。詳細は [LICENSE](LICENSE) ファイルを参照してください。
