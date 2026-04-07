# データモデル

## Issue

ポーリングループが管理する中心的なエンティティ。

| フィールド | 型 | 説明 |
|-----------|----|----|
| `id` | `i64` | DB primary key |
| `github_issue_number` | `u64` | GitHub Issue 番号 |
| `state` | `State` | 現在の状態 |
| `feature_name` | `String` | スペック名（デフォルト: `issue-{N}`）。Claude Code へのインプットとして使用 |
| `weight` | `TaskWeight` | タスク重み（ラベルから取得） |
| `worktree_path` | `Option<String>` | Git worktree のローカルパス |
| `ci_fix_count` | `u32` | CI/Conflict 修正要求回数。`max_ci_fix_cycles` までは Fixing への遷移要求をカウント。上限到達時にさらに +1 して `max+1` で固定（以降インクリメントしない） |
| `close_finished` | `bool` | CloseIssue が成功済みかどうか（初期値 `false`） |
| `created_at` | `DateTime<Utc>` | 作成時刻 |
| `updated_at` | `DateTime<Utc>` | 更新時刻 |

### ブランチ構造

```
origin/{default_branch}
  └─ cupola/{feature_name}/main   ← Implementation フェーズのベース
       └─ cupola/{feature_name}/design  ← Design フェーズの作業ブランチ
```

## ProcessRun

プロセス実行の記録。Execute フェーズがスポーン時に作成し、Resolve フェーズが終了時に更新する。

| フィールド | 型 | 説明 |
|-----------|----|----|
| `id` | `i64` | DB primary key |
| `issue_id` | `i64` | Issue.id への外部キー |
| `type` | `ProcessRunType` | プロセス種別 |
| `index` | `u32` | 同 type 内の通し番号（0始まり） |
| `state` | `ProcessRunState` | 実行状態 |
| `pid` | `Option<u32>` | 実行中プロセスの OS PID |
| `pr_number` | `Option<u64>` | 作成した PR 番号（design / impl のみ） |
| `causes` | `Option<Vec<FixingProblemKind>>` | 修正原因（design_fix / impl_fix のみ） |
| `error_message` | `Option<String>` | エラーメッセージ |
| `started_at` | `DateTime<Utc>` | 開始時刻 |
| `finished_at` | `Option<DateTime<Utc>>` | 終了時刻 |

### ProcessRunType

| バリアント | 説明 |
|-----------|------|
| `init` | worktree・branch 作成 |
| `design` | Claude Code が Design PR を作成 |
| `design_fix` | Design PR の修正 |
| `impl` | Claude Code が Impl PR を作成 |
| `impl_fix` | Impl PR の修正 |

### ProcessRunState

| バリアント | 意味 |
|-----------|------|
| `running` | 実行中 |
| `succeeded` | 正常終了（後処理含む） |
| `failed` | 失敗（非0終了・タイムアウト・後処理失敗） |
| `stale` | 終了検知時に Issue 状態が既に進んでいた（後処理スキップ）。consecutive_failures にカウントしない |

### ProcessRun の操作者

- **Execute**: スポーン直前に state=running で INSERT
- **Resolve**: プロセス終了時に state を succeeded/failed/stale に UPDATE、pid・pr_number・error_message・finished_at を更新

## TaskWeight

GitHub ラベルから取得。

| ラベル | Weight |
|-------|--------|
| `weight:heavy` | `Heavy` |
| `weight:medium` | `Medium`（ラベルあり） |
| `weight:light` | `Light` |
| （なし） | `Medium`（デフォルト） |

`weight:medium` ラベルはラベルなしと同等の扱い。複数ラベルが付いている場合は `Heavy > Light > Medium` の優先度。

## FixingProblemKind

ProcessRun.causes に格納され、Fixing フェーズでの修正内容を決定する。

| バリアント | 発生条件 |
|-----------|---------|
| `ReviewComments` | unresolved review thread あり |
| `CiFailure` | CI check-run が failure または timed_out |
| `Conflict` | PR が mergeable でない |

複数同時に発生する場合がある。

## Model Selection Key

モデル選択は現在の `State` を設定ファイル上のキー文字列へ写像して行う。

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
| `max_retries` | `3` | 連続失敗の許容数。`consecutive_failures >= max_retries` で Cancelled に遷移。`max_retries=3` は「3 回連続失敗で打ち切り」を意味し、「初回 + 3 回リトライ（計 4 回）」ではない |
| `max_ci_fix_cycles` | `3` | CI/Conflict 修正の上限回数 |
| `stall_timeout_secs` | `1800` | プロセス stall 検知タイムアウト（秒） |
| `max_concurrent_sessions` | `Some(3)` | Claude Code プロセス（SessionManager 管理）の並行数上限。SpawnInit（InitTaskManager/JoinHandle 管理）は対象外のため、agent:ready が大量に付くと init タスクは上限なしに並行起動する |
| `models` | - | weight × state-key → モデル名のマッピング |
| `trusted_associations` | `[Owner, Member, Collaborator]` | ① `agent:ready` ラベル付与者の信頼判定、② PR レビュースレッドの信頼判定（非 trusted のスレッドは `has_review_comments` に反映されない）の両方に使用する |

## AuthorAssociation

GitHub の author association。`trusted_associations` との照合に使用。

```
Owner > Member > Collaborator > Contributor > FirstTimeContributor > FirstTimer > None
```

`TrustedAssociations::All` 設定時はチェックをスキップ（全ユーザー信頼）。
