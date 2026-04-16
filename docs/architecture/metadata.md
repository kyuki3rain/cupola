# メタデータ更新ルール

各フィールドがいつ・誰によって更新されるかを定義する。更新の主体は4種類：

- **Resolve**: 非同期処理完了回収時に更新（プロセス終了・Init タスク完了）
- **Decide**: 更新値を決定する（DB 書き込みなし）
- **Persist**: Decide の決定を DB にコミットする
- **Execute**: 副作用実行時に更新（プロセス起動・cleanup 時）

Collect は純粋な観測のみで DB を書かない。

## Issue テーブル

### `state`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| 更新 | WorldSnapshot が遷移条件を満たした時 | Persist（Decide が決定） |

---

### `weight`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット/更新 | `github_issue.weight` が現在値と異なる時 | Persist（Decide が決定） |

---

### `feature_name`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `Idle → InitializeRunning` 遷移時（デフォルト: `issue-{N}`） | Persist（Decide が決定） |

---

### `worktree_path`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | Init タスク完了時（worktree 作成完了後） | Resolve |
| クリア | `CleanupWorktree` 成功時 | Execute |

---

### `ci_fix_count`

主に状態遷移時に更新される。Fixing 状態中に CI 失敗が観測されても遷移が発生しないため更新されない（例外: `*Fixing + pr.state == closed → *Running` 遷移時はリセット）。ただし上限到達時（ReviewWaiting + ci/conflict + ci_fix_count == max、遷移なし）は例外的に max+1 を書き込む。`Cancelled → Idle` 遷移時はリセットしない（CI修正上限は再始動後も引き継ぐ。has_review_comments による Fixing 遷移でリセット可能）。

**予算の単位は「PR 世代」**: ci_fix_count は PR close/merge/review_comments によるリセットを境界とする「PR 世代ごとの予算」として機能する。一度 max+1 に到達した後 CI が自然回復しても予算は復活しない（PR close → *Running 遷移まで持ち越される）。「連続失敗回数」ではなく「この PR 世代でのトータル CI/Conflict 修正要求回数」に近い意味合いになる。

**既知の副作用 — レビューコメントによる CI 予算リセット**: `has_review_comments` が成立した場合のリセットは CI/Conflict と同時でも適用される。このため、trusted reviewer が未解決スレッドを残し続けると、CI 修正上限（`max_ci_fix_cycles`）を実質的に回避できる。レビュースレッドが1本でも残っていれば毎回予算が 0 に戻るため、恒常的な CI 失敗があっても上限到達しない。これは「reviewer と協調して修正するケースでは CI 予算より reviewer の判断を優先する」という設計意図の副作用であり、CI 予算とレビュー予算を同一カウンタで管理していることに起因する。

| 操作 | タイミング | 主体 |
|------|-----------|------|
| インクリメント | ReviewWaiting → Fixing 遷移が `design_pr.ci_status == failure` / `design_pr.has_conflict` / `impl_pr.ci_status == failure` / `impl_pr.has_conflict` で発生した時 | Persist（Decide が決定） |
| `max_ci_fix_cycles + 1` にセット | ReviewWaiting + `(ci_status == failure \| has_conflict)` + `ci_fix_count == max`（上限到達時の修正要求カウント。PostCiFixLimitComment と同時に Decide が決定） | Persist（Decide が決定） |
| リセット | ReviewWaiting → Fixing 遷移が `design_pr.has_review_comments` / `impl_pr.has_review_comments` で発生した時（CI/Conflict と同時でもリセット） | Persist（Decide が決定） |
| リセット | `*ReviewWaiting + pr.state == closed` → `*Running` 遷移時（PR 再作成のため新 PR 世代の CI 修正カウントをリセット） | Persist（Decide が決定） |
| リセット | `*Fixing + pr.state == closed` → `*Running` 遷移時（同上） | Persist（Decide が決定） |
| リセット | `design_pr.state == merged` → `ImplementationRunning` 遷移時 | Persist（Decide が決定） |
| リセット | `impl_pr.state == merged` → `Completed` 遷移時 | Persist（Decide が決定） |
| `0` にリセット | `cupola cleanup` 実行時（`Cancelled` Issue のみ対象） | cleanup コマンド |

---

### `consecutive_failures_epoch`

`Cancelled → Idle` 遷移時に現在時刻をセットする。Collect はこれ以降に作成された ProcessRun のみを `consecutive_failures` の集計対象にすることで、再起動後の連続失敗カウントを 0 からやり直す。

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `Cancelled → Idle` 遷移時（現在時刻） | Persist（Decide が決定） |

---

### `close_finished`

CloseIssue API 呼び出しが成功したかどうかを追跡するフラグ。Issue が既に人手でクローズ済みの場合も API は冪等に成功するため `true` になる。Cancelled では、このフラグが `true` かつ `github_issue is Open`（再オープン検知）になると `Cancelled → Idle` 遷移のトリガーとなる。

| 操作 | タイミング | 主体 |
|------|-----------|------|
| `true` にセット | CloseIssue 成功時（Completed・Cancelled 両方） | Execute |
| `false` にリセット | `Cancelled → Idle` 遷移時 | Persist（Decide が決定） |

**既知の制約 — CloseIssue 永続失敗によるデッドロック**: cupola の GitHub トークンに Issue クローズ権限がないなど、CloseIssue が永続的に失敗し続けると `close_finished` が `false` のままとなる。この場合、Issue が GitHub 上で open であっても `Cancelled → Idle` 遷移条件（`close_finished && github_issue is Open`）が成立しないため、再起動不能になる。回復手順: (a) 権限を修正して CloseIssue が成功するようにする、または (b) DB で `close_finished = true` に直接更新する。

---

## ProcessRun テーブル

### レコード全体

| 操作 | タイミング | 主体 |
|------|-----------|------|
| INSERT（state=running） | SpawnProcess エフェクト実行時、セッション枠あり | Execute |
| INSERT（state=pending） | SpawnProcess エフェクト実行時、セッション枠なし | Execute |

---

### `pid`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | プロセス起動時（state=running での INSERT または pending → running UPDATE と同時） | Execute |
| クリア | プロセス終了検知時 | Resolve |
| クリア | 起動時リカバリ（孤児プロセス kill 後） | Resolve |

---

### `state`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| `running` で INSERT | SpawnProcess エフェクト実行時（pending_run_id=None、セッション枠あり） | Execute |
| `pending` で INSERT | SpawnProcess エフェクト実行時（pending_run_id=None、セッション枠なし） | Execute |
| `running` に UPDATE | SpawnProcess エフェクト実行時（pending_run_id=Some、セッション枠あり） | Execute |
| `succeeded` に UPDATE | プロセス成功終了かつ Resolve 内後処理成功時 | Resolve |
| `failed` に UPDATE | 非 0 終了、stall kill、Init タスク失敗、または Resolve 内後処理失敗時 | Resolve |
| `stale` に UPDATE | Stale Guard 発動時（終了検知時に `registered_state != current_state`） | Resolve |

---

### `pr_number`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `DesignRunning` / `ImplementationRunning` でプロセス終了・PR 作成完了時 | Resolve |
| クリア（`None`） | `cupola cleanup` 実行時（`Cancelled` Issue のみ対象） | cleanup コマンド |

---

### `error_message`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | `state = failed` に UPDATE する時 | Resolve |

---

### `finished_at`

| 操作 | タイミング | 主体 |
|------|-----------|------|
| セット | プロセス終了検知時 | Resolve |

---

## 導出値

以下の値は DB フィールドとして保持せず、ProcessRun テーブルから動的に導出する。

| 値 | 導出方法 |
|----|---------|
| `retry_count` 相当 | `ProcessSnapshot.consecutive_failures`（Collect が集計。`consecutive_failures_epoch` 以降に作成された ProcessRun の中で、同 type の最新から遡った連続失敗数） |
| `initialized` 相当 | `ProcessRun(type=init).state == succeeded` |
| `design_pr_number` 相当 | `ProcessRun(type=design).pr_number` |
| `impl_pr_number` 相当 | `ProcessRun(type=impl).pr_number` |
| `current_pid` 相当 | `ProcessRun(state=running).pid`（当該 issue の実行中レコード） |
| `fixing_causes` 相当 | `ProcessRun(type=*_fix, 最新).causes`（監査用記録。Decide は毎サイクル WorldSnapshot から再決定する。詳細は effects.md SpawnProcess 参照） |
| `error_message` 相当 | `ProcessRun(最新).error_message` |
