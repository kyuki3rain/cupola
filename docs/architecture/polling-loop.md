# ポーリングループ設計

## 概要

ポーリングループは `polling_interval_secs`（デフォルト60秒）ごとに1サイクル実行される。1サイクルは4ステップで構成される。

```
┌─────────────────────────────────────────────┐
│  Resolve（非同期処理の完了回収・後処理）       │
│    プロセス終了検知 → 副作用実行              │
│    → メタデータを DB にコミット               │
├─────────────────────────────────────────────┤
│  Collect（純粋な観測）                        │
│    GitHub API・DB から観測                    │
│    → Issue ごとの全シグナルを生成             │
├─────────────────────────────────────────────┤
│  Apply（状態遷移）                            │
│    全シグナルセットを解決                     │
│    → state・metadata 更新を決定               │
│    → effect キューを構築                      │
│    → state・metadata を DB にコミット         │
├─────────────────────────────────────────────┤
│  Execute（副作用実行）                        │
│    プロセススポーン                           │
│    tokio::spawn でバックグラウンド処理        │
└─────────────────────────────────────────────┘
```

## Resolve

非同期処理の完了を回収し、後処理とメタデータ更新を行う。stale guard を通過した結果のみ DB に保存し、シグナル化は Collect フェーズが行う。

### Claude Code プロセスの終了処理

SessionManager から終了済みセッションを回収し、状態に応じて後処理を実行する。

| 終了状態 | 後処理 | メタデータ更新 | 生成シグナル |
|---------|--------|-------------|------------|
| Running 系 + 成功、後処理も成功 | stdout パース → PR 作成（GitHub API） | `design_pr_number` / `impl_pr_number` セット、`current_pid` クリア、`last_process_exit = Succeeded` | なし |
| Running 系 + 成功、後処理失敗 | stdout パース → PR 作成失敗 | `retry_count` インクリメント、`error_message` セット、`current_pid` クリア、`last_process_exit = Failed` | なし |
| Fixing 系 + 成功、後処理も成功 | stdout パース → スレッド返信・resolve（GitHub API） | `current_pid` クリア、`last_process_exit = Succeeded` | なし |
| Fixing 系 + 成功、後処理失敗 | stdout パース → スレッド返信・resolve 失敗 | `retry_count` インクリメント、`error_message` セット、`current_pid` クリア、`last_process_exit = Failed` | なし |
| 失敗（非 0 終了） | なし | `retry_count` インクリメント、`error_message` セット、`current_pid` クリア、`last_process_exit = Failed` | なし |

PR タイトル・本文は Claude Code の `structured_output` から取得し、パース失敗時はフォールバック文字列を使用する。

### Init タスクの完了処理

`InitTaskManager`（`HashMap<issue_id, JoinHandle<Result<()>>>`）を issue ループ内で確認する。

```rust
if let Some(handle) = self.init_handles.get(&issue.id) {
    if handle.is_finished() {
        let handle = self.init_handles.remove(&issue.id).unwrap();
        match handle.await {
            Ok(Ok(_)) => {
                // worktree_path と initialized = true を DB に書く
            }
            _ => {
                // retry_count インクリメント、error_message セット
                // last_process_exit = Failed を DB に書く
            }
        }
    }
}
```

### Stall 検知

`stall_timeout_secs`（デフォルト30分）を超過したプロセスをこのステップで SIGKILL する。kill されたプロセスは同サイクルの Resolve で `ProcessFailed` として回収される。

### 更新メタデータ一覧

| メタデータ | 更新条件 |
|-----------|---------|
| `last_process_exit = Succeeded` | Claude Code / Fixing プロセスが成功終了し、Resolve 内後処理も成功 |
| `last_process_exit = Failed` | 非 0 終了、タイムアウト強制終了、Init タスク失敗、または Resolve 内後処理失敗 |
| `initialized = true` | Init タスクの JoinHandle が `Ok(Ok(_))` |

## Collect

副作用なし。GitHub API・DB から観測を行い、観測系シグナルを生成する。

### 観測とシグナルの対応

| 観測元 | 生成されうるシグナル |
|-------|-------------------|
| GitHub Issues API | `ReadyIssueObserved`、`UntrustedReadyIssueObserved`、`IssueOpenObserved`、`IssueClosedObserved`、`WeightHeavyObserved`、`WeightLightObserved`、`WeightMediumObserved` |
| GitHub Pull Requests API | `DesignPrOpen`、`DesignPrMerged`、`ImplPrOpen`、`ImplPrMerged` |
| GitHub Review Threads API | `DesignReviewCommentFound`、`ImplReviewCommentFound` |
| GitHub Check Runs API | `DesignCiFailureFound`、`ImplCiFailureFound`、`CiFixExhausted` |
| GitHub PR `mergeable` | `DesignConflictFound`、`ImplConflictFound` |

Resolve で `design_pr_number` / `impl_pr_number` が確定している場合、同サイクル内で `DesignPrOpen` / `ImplPrOpen` の観測が可能。

また、Collect は DB の `last_process_exit` と `initialized` を観測し、`ProcessSucceeded` / `ProcessFailed` / `RetryExhausted` / `InitializationSucceeded` を生成する。signal 化後、`last_process_exit` は `None` に戻す。

## Apply

各 Issue のシグナルセット（Resolve 由来の bridge metadata を Collect が signal 化したもの + Collect 観測シグナル）を解決し、workflow state・metadata 更新・effect を決定する。

```
for each issue:
  1. シグナルセットと遷移テーブルを照合（優先度順）
  2. workflow state の遷移有無を決定
  3. weight / retry_count / ci_fix_count / fixing_causes などの metadata 更新を決定
  4. 遷移トリガー effect と補助 effect（RejectUntrustedReadyIssue など）をキューへ積む
  5. state + metadata を DB にコミット
```

Apply は pure decision layer であり、signal から「この cycle でどんな state 更新を行うか」「どんな effect を実行するか」をフラットに決める。GitHub API 呼び出しや process spawn などの副作用実行は全て Execute が担当する。

メタデータの更新ルールは [metadata.md](./metadata.md) を参照。

Apply は workflow state、補助メタデータ、effect planning を分けずに扱う。特に `Weight*Observed` シグナルは workflow 遷移を伴わなくても毎サイクル評価され、現在値と異なる場合に `weight` を更新する。`UntrustedReadyIssueObserved` のように workflow 遷移を起こさず effect だけを発火するシグナルも、同じ Apply レイヤーで解決する。

## Execute

Apply がキューに積んだ遷移トリガーエフェクトを実行し、加えて状態トリガーエフェクトを毎サイクル判定・実行する。

エフェクトは Issue ごとに順次実行される。前のエフェクトが失敗した場合、後続はスキップされる。

### 状態トリガーエフェクト（毎サイクル判定）

| 条件 | エフェクト |
|------|-----------|
| `InitializeRunning` かつ `initialized = false` かつ init handle なし | Initialize（tokio::spawn） |
| `ImplementationRunning` かつ `current_pid = null` | SwitchToImplBranch → SpawnProcess（順次） |
| その他プロセス実行中状態 かつ `current_pid = null` かつ `CiFixExhausted` シグナルなし | SpawnProcess |

### 遷移トリガーエフェクト（Apply がキュー積み）

| エフェクト | 実行方法 |
|-----------|---------|
| PostCompletedComment | 同期実行（best-effort） |
| CleanupWorktree | 同期実行（best-effort） |
| RejectUntrustedReadyIssue | 同期実行（best-effort） |
| CancelWithoutCleanup | 同期実行（best-effort） |
| CancelRetryExhausted | 同期実行（best-effort） |
| PostCiFixLimitComment | 同期実行（best-effort） |

### Claude Code スポーンの前提

プロセスをスポーンする前に以下のメタデータが必要：

| メタデータ | 用途 |
|-----------|------|
| `worktree_path` | 作業ディレクトリの指定 |
| `weight` | モデル選択 |
| `fixing_causes` | Fixing 状態時の input files 生成 |

## プロセス管理

### セッションマネージャ

実行中プロセスを `HashMap<issue_id, SessionEntry>` で管理する。

```
SessionEntry:
  child:             プロセスハンドル
  started_at:        スポーン時刻（stall 検知用）
  registered_state:  スポーン時の State（stale guard 用）
  stdout/stderr:     スレッドで並行読み取り（バッファ満杯防止）
  log_id:            実行ログ ID
```

### Stale Guard

プロセス終了検知時、`registered_state != current_state` の場合はスキップする。状態が既に進んでいるプロセスの終了を誤って拾うことを防ぐ。

## モデル選択

`weight × state-key` の組み合わせでモデルを選択する。4段フォールバック：

1. `weight` 固有 + `state-key` 指定
2. `weight` 固有 + `base state-key`（`design_fix` → `design` 等）
3. `weight` 固有のみ
4. グローバルデフォルト

## 並行セッション制限

`max_concurrent_sessions`（デフォルト3）を超えるプロセスはスポーンしない。次サイクル以降に持ち越される。

## 起動時リカバリ

起動時に `current_pid` が残っているプロセス（前回クラッシュ時の孤児）を SIGKILL で終了し、`current_pid` を NULL にリセットする。次のサイクルで通常の再試行フローに入る。
