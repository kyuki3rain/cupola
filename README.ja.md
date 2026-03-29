# Cupola

[English](./README.md)

GitHub Issue を起点に、設計から実装までを自動化するローカル常駐エージェントです。

## 目次

- [プロジェクト概要](#プロジェクト概要)
- [前提条件](#前提条件)
- [インストールとセットアップ](#インストールとセットアップ)
- [使い方](#使い方)
- [CLI コマンドリファレンス](#cli-コマンドリファレンス)
- [設定リファレンス](#設定リファレンス)
- [アーキテクチャ概要](#アーキテクチャ概要)
- [ライセンス](#ライセンス)

## プロジェクト概要

Cupola は、GitHub Issue と PR を唯一のインターフェースとして、Claude Code と cc-sdd を活用し設計から実装までを自動化するローカル常駐エージェントです。人間が行うのは Issue の作成、ラベルの付与、PR のレビューだけで、Cupola が設計ドキュメントの生成から実装、レビュー対応、完了後のクリーンアップまでをすべて処理します。GitHub の既存ワークフロー（Issue + PR + レビュー）を活用することで、専用 UI を必要とせずに品質保証と自動化の両立を実現しています。

## 前提条件

| Tool | 用途 | 備考 |
|------|------|------|
| Rust stable | ビルド | devbox で管理 |
| Claude Code CLI | AI コード生成 | Anthropic 提供 |
| gh CLI | GitHub API 操作 | GitHub 公式 |
| Git | バージョン管理 | — |
| devbox | 開発環境管理 | Nix ベース |

**cc-sdd（仕様駆動開発）** は、要件定義・設計・タスク分解・実装を段階的に進める仕様駆動の開発手法です。Cupola は内部で cc-sdd を駆動し、Issue の内容から要件・設計・タスクを自動生成したうえで実装に進みます。

devbox を使用する場合は、リポジトリルートで `devbox shell` を実行すると、必要なツール（Rust 等）を一括でセットアップできます。

## インストールとセットアップ

1. リポジトリをクローンする

   ```bash
   git clone https://github.com/<owner>/<repo>.git
   cd <repo>
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

4. `.cupola/cupola.toml` を作成する

   ```toml
   owner = "your-github-username"
   repo = "your-repo-name"
   default_branch = "main"
   ```

   すべての設定項目の詳細は[設定リファレンス](#設定リファレンス)を参照してください。

5. SQLite スキーマを初期化する

   ```bash
   cupola init
   ```

6. GitHub リポジトリに `agent:ready` ラベルを作成する

   ```bash
   gh label create "agent:ready" --description "Cupola automation trigger"
   ```

7. ポーリングを開始する

   ```bash
   cupola run
   ```

## 使い方

Issue の作成からマージまでのワークフロー：

1. **[Human]** GitHub Issue を作成し、要件を記述する
2. **[Human]** Issue に `agent:ready` ラベルを付与する — これが Cupola のトリガーになる
3. **[Cupola]** Issue を検知し、cc-sdd を使って設計ドキュメント（要件 / 設計 / タスク）を自動生成する
4. **[Cupola]** 設計 PR を作成する
5. **[Human]** 設計 PR をレビューし、承認する
6. **[Cupola]** タスクに基づいて実装を自動生成する
7. **[Cupola]** 実装 PR を作成する
8. **[Human]** 実装 PR をレビューし、承認・マージする
9. **[Cupola]** クリーンアップ（ラベルの削除など）を実行する

設計 PR と実装 PR の 2 段階レビューフローにより、人間のレビュー承認を唯一のゲートとして品質を担保しています。

## CLI コマンドリファレンス

### `cupola run`

ポーリングループを開始し、`agent:ready` ラベルが付いた Issue を監視します。

| Option | 説明 | デフォルト |
|--------|------|------------|
| `--polling-interval-secs <seconds>` | ポーリング間隔を上書きする（秒） | `cupola.toml` の値 |
| `--log-level <level>` | ログレベルを上書きする（trace / debug / info / warn / error） | `cupola.toml` の値 |
| `--config <path>` | 設定ファイルのパス | `.cupola/cupola.toml` |

```bash
# デフォルト設定で起動する
cupola run

# ポーリング間隔とログレベルを指定して起動する
cupola run --polling-interval-secs 30 --log-level debug
```

### `cupola init`

SQLite スキーマを初期化します。初回セットアップ時に一度だけ実行してください。

```bash
cupola init
```

### `cupola status`

全 Issue の処理状況を一覧表示します。

```bash
cupola status
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
│   ├── config.rs
│   ├── event.rs
│   ├── execution_log.rs
│   ├── issue.rs
│   ├── state.rs
│   └── state_machine.rs
├── application/
│   ├── error.rs
│   ├── io.rs
│   ├── polling_use_case.rs
│   ├── prompt.rs
│   ├── retry_policy.rs
│   ├── session_manager.rs
│   ├── transition_use_case.rs
│   └── port/
│       ├── claude_code_runner.rs
│       ├── execution_log_repository.rs
│       ├── git_worktree.rs
│       ├── github_client.rs
│       └── issue_repository.rs
├── adapter/
│   ├── inbound/
│   │   └── cli.rs
│   └── outbound/
│       ├── claude_code_process.rs
│       ├── git_worktree_manager.rs
│       ├── github_client_impl.rs
│       ├── github_graphql_client.rs
│       ├── github_rest_client.rs
│       ├── sqlite_connection.rs
│       ├── sqlite_execution_log_repository.rs
│       └── sqlite_issue_repository.rs
└── bootstrap/
    ├── app.rs
    ├── config_loader.rs
    └── logging.rs
```

## ライセンス

> ライセンスは未定です。LICENSE ファイルが作成され次第、ここにリンクを追加します。
