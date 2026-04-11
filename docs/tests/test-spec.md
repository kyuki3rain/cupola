# Cupola 自動テストカタログ

Cupola の自動テストが網羅すべき振る舞いを領域別に列挙した**テストケース仕様**。
`docs/architecture/` と `docs/commands/` に書かれた設計を 1:1 で自動テストに落とす
ための参照資料であり、新しいテストを追加する時・既存テストを読み直す時・
仕様変更時の影響範囲を洗い出す時に使う。

- 各項目の ID（例: `T-D.drw.6`）は Rust の `#[test] fn` 名として安定運用する。
  名前を変更する場合はこのファイルと実装テストを同時に更新する。
- 項目はすべて **docs 側の記述が正本**。docs が変われば対応する項目も更新する。
- このファイルに載っていない振る舞いをテストで保証している場合、実装側のテスト
  コメントでその旨を明記し、本ファイルにも追記する。

## 目次

| セクション | 領域 | ID 前置詞 |
|---|---|---|
| 1 | Domain — State | `T-S.*` |
| 2 | Domain — ProcessRun | `T-P.*` |
| 3 | Domain — Issue | `T-I.*` |
| 4 | Domain — WorldSnapshot | `T-W.*` |
| 5 | Domain — Effect | `T-E.*` |
| 6 | Domain — decide （純粋遷移関数） | `T-D.*` |
| 7 | Domain — MetadataUpdates | `T-M.*` |
| 8 | Application — ProcessRunRepository | `T-R.*` |
| 9 | Application — IssueRepository スキーマ | `T-IR.*` |
| 10 | Application — Polling/Resolve | `T-RS.*` |
| 11 | Application — Polling/Collect | `T-CO.*` |
| 12 | Application — Polling/Persist | `T-PE.*` |
| 13 | Application — Polling/Execute | `T-EX.*` |
| 14 | Application — SessionManager | `T-SM.*` |
| 15 | Application — InitTaskManager | `T-IT.*` |
| 16 | Application — 起動時 orphan recovery | `T-O.*` |
| 17 | Application — cleanup コマンド | `T-CL.*` |
| 18 | Application — status コマンド | `T-ST.*` |
| 19 | Application — doctor コマンド | `T-DR.*` |
| 20 | Application — stop コマンド | `T-SP.*` |
| 21 | Application — init コマンド | `T-IN.*` |
| 22 | Application — compress コマンド | `T-CP.*` |
| 23 | Application — logs コマンド | `T-LG.*` |
| 24 | Adapter — GitHub | `T-GH.*` |
| 25 | Adapter — Git worktree | `T-WT.*` |
| 26 | Adapter — Claude Code プロセス | `T-CC.*` |
| 27 | 統合シナリオ | `T-IS.*` |

## モジュール対応表

| テストモジュール | 対象ソース |
|---|---|
| `domain::state::tests` | `src/domain/state.rs` |
| `domain::process_run::tests` | `src/domain/process_run.rs` |
| `domain::issue::tests` | `src/domain/issue.rs` |
| `domain::world_snapshot::tests` | `src/domain/world_snapshot.rs` |
| `domain::effect::tests` | `src/domain/effect.rs` |
| `domain::decide::tests` | `src/domain/decide.rs` |
| `domain::metadata_update::tests` | `src/domain/metadata_update.rs` |
| `application::polling::resolve::tests` | `src/application/polling/resolve.rs` |
| `application::polling::collect::tests` | `src/application/polling/collect.rs` |
| `application::polling::persist::tests` | `src/application/polling/persist.rs` |
| `application::polling::execute::tests` | `src/application/polling/execute.rs` |
| `application::session_manager::tests` | `src/application/session_manager.rs` |
| `application::init_task_manager::tests` | `src/application/init_task_manager.rs` |
| `application::cleanup_use_case::tests` | `src/application/cleanup_use_case.rs` |
| `application::status_use_case::tests` | `src/application/status_use_case.rs` |
| `application::doctor_use_case::tests` | `src/application/doctor_use_case.rs` |
| `application::stop_use_case::tests` | `src/application/stop_use_case.rs` |
| `application::start_use_case::tests` | `src/application/start_use_case.rs` |
| `application::init_use_case::tests` | `src/application/init_use_case.rs` |
| `application::compress_use_case::tests` | `src/application/compress_use_case.rs` |
| `application::logs_use_case::tests` | `src/application/logs_use_case.rs` |
| `adapter::outbound::sqlite_process_run_repository::tests` | `src/adapter/outbound/sqlite_process_run_repository.rs` |
| `adapter::outbound::sqlite_issue_repository::tests` | `src/adapter/outbound/sqlite_issue_repository.rs` |
| `adapter::outbound::github_rest_client::tests` | `src/adapter/outbound/github_rest_client.rs` |
| `adapter::outbound::github_graphql_client::tests` | `src/adapter/outbound/github_graphql_client.rs` |
| `adapter::outbound::git_worktree_manager::tests` | `src/adapter/outbound/git_worktree_manager.rs` |
| `adapter::outbound::claude_code_process::tests` | `src/adapter/outbound/claude_code_process.rs` |
| `tests/scenarios.rs` | エンドツーエンド統合シナリオ |

---

## 1. Domain — State (`T-S.*`)

- **T-S.1** `State` は 10 バリアントちょうど: `Idle`, `InitializeRunning`, `DesignRunning`, `DesignReviewWaiting`, `DesignFixing`, `ImplementationRunning`, `ImplementationReviewWaiting`, `ImplementationFixing`, `Completed`, `Cancelled`。
- **T-S.2** `is_terminal` は `Completed` / `Cancelled` のみ true。
- **T-S.3** `is_review_waiting` は `DesignReviewWaiting` / `ImplementationReviewWaiting` のみ true。
- **T-S.4** `Display` / `FromStr` は DB 保存用の snake_case (`idle`, `initialize_running`, …) で round-trip する。
- **T-S.5** 各バリアントが `docs/architecture/data-model.md` に記載の snake_case キーと 1:1 対応。

## 2. Domain — ProcessRun (`T-P.*`)

- **T-P.1** `ProcessRunType` は 5 バリアント: `Init`, `Design`, `DesignFix`, `Impl`, `ImplFix`。snake_case (`init` / `design` / `design_fix` / `impl` / `impl_fix`) で round-trip。
- **T-P.2** `ProcessRunState` は 5 バリアント: `Pending`, `Running`, `Succeeded`, `Failed`, `Stale`。snake_case で round-trip。
- **T-P.3** `ProcessRun` は `id, issue_id, type_, index, state, pid, pr_number, causes, error_message, started_at, finished_at` のみを保持。
- **T-P.4** `ProcessRun::new_running(...)` は `state=Running, pid=None, pr_number=None, finished_at=None` で構築。
- **T-P.5** `ProcessRun::new_pending(...)` は `state=Pending, pid=None, pr_number=None, error_message=None, finished_at=None` で構築。`causes` と `started_at` は INSERT 時に確定。

## 3. Domain — Issue (`T-I.*`)

- **T-I.1** `Issue` は `id, github_issue_number, state, feature_name, weight, worktree_path, ci_fix_count, close_finished, consecutive_failures_epoch, created_at, updated_at` のみを保持する。PR 番号・PID・リトライ数・エラーメッセージ・fixing causes は `process_runs` テーブルから導出する。
- **T-I.2** デフォルト状態は `state=Idle, ci_fix_count=0, close_finished=false, consecutive_failures_epoch=None`。

## 4. Domain — WorldSnapshot (`T-W.*`)

`docs/architecture/observations.md` が正本。

- **T-W.1** `WorldSnapshot` は `github_issue, design_pr, impl_pr, processes, ci_fix_exhausted` を保持。
- **T-W.2** `GithubIssueSnapshot` は `state, has_ready_label, ready_label_trusted, weight` を保持。
- **T-W.3** `PrSnapshot` は `state ∈ {Open, Merged, Closed}, has_review_comments, ci_status ∈ {Ok, Failure, Unknown}, has_conflict`。
- **T-W.4** `ProcessesSnapshot` は `init, design, design_fix, impl_, impl_fix: Option<ProcessSnapshot>`。
- **T-W.5** `ProcessSnapshot` は `state ∈ {Pending, Running, Succeeded, Failed, Stale}, index, run_id, consecutive_failures: u32`。
- **T-W.6** テスト用 fixture（`world_snapshot::fixtures::*`）が存在し、decide テストで再利用できる。

## 5. Domain — Effect (`T-E.*`)

`docs/architecture/effects.md` が正本。

- **T-E.1** `Effect` は 10 バリアントちょうど: `PostCompletedComment, PostCancelComment, PostRetryExhaustedComment, RejectUntrustedReadyIssue, PostCiFixLimitComment, SpawnInit, SwitchToImplBranch, SpawnProcess { type_, causes, pending_run_id }, CleanupWorktree, CloseIssue`。
- **T-E.2** `Effect::priority()` はグローバル優先度順 (transition → event → SpawnInit → SwitchToImplBranch → SpawnProcess → CleanupWorktree → CloseIssue)。`Vec<Effect>` を priority でソートすると docs 定義の順序になる。
- **T-E.3** `is_best_effort()` は `PostCompletedComment / PostCancelComment / PostRetryExhaustedComment / RejectUntrustedReadyIssue / PostCiFixLimitComment / CleanupWorktree / CloseIssue` で true、`SpawnInit / SwitchToImplBranch / SpawnProcess` で false。

## 6. Domain — `decide` 純粋関数 (`T-D.*`)

`decide(prev: &Issue, snap: &WorldSnapshot, cfg: &Config) -> Decision`。
正本は `docs/architecture/state-machine.md` の全遷移テーブル。1 行 = 1 テスト。

### 6.1 Idle (`T-D.idle.*`)
- **T-D.idle.1** `has_ready_label && ready_label_trusted` → `InitializeRunning`。
- **T-D.idle.2** `has_ready_label && !ready_label_trusted` → Idle 維持、`RejectUntrustedReadyIssue` を発行。
- **T-D.idle.3** ready ラベルなし → Idle 維持、effects なし。

### 6.2 InitializeRunning (`T-D.init.*`)
- **T-D.init.1** `issue.closed` → `Cancelled` + `PostCancelComment`。
- **T-D.init.2** `init.consecutive_failures >= max_retries` → `Cancelled` + `PostRetryExhaustedComment`。
- **T-D.init.3** `init.state == failed`（上限未到達）→ 状態維持、`SpawnInit`。
- **T-D.init.4** `init.succeeded + impl_pr.merged` → `Completed` + `PostCompletedComment + CleanupWorktree + CloseIssue`。
- **T-D.init.5** `init.succeeded + impl_pr.open` → `ImplementationReviewWaiting`。
- **T-D.init.6** `init.succeeded + design_pr.merged` → `ImplementationRunning` + `ci_fix_count` リセット。
- **T-D.init.7** `init.succeeded + design_pr.open` → `DesignReviewWaiting`。
- **T-D.init.8** `init.succeeded`（PR なし）→ `DesignRunning`。
- **T-D.init.9** `init == None` → 状態維持、`SpawnInit`。
- **T-D.init.10** スマートルーティング優先度: issue closed 行は他のどの行より優先。

### 6.3 DesignRunning (`T-D.dr.*`)
- **T-D.dr.1** `issue.closed` → `Cancelled` + `PostCancelComment`。
- **T-D.dr.2** `design.consecutive_failures >= max_retries` → `Cancelled` + `PostRetryExhaustedComment`。
- **T-D.dr.3** `design.pending` → 状態維持、`SpawnProcess(design, pending_run_id=Some)`。
- **T-D.dr.4** `design.failed`（上限未到達）→ 状態維持、`SpawnProcess(design, pending_run_id=None)`。
- **T-D.dr.5** `design.stale` → 状態維持、`SpawnProcess(design, pending_run_id=None)`。
- **T-D.dr.6** `design.succeeded + design_pr.merged` → `ImplementationRunning` + `ci_fix_count` リセット。
- **T-D.dr.7** `design.succeeded + design_pr.closed` → 状態維持（PR 再作成）+ `SpawnProcess(design, pending_run_id=None)` + `ci_fix_count` リセット。
- **T-D.dr.8** `design.succeeded + design_pr.open` → `DesignReviewWaiting`。
- **T-D.dr.9** プロセスレコードなし → `SpawnProcess(design, pending_run_id=None)`。

### 6.4 DesignReviewWaiting (`T-D.drw.*`)
- **T-D.drw.1** `issue.closed` → `Cancelled` + `PostCancelComment`。
- **T-D.drw.2** `design_pr.merged` → `ImplementationRunning` + `ci_fix_count` リセット。
- **T-D.drw.3** `design_pr.closed` → `DesignRunning` + `ci_fix_count` リセット。
- **T-D.drw.4** `has_review_comments` → `DesignFixing` + `ci_fix_count` を 0 にリセット + `causes` に `ReviewComments`。
- **T-D.drw.5** `has_review_comments + ci_failure` → `DesignFixing` + `ci_fix_count` リセット（review がリセット権を持つ。`causes` は両方含む）。
- **T-D.drw.6** `ci_status.failure + ci_fix_exhausted` → 状態維持、`PostCiFixLimitComment`、`metadata.ci_fix_count = max+1`。
- **T-D.drw.7** `ci_status.failure`（上限未到達）→ `DesignFixing` + `ci_fix_count` インクリメント + `causes` に `CiFailure`。
- **T-D.drw.8** `has_conflict + ci_fix_exhausted` → 状態維持、`PostCiFixLimitComment`、`ci_fix_count = max+1`。
- **T-D.drw.9** `has_conflict`（上限未到達）→ `DesignFixing` + `ci_fix_count` インクリメント + `causes` に `Conflict`。
- **T-D.drw.10** `design_pr == None` → `DesignRunning`。
- **T-D.drw.11** 優先度: merge > close > review > CI > conflict。
- **T-D.drw.12** `PostCiFixLimitComment` 一回性: 次サイクルは `ci_fix_count == max+1` になるためトリガ条件 `== max` が成立せず再発火しない。

### 6.5 DesignFixing (`T-D.df.*`)
- **T-D.df.1** `issue.closed` → `Cancelled` + `PostCancelComment`。
- **T-D.df.2** `design_pr.merged` → `ImplementationRunning` + `ci_fix_count` リセット。
- **T-D.df.3** `design_pr.closed` → `DesignRunning` + `ci_fix_count` リセット。
- **T-D.df.4** `design_pr == None` → `DesignRunning`。
- **T-D.df.5** `design_fix.consecutive_failures >= max_retries` → `Cancelled` + `PostRetryExhaustedComment`。
- **T-D.df.6** `design_fix.pending` → 状態維持、`SpawnProcess(design_fix, causes, pending_run_id=Some)`。
- **T-D.df.7** `design_fix.failed`（上限未到達）→ 状態維持、`SpawnProcess(design_fix, causes, pending_run_id=None)`。
- **T-D.df.8** `design_fix.stale` → 状態維持、`SpawnProcess(design_fix, causes, pending_run_id=None)`。
- **T-D.df.9** `design_fix.succeeded` → `DesignReviewWaiting`。
- **T-D.df.10** プロセスレコードなし → `SpawnProcess(design_fix, causes, pending_run_id=None)`。
- **T-D.df.11** `causes` は毎サイクル `WorldSnapshot` から再決定される（過去 ProcessRun の causes は参照しない）。

### 6.6 ImplementationRunning (`T-D.ir.*`)
DesignRunning と対称（`impl` / `impl_pr` / 到達先 `Completed`）。

- **T-D.ir.1–9** DesignRunning `T-D.dr.1–9` と同形。
- **T-D.ir.9** `SpawnProcess` が発行される場合、`SwitchToImplBranch` が前置され優先度順で前に並ぶ。

### 6.7 ImplementationReviewWaiting (`T-D.irw.*`)
DesignReviewWaiting と対称（到達先 `Completed`）。`T-D.irw.1–12` は `T-D.drw.1–12` と同形。

- **T-D.irw.merge** `impl_pr.merged` → `Completed` + `PostCompletedComment + CleanupWorktree + CloseIssue` + `ci_fix_count` リセット。

### 6.8 ImplementationFixing (`T-D.if.*`)
DesignFixing と対称。`T-D.if.1–11` は `T-D.df.1–11` と同形。

### 6.9 Completed (`T-D.c.*`)
- **T-D.c.1** 他の状態へ遷移しない。
- **T-D.c.2** `worktree_path != null` → `CleanupWorktree` 発行（持続性）。
- **T-D.c.3** `!close_finished` → `CloseIssue` 発行。
- **T-D.c.4** `worktree_path == null && close_finished` → effects なし。

### 6.10 Cancelled (`T-D.x.*`)
- **T-D.x.1** `close_finished && github_issue.state == open` → `Idle` + `metadata.consecutive_failures_epoch = now` + `metadata.close_finished = false`。
- **T-D.x.2** `!close_finished` → 状態維持、`CloseIssue` 発行。
- **T-D.x.3** `close_finished && github_issue.state == closed` → 状態維持、effects なし。
- **T-D.x.4** Cancelled → Idle 遷移は `ci_fix_count` を**リセットしない**。

### 6.11 Cross-cutting (`T-D.X.*`)
- **T-D.X.1** `snap.github_issue.weight != prev.weight` のとき `weight` が `MetadataUpdates` に入る。
- **T-D.X.2** `Decision.effects` は挿入順に関わらずグローバル優先度順で返る。
- **T-D.X.3** `decide` は I/O を行わない（借用のみ引数、Send + 'static 境界で検証）。
- **T-D.X.4** `PostCiFixLimitComment` と `ci_fix_count = max+1` は同一 `MetadataUpdates` に載る。

## 7. Domain — MetadataUpdates (`T-M.*`)

- **T-M.1** `MetadataUpdates { state, weight, ci_fix_count, close_finished, feature_name, consecutive_failures_epoch, worktree_path }` — 各フィールドは `Option<T>`、`None` は「変更しない」。
- **T-M.2** `MetadataUpdates::default()` は全フィールド `None`。
- **T-M.3** `apply_to(&mut Issue)` は `Some` のフィールドだけを書き換える。

## 8. Application — ProcessRunRepository (`T-R.*`)

- **T-R.1** `save(run)` は新規 id を返し、`state=running, index = max(index)+1`（`(issue_id, type_)` 内）で INSERT する。
- **T-R.2** `update_pid(id, pid)` は pid のみ更新する。
- **T-R.3** `mark_succeeded(id, pr_number)` は `state=succeeded, finished_at=now, pr_number` を書き込む。
- **T-R.4** `mark_failed(id, error)` は `state=failed, finished_at=now, error_message` を書き込む。
- **T-R.5** `mark_stale(id)` は `state=stale, pid=NULL, finished_at=now` を書き込む。
- **T-R.6** `mark_stale_for_issue(issue_id)` は同一 issue の `state=running` 行を一括で `stale / pid=NULL / finished_at=now` にする（起動時リカバリ用）。
- **T-R.7** `find_latest(issue_id, type)` は最大 index のレコードを返す。
- **T-R.8** `count_consecutive_failures(issue_id, type)` は index 降順で `failed` を数え、`succeeded|stale|running` に当たった時点で停止。
- **T-R.9** `find_all_running()` は `state=running` のレコードを全 issue 横断で返す。

## 9. Application — IssueRepository スキーマ (`T-IR.*`)

- **T-IR.1** スキーマ初期化は idempotent（2 回実行しても結果同じ）。
- **T-IR.2** `find_active()` は `Completed` / `Cancelled` を除外する。
- **T-IR.3** `find_all()` は終端状態も含めて返す。
- **T-IR.4** `update_state_and_metadata(issue_id, MetadataUpdates)` は `None` でないフィールドだけ書き込む。
- **T-IR.5** `consecutive_failures_epoch` は `DateTime<Utc>` および `None` を round-trip する。
- **T-IR.6** `close_finished` は `bool` を round-trip する。

## 10. Application — Polling/Resolve (`T-RS.*`)

`docs/architecture/polling-loop.md` Resolve 節が正本。

- **T-RS.1** 終了セッションかつ `registered_state == current_state`、exit=0:
  - Running 系 → PR 作成後処理 → `ProcessRun.state=succeeded` + `pr_number` セット。
- **T-RS.2** 終了セッションかつ `registered_state != current_state` → Stale Guard 発動: `mark_stale` 呼出（`state=stale, pid=NULL, finished_at=now`）、PR 作成・スレッド返信を**スキップ**。
- **T-RS.3** exit コード非 0 → `state=failed`、`error_message` に `"exit code {code}: {stderr snippet}"` を格納（stderr は `STDERR_SNIPPET_MAX` で切詰）。
- **T-RS.4** stall タイムアウト超過プロセス → SIGKILL 送信 → 次サイクルで failure として処理。
- **T-RS.5** Init `JoinHandle` が `Ok(path)` → `ProcessRun(init).state=succeeded` + `Issue.worktree_path` セット。
- **T-RS.6** Init `JoinHandle` が `Err` → `state=failed` + `error_message` 設定。
- **T-RS.7** Running 系の後処理（PR 作成）失敗 → `state=failed` に分類される（succeeded にしない）、`pr_number` は書かない。
- **T-RS.8** Fixing 系で後処理成功 → スレッド返信・resolve を GitHub に送信。返信失敗は warn ログ（ProcessRun は succeeded）。

## 11. Application — Polling/Collect (`T-CO.*`)

- **T-CO.1** 全 issue（Completed / Cancelled 含む）に対して `WorldSnapshot` を構築する（Completed の持続性 effect / Cancelled→Idle 遷移のため）。
- **T-CO.2** `design_pr` / `impl_pr` は `ProcessRun(type).pr_number IS NOT NULL` の最新レコードを起点に観測する。
- **T-CO.3** PR API 404 → snapshot は `None`。
- **T-CO.4** PR API 5xx → `Err` を返しその issue のサイクルのみスキップ。
- **T-CO.5** `consecutive_failures` は `consecutive_failures_epoch` 以降のレコードに限定して集計。
- **T-CO.6** `ci_fix_exhausted = (issue.ci_fix_count >= config.max_ci_fix_cycles)`。
- **T-CO.7** `has_review_comments` は trust 判定（`trusted_associations` によるロール判定 OR `trusted_reviewers` によるユーザー名判定）を通過するコメントを1件も含まないスレッドを除外する。除外されたスレッドは遷移トリガーにもならず、Claude Code にも渡されない。
- **T-CO.8** `ci_status` 集約: `failure | timed_out → Failure`、`cancelled | null → Unknown`、全 `success/neutral/skipped` で `Ok`、check runs ゼロ → `Unknown`。
- **T-CO.9** `has_conflict` は `mergeable == Some(false)` のときのみ true（`None` は false 扱い）。
- **T-CO.10** Collect は DB への書き込みを行わない（write を panic する mock で検証）。

## 12. Application — Polling/Persist (`T-PE.*`)

- **T-PE.1** `MetadataUpdates` のうち `Some` のフィールドだけを書き込む。
- **T-PE.2** `next_state != prev_state` のときは `state` と `updated_at` を更新する。
- **T-PE.3** `next_state == prev_state` かつ他の更新もない場合、`updated_at` をバンプしない。

## 13. Application — Polling/Execute (`T-EX.*`)

各 Effect について:

- **T-EX.post_completed** `PostCompletedComment` は GitHub に comment を投稿する best-effort。失敗はログのみ。
- **T-EX.post_cancel** `PostCancelComment` 同様。
- **T-EX.post_retry** `PostRetryExhaustedComment` のメッセージは `consecutive_failures` と `error_message` を含む。
- **T-EX.post_ci_limit** `PostCiFixLimitComment` は Decide が同一サイクル内で `ci_fix_count = max+1` に上げるため 1 回しか発火しない。
- **T-EX.reject_untrusted** `RejectUntrustedReadyIssue` は label 削除 → コメント投稿の順。label 削除失敗時はコメントを**スキップ**して `Ok` を返す（fail-fast 内部、外部チェーンは止めない）。
- **T-EX.spawn_init** `SpawnInit` は `InitTaskManager` に `JoinHandle` を登録する。複数 issue の init は並行起動する。DB 更新は**次サイクル Resolve** が行う。
- **T-EX.switch_impl** `SwitchToImplBranch` は checkout + pull を同期実行し、失敗するとチェーン中断（後続 `SpawnProcess` は実行されない）。
- **T-EX.spawn_process** `SpawnProcess` はスポーン前に `ProcessRun(state=running)` を INSERT し、成功時は `update_pid(child.id())` を呼ぶ。同期スポーン失敗時は即 `state=failed` に UPDATE する。
- **T-EX.cleanup_worktree** `CleanupWorktree` は worktree を削除し、成功したら `issue.worktree_path` をクリアする。
- **T-EX.close_issue** `CloseIssue` は成功時に `close_finished=true` を書き込む。既クローズ済み issue に対しても冪等に成功。
- **T-EX.order** Effects は 1 つの issue について常にグローバル優先度順で実行される。
- **T-EX.max_sessions** `SpawnProcess` は `SessionManager.count()` で並行上限を判定する（`ProcessRun.state=running` の数ではない）。上限到達時は静かに次サイクルに持ち越す。
- **T-EX.pr_idempotency** PR 作成は head branch に対する**open 状態の** PR を検索し、存在すれば再作成せず pr_number だけ取得する。closed PR は検索対象に含めない。

## 14. Application — SessionManager (`T-SM.*`)

- **T-SM.1** セッションエントリは `registered_state: State` を保持する。
- **T-SM.2** `count()` はエントリ数を返す（並行上限チェック用）。
- **T-SM.3** `collect_exited()` は終了したプロセスを `ExitedSession` として取り出す。

## 15. Application — InitTaskManager (`T-IT.*`)

- **T-IT.1** `register(issue_id, handle)` + `is_active(issue_id)` + `count()`。
- **T-IT.2** `collect_finished()` は完了済み handle のみを返し、`Ok(path)` / `Err(e)` を区別する。
- **T-IT.3** 実行中の handle は `collect_finished()` では返らない。
- **T-IT.4** `cancel(issue_id)` は handle を `abort()` して削除する。
- **T-IT.5** InitTaskManager は `max_concurrent_sessions` の対象外（SessionManager とは別系）。

## 16. Application — 起動時 orphan recovery (`T-O.*`)

- **T-O.1** 起動時に `process_runs.state='running'` のレコードを全件取得し、`mark_stale_for_issue` 等の手段で `state=failed`、`error_message='orphaned at startup'` に更新する。
- **T-O.2** PID が null の running 行も同様に failed 化される。
- **T-O.3** PID が生きているプロセスは SIGKILL を 1 回だけ送信する。

## 17. Application — `cupola cleanup` (`T-CL.*`)

- **T-CL.1** PID ファイルが有効なら `cupola is running (pid=...)` でエラー終了する。
- **T-CL.2** Cancelled の issue が 0 件なら「対象の Cancelled Issue が見つかりませんでした」を出して終了。
- **T-CL.3** Cancelled issue ごとに: worktree 削除、`cupola/{feature}/main` と `cupola/{feature}/design` を削除、open PR を close（closed/merged はスキップ）、`process_runs.pr_number` を design/impl ともにクリア、`ci_fix_count=0` にリセット。`close_finished` / `feature_name` / `consecutive_failures_epoch` は変更しない。
- **T-CL.4** worktree パスが存在しない場合は情報ログのみで続行。
- **T-CL.5** branch 削除失敗は次の issue へ進む（全体は失敗にしない）。
- **T-CL.6** `pr_number` は成否に関わらず常にクリアする。
- **T-CL.7** 標準出力は `docs/commands/cleanup.md` の例と同じ行形式。

## 18. Application — `cupola status` (`T-ST.*`)

- **T-ST.1** PID ファイルがないと `Process: not running`。
- **T-ST.2** 有効な PID ファイルは 2 行目（`foreground` / `daemon`）を読んで `Process: running (<mode>, pid=...)` を表示する。
- **T-ST.3** 死んだ PID の場合 `stale PID file cleaned` または `stale PID file exists, but cleanup failed` を出す。
- **T-ST.4** active issue が 0 件なら `No active issues.` を出す。
- **T-ST.5** active issue がある場合 `Claude sessions: <alive>` もしくは `Claude sessions: <alive>/<max>` を出す（`max_concurrent_sessions` 設定時）。
- **T-ST.6** デフォルトは `Completed` / `Cancelled` を除外する。
- **T-ST.7** `--all` フラグ指定時は終端状態も含め、Cancelled 行には `close_finished=true|false` を表示する。
- **T-ST.8** 1 行表示に GitHub issue 番号・state・design_pr・impl_pr・retry_count・worktree_path・error_message・current_pid とその alive/dead 判定が含まれる。

## 19. Application — `cupola doctor` (`T-DR.*`)

- **T-DR.1** `Start Readiness` セクションで config / git / github token / claude CLI / database をチェックする。
- **T-DR.2** `Start Readiness` に Fail が 1 件でもあるとコマンドは非 0 で終了する。
- **T-DR.3** `Operational Readiness` セクションで assets version / assets / steering / agent:ready ラベル / weight ラベルをチェックし、Warn は非 0 終了にしない。
- **T-DR.4** assets version は `.cupola/settings/.version` とバイナリ同梱バージョンを比較する。不一致時は `fix: cupola init --upgrade` を提案する。
- **T-DR.5** 表示は `✅` / `⚠️` / `❌` のいずれか。

## 20. Application — `cupola stop` (`T-SP.*`)

- **T-SP.1** PID ファイルがないと `cupola is not running` で正常終了。
- **T-SP.2** PID が死んでいる場合 `cleaned up stale PID file` を出して削除する。
- **T-SP.3** SIGTERM 送信 → 500ms ごとに存活確認。
- **T-SP.4** 30 秒以内に終了しない場合 SIGKILL にエスカレート。
- **T-SP.5** 停止確認後に PID ファイルを削除する。

## 21. Application — `cupola init` (`T-IN.*`)

- **T-IN.1** `.cupola/cupola.toml` / `.cupola/cupola.db` / `.cupola/.gitignore` を作成（存在しなければ）。
- **T-IN.2** `.claude/commands/cupola/*` と `.cupola/settings/*` を配置する。
- **T-IN.3** `.cupola/.gitignore` の内容が `docs/commands/init.md` と完全一致する。
- **T-IN.4** `--upgrade` で Cupola 管理ファイルは上書き、ユーザー所有（`cupola.toml` / `steering/*` / `specs/*`）はスキップする。
- **T-IN.5** steering bootstrap は `claude --dangerously-skip-permissions -p "..."` を呼ぶ。`claude` 未導入なら `skipped (claude not installed)` を出す。
- **T-IN.6** 再実行は idempotent。

## 22. Application — `cupola compress` (`T-CP.*`)

- **T-CP.1** specs ディレクトリがないと `specs ディレクトリが存在しません`。
- **T-CP.2** 完了済み spec が 0 件なら `完了済みの spec が見つかりません`。
- **T-CP.3** 検出対象は `phase ∈ {implementation-complete, completed}`。他の phase は無視。
- **T-CP.4** `claude --dangerously-skip-permissions -p "..."` を固定済みの spec 名リストと共に呼び出す。
- **T-CP.5** `claude` 未導入なら `skipped (claude not installed)`。

## 23. Application — `cupola logs` (`T-LG.*`)

- **T-LG.1** `.cupola/logs/` の `cupola.*` ファイルのうち辞書順で最後のものを対象にする。
- **T-LG.2** デフォルトは末尾 20 行を streaming で表示する（全量バッファしない）。
- **T-LG.3** `-f` は 200ms ごとに poll し、新しい `cupola.*` が現れたらそちらに切り替える。
- **T-LG.4** ログディレクトリがない・`cupola.*` が 1 件もない場合はエラー終了。

## 24. Adapter — GitHub (`T-GH.*`)

- **T-GH.1** `find_open_pr_by_head(branch)` は open 状態の PR 番号を返す。closed PR は返さない。
- **T-GH.2** `list_review_threads(pr)` は全スレッドを返す（trust フィルタは Collect フェーズ側で適用する）。
- **T-GH.3** `check_runs(sha)` の集約: `failure|timed_out → Failure`、`cancelled|null → Unknown`、`success|neutral|skipped` 全て → `Ok`、レコードなし → `Unknown`。
- **T-GH.4** `comment_on_issue` の呼び出しが best-effort ラッパーで包まれる前提で、ラッパー側がエラーをログ化するだけで継続する。
- **T-GH.5** `close_issue` は既クローズ済みの issue に対しても冪等に成功する。
- **T-GH.6** `remove_label` はラベル未付与の issue に対しても `Ok` を返す。

## 25. Adapter — Git worktree (`T-WT.*`)

- **T-WT.1** `fetch` + `merge` 系メソッドは conflict（exit code 1）を `MergeConflictError` として返し、fatal（exit code 2+）は `anyhow::Error` として返す。conflict は Claude に解決させる。
- **T-WT.2** `checkout` + `pull` はクリーンな worktree で連続実行可能。
- **T-WT.3** `delete_branch` は存在しないブランチに対しても `Ok` を返す（冪等）。

## 26. Adapter — Claude Code プロセス (`T-CC.*`)

- **T-CC.1** spawn された子プロセスの stdout/stderr はスレッドで非同期読み取りされ、パイプ満杯によるデッドロックが発生しない。
- **T-CC.2** structured_output JSON パーサーは `pr_title`, `pr_body`（Running 系）と `threads[]`（Fixing 系）を抽出する。
- **T-CC.3** パース失敗時はドキュメントに記載のフォールバック文字列にフォールスルーする。

## 27. 統合シナリオ (`T-IS.*`)

`tests/scenarios.rs` 配下に in-memory SQLite + mock GitHub + mock Claude runner で配置。
`docs/architecture/state-machine.md` の `## シナリオ例` を写経している。

- **T-IS.normal** Idle → … → Completed の happy path。
- **T-IS.recovery** PR が既に open な状態で再起動 → `InitializeRunning` がスマートルーティングで `DesignReviewWaiting` へ復帰。
- **T-IS.fixing_merge** DesignFixing 中にレビュワーが PR を merge → `ImplementationRunning` へ直接遷移。
- **T-IS.cancel_close** issue が closed → `Cancelled` + `PostCancelComment` + `CloseIssue`。
- **T-IS.cancel_retry** プロセスが連続失敗で retry 枯渇 → `Cancelled` + `PostRetryExhaustedComment`。
- **T-IS.reopen** `close_finished` 済みの Cancelled を reopen → `Cancelled → Idle → InitializeRunning`（別サイクル）。
- **T-IS.pr_closed** `DesignReviewWaiting` で PR が close → `DesignRunning` へ戻り、冪等性チェックを経て新 PR を作成。
- **T-IS.ci_limit** ReviewWaiting で CI 失敗を繰り返す → `ci_fix_count == max` → `PostCiFixLimitComment` → 以降は fixing なし。
- **T-IS.review_resets_ci** CI 上限到達後に trusted reviewer がスレッドを追加 → `DesignFixing` へ遷移し `ci_fix_count` リセット。
- **T-IS.orphan_recovery** 起動時に `ProcessRun.state=running` が残っている状態 → 全件 failed 化される。

---

## 実行コマンド

```bash
# 全テスト
devbox run -- cargo test

# 領域別
devbox run -- cargo test domain::
devbox run -- cargo test application::polling::
devbox run -- cargo test adapter::
devbox run -- cargo test --test scenarios
```

CI の最小合格基準: `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
`cargo test`（0 failed / 0 ignored）の 3 点がすべてクリーンであること。
