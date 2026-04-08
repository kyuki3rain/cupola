# ポーリングループ設計

## 概要

ポーリングループは `polling_interval_secs`（デフォルト60秒）ごとに1サイクル実行される。1サイクルは5ステップで構成される。

```
┌─────────────────────────────────────────────┐
│  Resolve（非同期処理の完了回収・後処理）       │
│    プロセス終了検知 → ProcessRun を更新       │
├─────────────────────────────────────────────┤
│  Collect（純粋な観測）                        │
│    GitHub API・DB から観測                    │
│    → Issue ごとの WorldSnapshot を構築        │
├─────────────────────────────────────────────┤
│  Decide（純粋関数、DB 書き込みなし）          │
│    (prev_state, WorldSnapshot)               │
│    → (next_state, metadata_updates, effects) │
├─────────────────────────────────────────────┤
│  Persist（DB コミット）                      │
│    Decide の決定を DB に書く                 │
├─────────────────────────────────────────────┤
│  Execute（副作用実行）                        │
│    effects を順次実行                        │
│    ProcessRun を INSERT / worktree 操作      │
└─────────────────────────────────────────────┘
```

## Resolve

非同期処理の完了を回収し、ProcessRun レコードと Issue メタデータを更新する。

### Claude Code プロセスの終了処理

SessionManager から終了済みセッションを回収し、状態に応じて後処理を実行する。

| 終了状態 | 後処理 | ProcessRun 更新 |
|---------|--------|----------------|
| Running 系 + 成功、後処理も成功 | stdout パース → PR 作成（GitHub API） | `state=succeeded`, `pr_number` セット, `pid` クリア, `finished_at` セット |
| Running 系 + 成功、後処理失敗 | stdout パース → PR 作成失敗 | `state=failed`, `error_message` セット, `pid` クリア, `finished_at` セット |

| Fixing 系 + 成功、後処理も成功 | stdout パース → スレッド返信・resolve（GitHub API） | `state=succeeded`, `pid` クリア, `finished_at` セット |
| Fixing 系 + 成功、後処理失敗 | スレッド返信・resolve 失敗 | `state=failed`, `error_message` セット, `pid` クリア, `finished_at` セット |
| 失敗（非 0 終了） | なし | `state=failed`, `error_message` セット, `pid` クリア, `finished_at` セット |

**不変条件（Running 系のみ）**: design/impl の Running 系プロセスで `state=succeeded` の場合、`pr_number` が DB に保存済みであることを保証する。pr_number 書き込み失敗は後処理失敗として `state=failed` に分類される。Fixing 系・Init 系の `state=succeeded` は pr_number を持たない。

PR タイトル・本文は Claude Code の `structured_output` から取得し、パース失敗時はフォールバック文字列を使用する。

**design PR 本文の closing keyword 除去**: design PR 作成時、本文から GitHub の closing keyword（`close[sd]?` / `fix(e[sd])?` / `resolve[sd]?` + 任意の `owner/repo` + `#N`）を除去してから `create_pr` を呼ぶ。Issue のライフサイクルは cupola の状態機械が所有しており、design PR の merge で issue が auto-close されると state machine の想定外の Cancelled 遷移を招くため。impl PR 本文は除去しない — impl PR merge による auto-close は cupola の `CloseIssue` effect と一致する正規の完了パスであり、むしろ保持すべき挙動である。

**stdout/stderr の永続化（best-effort）**: Claude Code プロセスの終了時、`ProcessRun.id > 0` であれば stdout・stderr 全文を `.cupola/logs/process-runs/run-{run_id}-stdout.log` と `run-{run_id}-stderr.log` に書き出す。`error_message` には stderr 先頭 512 文字しか載らないため、Claude が stderr を出さず stdout に JSON エラーを出して終了するケース（exit code 1 + stderr 空）でも原因特定できるよう、終了した全プロセス（成功・失敗問わず）について保存する。書き込み失敗は `tracing::warn!` に留め Resolve 本体の成否には影響させない。`cupola logs` コマンドはこのサブディレクトリを見ない（後述）。

**PR 作成の冪等性要件**: GitHub 上に PR が作成された後、DB への `pr_number` 書き込みが失敗すると、次サイクルで PR を観測できず重複作成が走る可能性がある。実装では PR 作成前にブランチに対する **open 状態の** 既存 PR を検索し、存在する場合は作成をスキップして `pr_number` のみ取得すること。closed PR は検索対象に含めない（含めると、PR close → *Running 遷移後に closed PR を再発見して pr_number を取得し、次サイクルで closed PR を観測して再度 *Running に戻る永久ループが発生する）。

### Init タスクの完了処理

`InitTaskManager`（`HashMap<issue_id, JoinHandle<Result<()>>>`）を issue ループ内で確認する。

```rust
if let Some(handle) = self.init_handles.get(&issue.id) {
    if handle.is_finished() {
        let handle = self.init_handles.remove(&issue.id).unwrap();
        match handle.await {
            Ok(Ok(_)) => {
                // ProcessRun(type=init).state = succeeded
                // Issue.worktree_path をセット
            }
            _ => {
                // ProcessRun(type=init).state = failed
                // ProcessRun.error_message をセット
            }
        }
    }
}
```

### Stale Guard

プロセス終了検知時、`registered_state != current_state` の場合は後処理（PR 作成・コメント投稿）をスキップし、**ProcessRun.state は `stale` に UPDATE する**。`running` のまま残すと次サイクルの Collect が SpawnProcess を抑止してしまうため終了状態にする必要があるが、`failed` にすると consecutive_failures を汚染するため `stale` で区別する。

**同一状態内での PR 進行**: Stale Guard は `registered_state != current_state` のみを判定するため、Fixing プロセスの完了と PR の merge/close が同一サイクル内で発生した場合（Resolve がプロセス完了を検知し、同サイクルの Collect が PR merge を観測）、Resolve 時点では Persist 前のため `current_state` がまだ Fixing のまま。Stale Guard は発動せず後処理（スレッド返信・resolve）が実行される。次サイクルでは `registered_state（Fixing）≠ current_state（ImplementationRunning 等）` で正しく stale になる。実害なし（merge 済み PR へのコメント・スレッド resolve は API 上許容される）。

### Stall 検知

`stall_timeout_secs`（デフォルト30分）を超過したプロセスをこのステップで SIGKILL する。kill されたプロセスは同サイクルの Resolve で失敗として処理される。

## Collect

副作用なし。GitHub API・DB から観測を行い、Issue ごとに WorldSnapshot を構築する。

WorldSnapshot の構造と構築ロジックの詳細は [observations.md](./observations.md) を参照。

Resolve で ProcessRun の `pr_number` が確定している場合、同サイクル内で `design_pr` / `impl_pr` の観測が可能。

## Decide

各 Issue の `(prev_state, WorldSnapshot)` を受け取り、`(next_state, metadata_updates, effects)` を決定する純粋関数層。DB 書き込みは行わない。

```
for each issue:
  1. WorldSnapshot と遷移テーブルを照合（優先度順）
  2. next_state を決定
  3. metadata_updates（weight / ci_fix_count / feature_name 等）を決定
  4. 一回性エフェクト: prev_state → next_state の遷移を検出して発行
  5. イベント時・持続性エフェクト: next_state と WorldSnapshot を見て毎サイクル発行判断
     （遷移サイクルでも next_state 基準で評価するため、遷移と同一サイクルで発火しうる。
      prev_state ではなく next_state を使うことで、遷移によって条件が成立しなくなるケースを正しく除外できる）
  6. (next_state, metadata_updates, effects) を返す
```

### Decide の入出力

```
Input:
  prev_state: { state, ci_fix_count, close_finished, weight, worktree_path, feature_name }
  snapshot:   WorldSnapshot

Output:
  next_state:       State
  metadata_updates: { weight, ci_fix_count, close_finished, feature_name, ... }
  effects:          Vec<Effect>
```

エフェクトの詳細は [effects.md](./effects.md) を参照。メタデータ更新ルールの詳細は [metadata.md](./metadata.md) を参照。

### Entry effect 不変条件

`*Running` 系の next_state（`InitializeRunning` / `DesignRunning` / `ImplementationRunning`）への**遷移**を返す分岐では、同じサイクル内で対応する起動 effect（`SpawnInit` / `SpawnProcess { Design }` / `SpawnProcess { Impl }`）を必ず effects に push する。これがないと「遷移サイクルでは Execute が起動 effect を発行せず、次サイクルの Decide が `processes.* == None` を観測してようやく spawn する」という 1 サイクル分（デフォルト 30 秒）の遅延が生まれる。

すでに同 state に滞在しているサイクル（`prev_state == next_state`）では Decide 内の通常分岐（`processes.* == None` や `Failed` / `Stale` など）が再 spawn を担当するため、entry effect は遷移エッジでのみ発行する。同一サイクル内で entry effect が走った後の次サイクルでは、Persist で挿入された ProcessRun が `state=running` で観測されるため重複 spawn は発生しない。

## Persist

Decide の決定（next_state、metadata_updates）を DB にコミットする。副作用（GitHub API・プロセス起動）は行わない。

```
for each issue:
  UPDATE issues SET
    state           = next_state,
    weight          = metadata_updates.weight,
    ci_fix_count    = metadata_updates.ci_fix_count,
    close_finished  = metadata_updates.close_finished,  // Cancelled→Idle 遷移時に false にリセット
    feature_name    = metadata_updates.feature_name,    // 変更がある場合のみ
    updated_at      = now()
```

## Execute

Decide が生成した effects リストを Issue ごとに順次実行する。前のエフェクトが失敗した場合、後続のエフェクトはスキップされる（best-effort エフェクトはエラーをログのみ残して継続）。

エフェクトの詳細は [effects.md](./effects.md) を参照。

### プロセス起動時の ProcessRun 作成

SpawnInit / SpawnProcess 実行時、Execute はプロセスをスポーンする直前に ProcessRun レコードを INSERT する。

```rust
let run = ProcessRun {
    issue_id:   issue.id,
    type:       process_type,
    index:      next_index,   // 同 type の最大 index + 1
    state:      running,
    pid:        None,         // スポーン後にセット
    causes:     fixing_causes, // *_fix の場合のみ
    ..
};
db.insert(run);
// プロセス起動
match spawn_process(...) {
    Ok(pid) => db.update_pid(run.id, pid),
    Err(e)  => db.update_failed(run.id, e.to_string()),  // state=failed, error_message セット
}
```

**起動失敗時の規約**: `spawn_process` が同期的に失敗した場合（CLI 不在・権限不足等）、同一サイクル内で `ProcessRun.state = failed` に UPDATE する。これにより次サイクルの Collect が `state == failed` を観測し、通常のリトライフローに入る。ProcessRun が `state=running` のまま残ると次サイクルで SpawnProcess が発火せず詰まるため、起動失敗は必ず同期的に failed 化すること。

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
  process_run_id:    ProcessRun.id
```

## モデル選択

`weight × state-key` の組み合わせでモデルを選択する。4段フォールバック：

1. `weight` 固有 + `state-key` 指定
2. `weight` 固有 + `base state-key`（`design_fix` → `design` 等）
3. `weight` 固有のみ
4. グローバルデフォルト

## 並行セッション制限

`max_concurrent_sessions`（デフォルト3）を超えるプロセスはスポーンしない。次サイクル以降に持ち越される。上限チェックは SessionManager のエントリ数（実行中プロセス数）を基準とし、ProcessRun.state=running のカウントではない。ProcessRun INSERT は上限チェック通過後・スポーン直前に行われるため、INSERT がセッション数に影響することはない。

## 起動時リカバリ

起動時に ProcessRun.state=running のレコードが残っているプロセス（前回クラッシュ時の孤児）を SIGKILL で終了し、ProcessRun.state=failed に更新する。次のサイクルで通常の再試行フローに入る。
