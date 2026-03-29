# Cupola

GitHub Issue を起点に、設計から実装までを自動化するローカル常駐エージェント。

## 目次

- [プロジェクト概要](#プロジェクト概要)
- [前提条件](#前提条件)
- [インストール・セットアップ](#インストールセットアップ)
- [使い方](#使い方)
- [CLI コマンドリファレンス](#cli-コマンドリファレンス)
- [設定ファイルリファレンス](#設定ファイルリファレンス)
- [アーキテクチャ概要](#アーキテクチャ概要)
- [ライセンス](#ライセンス)

## プロジェクト概要

Cupola は、GitHub Issue / PR を唯一の操作面とし、Claude Code + cc-sdd を駆動して設計・実装を自動化するローカル常駐エージェントです。人間は Issue 作成・ラベル付与・PR レビューのみを行い、設計ドキュメント生成から実装、レビュー対応、完了 cleanup までを Cupola が自動化します。GitHub の既存ワークフロー（Issue + PR + review）をそのまま活用し、専用 UI なしで品質担保と自動化を両立します。

## 前提条件

| ツール | 用途 | 備考 |
|--------|------|------|
| Rust stable | ビルド | devbox 経由で管理 |
| Claude Code CLI | AI コード生成 | Anthropic 提供 |
| gh CLI | GitHub API 操作 | GitHub 公式 |
| Git | バージョン管理 | &#8212; |
| devbox | 開発環境管理 | Nix ベース |

**cc-sdd（spec-driven development）** は、要件定義・設計・タスク分解・実装を段階的に進める仕様駆動開発の手法です。Cupola は内部で cc-sdd を駆動し、Issue の内容から requirements / design / tasks を自動生成した上で実装を行います。

devbox を利用する場合、リポジトリルートで `devbox shell` を実行すると必要なツール（Rust 等）が一括でセットアップされます。

## インストール・セットアップ

1. リポジトリをクローンする

   ```bash
   git clone https://github.com/<owner>/<repo>.git
   cd <repo>
   ```

2. 開発環境に入る（devbox 利用時）

   ```bash
   devbox shell
   ```

   devbox を使わない場合は、Rust stable を手動でインストールしてください。

3. ビルド・インストールする

   ```bash
   cargo install --path .
   ```

   > `cargo install` により `cupola` バイナリが `~/.cargo/bin/` に配置されます。PATH に `~/.cargo/bin` が含まれていることを確認してください。

4. `.cupola/cupola.toml` を作成する

   ```toml
   owner = "your-github-username"
   repo = "your-repo-name"
   default_branch = "main"
   ```

   全設定項目の詳細は [設定ファイルリファレンス](#設定ファイルリファレンス) を参照してください。

5. SQLite スキーマを初期化する

   ```bash
   cupola init
   ```

6. GitHub リポジトリに `agent:ready` ラベルを作成する

   ```bash
   gh label create "agent:ready" --description "Cupola automation trigger"
   ```

7. polling を開始する

   ```bash
   cupola run
   ```

## 使い方

Issue 作成から merge までのワークフロー:

1. **[人間]** GitHub Issue を作成し、要求事項を記述する
2. **[人間]** Issue に `agent:ready` ラベルを付与する &#8212; これが Cupola のトリガーとなる
3. **[Cupola]** Issue を検知し、cc-sdd で設計ドキュメント（requirements / design / tasks）を自動生成する
4. **[Cupola]** 設計 PR を作成する
5. **[人間]** 設計 PR をレビューし、approve する
6. **[Cupola]** タスクに基づき実装を自動生成する
7. **[Cupola]** 実装 PR を作成する
8. **[人間]** 実装 PR をレビューし、approve → merge する
9. **[Cupola]** cleanup 処理（ラベル除去等）を実行する

設計 PR と実装 PR の 2 段階レビューフローにより、人間のレビュー承認を唯一のゲートとして品質を担保します。

## CLI コマンドリファレンス

### `cupola run`

polling ループを開始し、`agent:ready` ラベル付き Issue を監視します。

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--polling-interval-secs <秒>` | polling 間隔の上書き（秒） | `cupola.toml` の値 |
| `--log-level <レベル>` | ログレベルの上書き（trace / debug / info / warn / error） | `cupola.toml` の値 |
| `--config <パス>` | 設定ファイルパス | `.cupola/cupola.toml` |

```bash
# デフォルト設定で起動
cupola run

# polling 間隔とログレベルを指定して起動
cupola run --polling-interval-secs 30 --log-level debug
```

### `cupola init`

SQLite スキーマを初期化します。初回セットアップ時に一度実行してください。

```bash
cupola init
```

### `cupola status`

全 Issue の処理状態を一覧表示します。

```bash
cupola status
```

## 設定ファイルリファレンス

設定ファイルは `.cupola/cupola.toml` に配置します。

| 項目 | 型 | デフォルト値 | 説明 |
|------|----|-------------|------|
| `owner` | String | &#8212; (必須) | GitHub リポジトリオーナー |
| `repo` | String | &#8212; (必須) | GitHub リポジトリ名 |
| `default_branch` | String | &#8212; (必須) | デフォルトブランチ名 |
| `language` | String | `"ja"` | 生成ドキュメントの言語 |
| `polling_interval_secs` | u64 | `60` | polling 間隔（秒） |
| `max_retries` | u32 | `3` | 最大リトライ回数 |
| `stall_timeout_secs` | u64 | `1800` | stall 判定タイムアウト（秒） |
| `[log] level` | String | `"info"` | ログレベル |
| `[log] dir` | String | &#8212; (任意) | ログ出力ディレクトリ |

完全な設定例:

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

Cupola は Clean Architecture（4 レイヤー）を採用しています。依存方向は内向きのみです。

| レイヤー | ディレクトリ | 責務 |
|---------|-------------|------|
| domain | `src/domain/` | 純粋ビジネスロジック。State, Event, StateMachine, Issue, Config。I/O 依存なし |
| application | `src/application/` | ユースケースとポート（trait）定義。外部依存を trait で抽象化 |
| adapter | `src/adapter/` | 外部接続の実装。inbound（CLI）/ outbound（GitHub, SQLite, Claude Code, Git） |
| bootstrap | `src/bootstrap/` | DI 配線、設定読み込み、ランタイム起動 |

依存方向: `domain` ← `application` ← `adapter` ← `bootstrap`（内向きのみ）

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
