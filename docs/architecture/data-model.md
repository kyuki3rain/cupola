# データモデル

## Issue

ポーリングループが管理する中心的なエンティティ。

| フィールド | 型 | 説明 |
|-----------|----|----|
| `id` | `i64` | DB primary key |
| `github_issue_number` | `u64` | GitHub Issue 番号 |
| `state` | `State` | 現在の状態 |
| `feature_name` | `String` | ブランチ名（デフォルト: `issue-{N}`） |
| `weight` | `TaskWeight` | タスク重み（ラベルから取得） |
| `worktree_path` | `Option<String>` | Git worktree のローカルパス |
| `initialized` | `bool` | 初期化完了フラグ |
| `design_pr_number` | `Option<u64>` | Design フェーズの PR 番号 |
| `impl_pr_number` | `Option<u64>` | Implementation フェーズの PR 番号 |
| `current_pid` | `Option<u32>` | 実行中プロセスの PID |
| `last_process_exit` | `Option<ProcessExit>` | Resolve が保存する直近プロセス終了結果（Collect が消費） |
| `retry_count` | `u32` | 現在フェーズのリトライ回数 |
| `ci_fix_count` | `u32` | CI/Conflict 修正サイクル数 |
| `fixing_causes` | `Vec<FixingProblemKind>` | 修正原因の一覧 |
| `error_message` | `Option<String>` | 直近のエラーメッセージ |
| `created_at` | `DateTime<Utc>` | 作成時刻 |
| `updated_at` | `DateTime<Utc>` | 更新時刻 |

### ブランチ構造

```
origin/{default_branch}
  └─ cupola/{issue_number}/main   ← Implementation フェーズのベース
       └─ cupola/{issue_number}/design  ← Design フェーズの作業ブランチ
```

## TaskWeight

GitHub ラベルから取得。

| ラベル | Weight |
|-------|--------|
| `weight:heavy` | `Heavy` |
| `weight:light` | `Light` |
| （なし） | `Medium`（デフォルト） |

複数ラベルが付いている場合は `Heavy > Light > Medium` の優先度。

## FixingProblemKind

`FixingRequired` イベントに付属し、Fixing フェーズでの修正内容を決定する。

| バリアント | 発生条件 |
|-----------|---------|
| `ReviewComments` | unresolved review thread あり |
| `CiFailure` | CI check-run が failure |
| `Conflict` | PR が mergeable でない |

複数同時に発生する場合がある。Fixing フェーズのプロセス起動時に input files の生成内容を決定する。

## ProcessExit

Resolve フェーズが stale guard 通過後に保存するプロセス終了結果。

| バリアント | 意味 |
|-----------|------|
| `Succeeded` | プロセス本体と Resolve 内後処理が成功 |
| `Failed` | 非 0 終了、stall kill、Init タスク失敗、または Resolve 内後処理失敗 |

Collect は `last_process_exit` を読んで `ProcessSucceeded` / `ProcessFailed` / `RetryExhausted` シグナルを生成し、その後 `None` に戻す。

## Model Selection Key

モデル選択は独立した `Phase` 概念ではなく、現在の `State` を設定ファイル上のキー文字列へ写像して行う。

| State | state-key |
|-------|-----------|
| `DesignRunning` | `design` |
| `DesignFixing` | `design_fix` |
| `ImplementationRunning` | `implementation` |
| `ImplementationFixing` | `implementation_fix` |

`InitializeRunning` / `*ReviewWaiting` / `Completed` / `Cancelled` は Claude Code プロセスを起動しないため、モデル選択キーを持たない。

## Config

`cupola.toml` から読み込む設定。

| フィールド | デフォルト | 説明 |
|-----------|----------|------|
| `owner` | 必須 | GitHub リポジトリオーナー |
| `repo` | 必須 | GitHub リポジトリ名 |
| `default_branch` | 必須 | デフォルトブランチ名 |
| `language` | 必須 | AI への出力言語 |
| `polling_interval_secs` | `60` | ポーリング間隔（秒） |
| `max_retries` | `3` | フェーズ内の最大リトライ数 |
| `max_ci_fix_cycles` | `3` | CI/Conflict 修正の上限回数 |
| `stall_timeout_secs` | `1800` | プロセス stall 検知タイムアウト（秒） |
| `max_concurrent_sessions` | `Some(3)` | 並行プロセス数の上限 |
| `models` | - | weight × state-key → モデル名のマッピング |
| `trusted_associations` | `[Owner, Member, Collaborator]` | ラベル付与を許可する author association |

## AuthorAssociation

GitHub の author association。`trusted_associations` との照合に使用。

```
Owner > Member > Collaborator > Contributor > FirstTimeContributor > FirstTimer > None
```

`TrustedAssociations::All` 設定時はチェックをスキップ（全ユーザー信頼）。
