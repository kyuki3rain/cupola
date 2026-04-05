# メタデータ更新ルール

各フィールドがいつ・誰によって更新されるかを定義する。更新の主体は3種類：

- **Resolve**: 非同期処理完了回収時に更新（プロセス終了・Init タスク完了）
- **Apply**: 状態遷移時に更新
- **Execute**: 副作用実行時に更新（プロセススポーン時）

Collect は純粋な観測のみで DB を書かない。

## フィールド別更新ルール

### `state`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| 更新 | シグナルの組み合わせが遷移条件を満たした時 | Apply |

---

### `weight`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット/更新 | `WeightHeavyObserved` / `WeightLightObserved` / `WeightMediumObserved` から導出した値が現在値と異なる時 | Apply |

---

### `feature_name`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `ReadyIssueObserved` による `Idle → InitializeRunning` 遷移時（Issue 番号から生成） | Apply |

---

### `worktree_path`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | Init タスク完了時（worktree 作成完了後） | Resolve |
| クリア | `CleanupWorktree` 成功時 | Execute |

---

### `initialized`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| `true` にセット | Init タスクの JoinHandle が `Ok(Ok(_))` の時 | Resolve |
| `false` にリセット | `InitializeRunning` に遷移する時 | Apply |

`InitializeRunning` 再遷移時にリセットすることで、Cancelled からの再起動時に初期化が再実行される。

---

### `design_pr_number`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `DesignRunning` でプロセス終了・PR 作成完了時 | Resolve |

---

### `impl_pr_number`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `ImplementationRunning` でプロセス終了・PR 作成完了時 | Resolve |

---

### `current_pid`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | プロセス起動時 | Execute |
| クリア | プロセス終了検知時 | Resolve |
| クリア | 起動時リカバリ（孤児プロセス kill 後） | Resolve |

---

### `last_process_exit`

Resolve と Collect の橋渡し用メタデータ。stale guard を通過した最新のプロセス終了結果だけを保持する。

| 操作 | タイミング | 主体 |
|------|-----------|------|
| `Succeeded` にセット | プロセス成功終了かつ Resolve 内後処理成功時 | Resolve |
| `Failed` にセット | 非 0 終了、stall kill、Init タスク失敗、または Resolve 内後処理失敗時 | Resolve |
| `None` にリセット | Collect が `ProcessSucceeded` / `ProcessFailed` を生成した後 | Collect |

`registered_state != current_state` の stale な終了結果は Resolve が破棄するため、`last_process_exit` には保存しない。

---

### `retry_count`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| インクリメント | `last_process_exit = Failed` を保存する時 | Resolve |
| リセット | `ProcessFailed` 以外のシグナルで状態遷移した時 | Apply |

---

### `ci_fix_count`

ReviewWaiting 状態からの遷移時のみ更新される。Fixing 状態中に CI 失敗シグナルが観測されても遷移が発生しないため更新されない。

| 操作 | タイミング | 主体 |
|------|-----------|------|
| インクリメント | ReviewWaiting → Fixing 遷移が `DesignCiFailureFound` / `DesignConflictFound` / `ImplCiFailureFound` / `ImplConflictFound` で発生した時 | Apply |
| `max_ci_fix_cycles + 1` にセット | `CiFixExhausted` により遷移なし（`—`）となり、上限到達コメントを投稿した時（再投稿防止） | Apply |
| リセット | ReviewWaiting → Fixing 遷移が `DesignReviewCommentFound` / `ImplReviewCommentFound` で発生した時（CI/Conflict と同時でもリセット） | Apply |
| リセット | `DesignPrMerged` → `ImplementationRunning` 遷移時 | Apply |
| リセット | `ImplPrMerged` → `Completed` 遷移時 | Apply |

---

### `fixing_causes`

ReviewWaiting 状態からの遷移時のみ更新される。Fixing 状態中に新たな CI 失敗等が観測されても遷移が発生しないため更新されない。

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | ReviewWaiting → Fixing 遷移時（遷移を引き起こしたシグナル群から決定） | Apply |
| クリア | Fixing 系プロセス成功を `last_process_exit = Succeeded` として保存する時 | Resolve |

---

### `error_message`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `last_process_exit = Failed` を保存する時 | Resolve |
| クリア | `ProcessFailed` 以外のシグナルで遷移が発生した時 | Apply |
