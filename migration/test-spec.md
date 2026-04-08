# Architecture Migration — Exhaustive Test Specification

This file is the **single source of truth for all tests** to be written before
implementation. Every implementation phase must achieve "all tests in this
document green" before being considered done.

Sections map 1:1 onto the cargo test modules that must exist after migration.

Notation: `T-<phase>.<group>.<n>` is a test ID; use as the rust `#[test] fn` name.

---

## 0. Layer / file map

| Test module | Source under test |
|---|---|
| `domain::state::tests` | `domain/state.rs` |
| `domain::process_run::tests` | `domain/process_run.rs` |
| `domain::issue::tests` | `domain/issue.rs` |
| `domain::world_snapshot::tests` | `domain/world_snapshot.rs` |
| `domain::effect::tests` | `domain/effect.rs` |
| `domain::decide::tests` | `domain/decide.rs` |
| `domain::metadata_update::tests` | `domain/metadata_update.rs` |
| `application::polling::resolve::tests` | `application/polling/resolve.rs` |
| `application::polling::collect::tests` | `application/polling/collect.rs` |
| `application::polling::persist::tests` | `application/polling/persist.rs` |
| `application::polling::execute::tests` | `application/polling/execute.rs` |
| `application::session_manager::tests` | `application/session_manager.rs` |
| `application::init_task_manager::tests` | `application/init_task_manager.rs` |
| `application::cleanup_use_case::tests` | `application/cleanup_use_case.rs` |
| `application::status_use_case::tests` | new |
| `application::doctor_use_case::tests` | `application/doctor_use_case.rs` |
| `application::stop_use_case::tests` | `application/stop_use_case.rs` |
| `application::start_use_case::tests` | new |
| `application::init_use_case::tests` | `application/init_use_case.rs` |
| `application::compress_use_case::tests` | `application/compress_use_case.rs` |
| `adapter::outbound::sqlite_process_run_repository::tests` | new |
| `adapter::outbound::sqlite_issue_repository::tests` | existing |
| `adapter::outbound::github_*::tests` | existing |
| `tests/scenarios/` | integration |

---

## 1. Domain — State (`T-1.S.*`)

- **T-1.S.1** `State::InitializeRunning` exists; the variant is named exactly `InitializeRunning` (no `Initialized`).
- **T-1.S.2** `State::is_terminal` returns true for `Completed` and `Cancelled`, false for the other 8.
- **T-1.S.3** `State::is_review_waiting` returns true only for `DesignReviewWaiting` / `ImplementationReviewWaiting`.
- **T-1.S.4** `State` round-trips through `Display` / `FromStr` via the snake_case names used in DB (`idle`, `initialize_running`, …).
- **T-1.S.5** `State` has exactly 10 variants; serializing every variant produces the canonical snake_case key listed in docs/data-model.md.

---

## 2. Domain — ProcessRun (`T-1.P.*`)

- **T-1.P.1** `ProcessRunType` has exactly 5 variants: `Init`, `Design`, `DesignFix`, `Impl`, `ImplFix`. snake_case (`init|design|design_fix|impl|impl_fix`) round-trip.
- **T-1.P.2** `ProcessRunState` has exactly 4 variants: `Running`, `Succeeded`, `Failed`, `Stale`. snake_case round-trip.
- **T-1.P.3** `ProcessRun` struct holds `id, issue_id, type, index, state, pid, pr_number, causes, error_message, started_at, finished_at` — exactly these fields, no more.
- **T-1.P.4** `ProcessRun::new_running(...)` constructor returns `state=Running, pid=None, pr_number=None, causes=...` and asserts `finished_at.is_none()`.

---

## 3. Domain — Issue (`T-1.I.*`)

- **T-1.I.1** `Issue` struct holds **only**: `id, github_issue_number, state, feature_name, weight, worktree_path, ci_fix_count, close_finished, consecutive_failures_epoch, created_at, updated_at`. Fields removed: `design_pr_number, impl_pr_number, retry_count, current_pid, error_message, fixing_causes`.
- **T-1.I.2** Default-constructed Issue has `state=Idle, ci_fix_count=0, close_finished=false, consecutive_failures_epoch=None`.

---

## 4. Domain — WorldSnapshot (`T-1.W.*`)

- **T-1.W.1** `WorldSnapshot` holds `github_issue, design_pr, impl_pr, processes, ci_fix_exhausted` — exactly these fields.
- **T-1.W.2** `GithubIssueSnapshot` holds `state, has_ready_label, ready_label_trusted, weight`.
- **T-1.W.3** `PrSnapshot` holds `state ∈ {Open,Merged,Closed}, has_review_comments, ci_status ∈ {Ok,Failure,Unknown}, has_conflict`.
- **T-1.W.4** `ProcessesSnapshot` holds `init, design, design_fix, impl_, impl_fix : Option<ProcessSnapshot>`.
- **T-1.W.5** `ProcessSnapshot` holds `state, index, consecutive_failures: u32`.
- **T-1.W.6** Builders compile and a fixture `world_snapshot::fixtures::idle()` returns a sane default for use in decide tests.

---

## 5. Domain — Effect (`T-1.E.*`)

- **T-1.E.1** `Effect` enum has exactly the following 11 variants (matching docs/architecture/effects.md):
  `PostCompletedComment, PostCancelComment, PostRetryExhaustedComment, RejectUntrustedReadyIssue, PostCiFixLimitComment, SpawnInit, SwitchToImplBranch, SpawnProcess { type_: ProcessRunType, causes: Vec<FixingProblemKind> }, CleanupWorktree, CloseIssue`.
  (SwitchToImplBranch and SpawnProcess are emitted as separate effects but Execute always runs them as a chain in priority order.)
- **T-1.E.2** `Effect::priority()` returns u8 ordering: transition < event < SpawnInit < SwitchToImplBranch < SpawnProcess < CleanupWorktree < CloseIssue. Sorting a Vec<Effect> by priority produces the docs-defined global order.
- **T-1.E.3** `Effect::is_best_effort()` is true for `PostCompletedComment, PostCancelComment, PostRetryExhaustedComment, RejectUntrustedReadyIssue, PostCiFixLimitComment, CleanupWorktree, CloseIssue` and false for `SpawnInit, SwitchToImplBranch, SpawnProcess`.

---

## 6. Domain — Decide (`T-1.D.*`)

The pure function `decide(prev: &Issue, snap: &WorldSnapshot, cfg: &Config) -> Decision`.

Each row of the **全遷移テーブル** in `docs/architecture/state-machine.md` is one test. They are listed below, grouped by `From` state. Test IDs are stable and must be used as the rust `fn` name.

### Idle (T-1.D.idle.*)
- **T-1.D.idle.1** `has_ready_label && ready_label_trusted` → `InitializeRunning`.
- **T-1.D.idle.2** `has_ready_label && !ready_label_trusted` → stay Idle, effect `RejectUntrustedReadyIssue` emitted.
- **T-1.D.idle.3** No ready label → stay Idle, no effects.

### InitializeRunning (T-1.D.init.*)
- **T-1.D.init.1** `github_issue.state == closed` → `Cancelled`, effect `PostCancelComment`.
- **T-1.D.init.2** `processes.init.consecutive_failures >= max_retries` → `Cancelled`, effect `PostRetryExhaustedComment`.
- **T-1.D.init.3** `processes.init.state == failed` (under cap) → stay, effect `SpawnInit`.
- **T-1.D.init.4** `init.succeeded + impl_pr.merged` → `Completed`, effects `PostCompletedComment + CleanupWorktree + CloseIssue`.
- **T-1.D.init.5** `init.succeeded + impl_pr.open` → `ImplementationReviewWaiting`.
- **T-1.D.init.6** `init.succeeded + design_pr.merged` → `ImplementationRunning`, ci_fix_count reset.
- **T-1.D.init.7** `init.succeeded + design_pr.open` → `DesignReviewWaiting`.
- **T-1.D.init.8** `init.succeeded` (no PR) → `DesignRunning`.
- **T-1.D.init.9** `init = None` → stay, effect `SpawnInit`.
- **T-1.D.init.10** Smart routing priority: closed-state row beats every other row.

### DesignRunning (T-1.D.dr.*)
- **T-1.D.dr.1** `issue.closed` → `Cancelled` + `PostCancelComment`.
- **T-1.D.dr.2** `processes.design.consecutive_failures >= max_retries` → `Cancelled` + `PostRetryExhaustedComment`.
- **T-1.D.dr.3** `design.failed` (under cap) → stay, effect `SpawnProcess(design)`.
- **T-1.D.dr.4** `design.stale` → stay, effect `SpawnProcess(design)`.
- **T-1.D.dr.5** `design.succeeded + design_pr.merged` → `ImplementationRunning`, ci_fix_count reset.
- **T-1.D.dr.6** `design.succeeded + design_pr.closed` → stay (re-create) + `SpawnProcess(design)` + ci_fix_count reset.
- **T-1.D.dr.7** `design.succeeded + design_pr.open` → `DesignReviewWaiting`.
- **T-1.D.dr.8** No process record → effect `SpawnProcess(design)`.

### DesignReviewWaiting (T-1.D.drw.*)
- **T-1.D.drw.1** `issue.closed` → `Cancelled` + `PostCancelComment`.
- **T-1.D.drw.2** `design_pr.merged` → `ImplementationRunning` + ci_fix_count reset.
- **T-1.D.drw.3** `design_pr.closed` → `DesignRunning` + ci_fix_count reset.
- **T-1.D.drw.4** `has_review_comments` → `DesignFixing` + ci_fix_count **reset to 0** + causes contains `ReviewComments`.
- **T-1.D.drw.5** `has_review_comments + ci_failure` → `DesignFixing` + ci_fix_count reset (review wins reset, but causes contains both).
- **T-1.D.drw.6** `ci_status.failure + ci_fix_exhausted` → stay, effect `PostCiFixLimitComment`, `metadata.ci_fix_count = max+1`.
- **T-1.D.drw.7** `ci_status.failure` (under cap) → `DesignFixing` + ci_fix_count incremented + causes `CiFailure`.
- **T-1.D.drw.8** `has_conflict + ci_fix_exhausted` → stay, `PostCiFixLimitComment`, ci_fix_count = max+1.
- **T-1.D.drw.9** `has_conflict` (under cap) → `DesignFixing` + ci_fix_count incremented + causes `Conflict`.
- **T-1.D.drw.10** `design_pr == None` → `DesignRunning`.
- **T-1.D.drw.11** Priority: merge beats close beats review beats CI beats conflict.
- **T-1.D.drw.12** PostCiFixLimitComment one-shot: when `ci_fix_count == max+1`, no PostCiFixLimitComment is emitted (because trigger is `== max`, not `>=`).

### DesignFixing (T-1.D.df.*)
- **T-1.D.df.1** `issue.closed` → `Cancelled` + `PostCancelComment`.
- **T-1.D.df.2** `design_pr.merged` → `ImplementationRunning` + ci_fix_count reset.
- **T-1.D.df.3** `design_pr.closed` → `DesignRunning` + ci_fix_count reset.
- **T-1.D.df.4** `design_pr == None` → `DesignRunning`.
- **T-1.D.df.5** `design_fix.consecutive_failures >= max_retries` → `Cancelled` + `PostRetryExhaustedComment`.
- **T-1.D.df.6** `design_fix.failed` (under cap) → stay, effect `SpawnProcess(design_fix, causes)`.
- **T-1.D.df.7** `design_fix.stale` → stay, effect `SpawnProcess(design_fix, causes)`.
- **T-1.D.df.8** `design_fix.succeeded` → `DesignReviewWaiting`.
- **T-1.D.df.9** No process record → effect `SpawnProcess(design_fix, causes)`.
- **T-1.D.df.10** Decide re-derives `causes` from WorldSnapshot every cycle (independent of stored causes).

### ImplementationRunning (T-1.D.ir.*)
- **T-1.D.ir.1**–**T-1.D.ir.8** mirror DesignRunning rows but with `impl` / `impl_pr` and target `Completed` instead of `ImplementationRunning`.
- **T-1.D.ir.9** Effect chain: when SpawnProcess is emitted, the prefix `SwitchToImplBranch` is also emitted and ordered before SpawnProcess.

### ImplementationReviewWaiting (T-1.D.irw.*)
- Mirror DesignReviewWaiting (12 rows). Merge target = `Completed`.
- **T-1.D.irw.merge** `impl_pr.merged` → `Completed` + `PostCompletedComment` + `CleanupWorktree` + `CloseIssue` + ci_fix_count reset.

### ImplementationFixing (T-1.D.if.*)
- Mirror DesignFixing (10 rows). Merge target = `Completed`.

### Completed (T-1.D.c.*)
- **T-1.D.c.1** stays Completed.
- **T-1.D.c.2** `worktree_path != null` → `CleanupWorktree` emitted (持続性).
- **T-1.D.c.3** `!close_finished` → `CloseIssue` emitted.
- **T-1.D.c.4** `worktree_path == null && close_finished` → no effects.

### Cancelled (T-1.D.x.*)
- **T-1.D.x.1** `close_finished && github_issue.state == open` → `Idle` + `metadata.consecutive_failures_epoch = now` + `metadata.close_finished = false`.
- **T-1.D.x.2** `!close_finished` → stay, effect `CloseIssue`.
- **T-1.D.x.3** `close_finished && github_issue.state == closed` → stay, no effects.
- **T-1.D.x.4** Cancelled → Idle does **not** reset ci_fix_count.

### Cross-cutting (T-1.D.X.*)
- **T-1.D.X.1** `weight` is propagated into metadata_updates whenever `snap.github_issue.weight != prev.weight`.
- **T-1.D.X.2** Effects are returned in global priority order regardless of insertion order.
- **T-1.D.X.3** `Decide` performs no I/O (verified by giving it borrowed-only inputs and Send + 'static bound).
- **T-1.D.X.4** PostCiFixLimitComment + ci_fix_count=max+1 are produced **together** in metadata_updates.

---

## 7. Domain — MetadataUpdates (`T-1.M.*`)

- **T-1.M.1** struct `MetadataUpdates { state, weight, ci_fix_count, close_finished, feature_name, consecutive_failures_epoch, worktree_path }` with all `Option<T>` (None = leave unchanged).
- **T-1.M.2** `MetadataUpdates::default()` is all-None.
- **T-1.M.3** `apply_to(&mut Issue)` only mutates fields whose update is `Some`.

---

## 8. Application — ProcessRunRepository (`T-2.R.*`)

- **T-2.R.1** `insert_running` returns row id, sets `state=running`, `index = max(index)+1` for `(issue_id, type)`.
- **T-2.R.2** `update_pid(id, pid)` sets pid only.
- **T-2.R.3** `mark_succeeded(id, pr_number=Some(_))` sets `state=succeeded, finished_at=now, pr_number`.
- **T-2.R.4** `mark_failed(id, error)` sets `state=failed, finished_at=now, pid=None, error_message`.
- **T-2.R.5** `mark_stale(id)` sets `state=stale, finished_at=now, pid=None`.
- **T-2.R.6** `latest(issue_id, type)` returns the highest-index row.
- **T-2.R.7** `consecutive_failures(issue_id, type, epoch)` walks index DESC, counts contiguous `failed`, stops on `succeeded|stale|running`, ignores rows with `started_at < epoch`.
- **T-2.R.8** `find_orphans()` returns all rows with `state=running` (across all issues).
- **T-2.R.9** `clear_pr_number(issue_id, type)` for all rows of that type.

---

## 9. Application — IssueRepository schema (`T-2.IR.*`)

- **T-2.IR.1** Migrating an existing schema from old → new is idempotent (running twice yields the same final schema).
- **T-2.IR.2** `find_active()` excludes `Completed` and `Cancelled`.
- **T-2.IR.3** `find_all()` includes `Completed` and `Cancelled`.
- **T-2.IR.4** `update_state_and_metadata(issue_id, MetadataUpdates)` writes only non-None fields.
- **T-2.IR.5** `consecutive_failures_epoch` round-trips Utc datetime and `None`.
- **T-2.IR.6** `close_finished` round-trips bool.

---

## 10. Application — Resolve (`T-3.RS.*`)

- **T-3.RS.1** Finished Claude Code session whose `registered_state == current_state` and exit=0:
  - design Running → ProcessRun set succeeded with `pr_number` (after PR creation).
- **T-3.RS.2** Finished session, `registered_state != current_state`: ProcessRun set `state=stale`, no PR creation, no thread reply.
- **T-3.RS.3** Non-zero exit → `state=failed`, `error_message` populated.
- **T-3.RS.4** Stall timeout exceeded → SIGKILL sent; resulting termination handled as failure.
- **T-3.RS.5** Init JoinHandle finished Ok → `process_runs(type=init).state=succeeded` and Issue.worktree_path set.
- **T-3.RS.6** Init JoinHandle finished Err → `state=failed` with error.
- **T-3.RS.7** PR creation post-processing failure → `state=failed` (not succeeded), pr_number not written.
- **T-3.RS.8** Fixing process success → reply/resolve threads via GitHub; failure of reply marks `failed`.

---

## 11. Application — Collect (`T-3.CO.*`)

- **T-3.CO.1** Builds WorldSnapshot for each active issue.
- **T-3.CO.2** PR snapshot only when `ProcessRun(type=design).pr_number` is `Some` (uses latest such row).
- **T-3.CO.3** PR API 404 → snapshot is `None`.
- **T-3.CO.4** PR API 5xx → returns Err for that issue (not panic, not silent skip).
- **T-3.CO.5** `consecutive_failures` aggregation respects `consecutive_failures_epoch`.
- **T-3.CO.6** `ci_fix_exhausted = (issue.ci_fix_count >= cfg.max_ci_fix_cycles)`.
- **T-3.CO.7** `has_review_comments` excludes non-trusted thread authors.
- **T-3.CO.8** `ci_status` maps cancelled→Unknown, failure/timed_out→Failure, success→Ok.
- **T-3.CO.9** `has_conflict` true iff `mergeable == false`; null→false.
- **T-3.CO.10** Collect performs zero DB writes (verified via mock that panics on write).

---

## 12. Application — Persist (`T-3.PE.*`)

- **T-3.PE.1** Writes only `MetadataUpdates`-marked fields.
- **T-3.PE.2** When `next_state != prev_state`, also updates `state` and `updated_at`.
- **T-3.PE.3** When `next_state == prev_state`, does not bump `updated_at` unless other fields change.

---

## 13. Application — Execute (`T-3.EX.*`)

For each Effect:
- **T-3.EX.PostCompleted** posts comment via GitHub mock; best-effort (failure logged but execution continues).
- **T-3.EX.PostCancel** likewise.
- **T-3.EX.PostRetry** message contains `consecutive_failures` and `error_message`.
- **T-3.EX.PostCiLimit** posted exactly once because Decide flips `ci_fix_count = max+1` same cycle.
- **T-3.EX.RejectUntrusted** removes label, then comments; if label removal fails, comment is **skipped**, returns Ok.
- **T-3.EX.SpawnInit** registers a JoinHandle in InitTaskManager; concurrent for multiple issues.
- **T-3.EX.SwitchImpl** runs checkout+pull synchronously; on failure, downstream `SpawnProcess` is **skipped** (fail-fast chain).
- **T-3.EX.SpawnProcess** INSERTs ProcessRun(state=running) before spawn; updates pid on success; updates state=failed on synchronous spawn error.
- **T-3.EX.CleanupWorktree** clears `worktree_path` on success.
- **T-3.EX.CloseIssue** sets `close_finished=true` on success; idempotent for already-closed.
- **T-3.EX.Order** Effects executed in global priority order for a given issue.
- **T-3.EX.MaxSessions** SpawnProcess respects `max_concurrent_sessions` based on SessionManager entry count (not ProcessRun.state=running count). Excess deferred silently.
- **T-3.EX.PrIdempotency** PR creation queries open PRs by head branch and reuses existing pr_number rather than re-creating; closed PRs are NOT reused.

---

## 14. Application — SessionManager (`T-3.SM.*`)

- **T-3.SM.1** Holds `registered_state` per session entry.
- **T-3.SM.2** `count()` returns entry count (used for max_concurrent_sessions check).
- **T-3.SM.3** `take_finished()` returns sessions whose process exited.

---

## 15. Application — InitTaskManager (`T-3.IT.*`)

- **T-3.IT.1** Stores `HashMap<issue_id, JoinHandle<Result<()>>>`.
- **T-3.IT.2** `take_finished()` returns finished JoinHandles for each issue.
- **T-3.IT.3** No concurrency cap (separate from SessionManager).

---

## 16. Application — start_use_case orphan recovery (`T-5.O.*`)

- **T-5.O.1** On startup, all `process_runs.state='running'` are SIGKILLed and updated to `state='failed'`, `error_message='orphaned'`.
- **T-5.O.2** PID=null running rows are still updated to failed.
- **T-5.O.3** PID-alive process gets killed exactly once.

---

## 17. Application — cleanup_use_case (`T-6.CL.*`)

- **T-6.CL.1** Errors with "cupola is running (pid=…)" if PID file is valid.
- **T-6.CL.2** No-op message when no Cancelled issues.
- **T-6.CL.3** Per Cancelled issue: removes worktree, deletes both branches, closes open PRs (skips closed/merged), clears `process_runs.pr_number` for design+impl, resets `ci_fix_count=0`, leaves `close_finished` and `feature_name` and `consecutive_failures_epoch` untouched.
- **T-6.CL.4** worktree path absent → continues with success log.
- **T-6.CL.5** Branch delete failure → continues to next issue.
- **T-6.CL.6** Open PR close failure → continues, but pr_number is still cleared on success path; on failure pr_number cleared anyway? Per docs, pr_number is cleared regardless. Test enforces "always cleared".
- **T-6.CL.7** Output lines match docs example format.

---

## 18. Application — status_use_case (`T-6.ST.*`)

- **T-6.ST.1** `Process: not running` when no PID file.
- **T-6.ST.2** `Process: running (foreground|daemon, pid=…)` reads PID file second line.
- **T-6.ST.3** Stale PID → message + cleanup attempted.
- **T-6.ST.4** No active issues → `No active issues.`.
- **T-6.ST.5** Active issues → `Claude sessions: a` or `a/b` if max set.
- **T-6.ST.6** Default excludes Completed/Cancelled.
- **T-6.ST.7** `--all` includes them and Cancelled rows show `close_finished=true|false`.
- **T-6.ST.8** Issue line contains GitHub#, state, design_pr, impl_pr, retry, worktree, error, current_pid, alive/dead.

---

## 19. Application — doctor_use_case (`T-6.DR.*`)

- **T-6.DR.1** Section "Start Readiness" runs config/git/github_token/claude/database checks.
- **T-6.DR.2** Any Fail in Start Readiness → exit non-zero.
- **T-6.DR.3** Section "Operational Readiness" runs assets_version/assets/steering/labels checks; warns do not fail.
- **T-6.DR.4** assets_version compares `.cupola/settings/.version` with embedded version; mismatch suggests `cupola init --upgrade`.
- **T-6.DR.5** Output uses `✅ / ⚠️ / ❌` symbols.

---

## 20. Application — stop_use_case (`T-6.SP.*`)

- **T-6.SP.1** Missing PID file → `cupola is not running`, exit 0.
- **T-6.SP.2** Stale PID → `cleaned up stale PID file`.
- **T-6.SP.3** SIGTERM sent first, polled every 500ms.
- **T-6.SP.4** Escalates to SIGKILL after 30s.
- **T-6.SP.5** PID file removed after termination.

---

## 21. Application — init_use_case (`T-6.IN.*`)

- **T-6.IN.1** Creates `.cupola/cupola.toml`, `.cupola/cupola.db`, `.cupola/.gitignore` (only if missing).
- **T-6.IN.2** Installs `.claude/commands/cupola/*` and `.cupola/settings/*`.
- **T-6.IN.3** `.cupola/.gitignore` content matches docs verbatim.
- **T-6.IN.4** `--upgrade` overwrites managed files but skips user-owned (`cupola.toml`, `steering/*`, `specs/*`).
- **T-6.IN.5** steering bootstrap invokes claude with the documented prompt; `claude` missing → `skipped (claude not installed)`.
- **T-6.IN.6** Re-running init is idempotent.

---

## 22. Application — compress_use_case (`T-6.CO.*`)

- **T-6.CO.1** No specs dir → `specs ディレクトリが存在しません`.
- **T-6.CO.2** No completed specs → `完了済みの spec が見つかりません`.
- **T-6.CO.3** Detects phase ∈ {`implementation-complete`, `completed`} and ignores others.
- **T-6.CO.4** Invokes `claude --dangerously-skip-permissions -p "…"` with documented prompt and the locked spec list.
- **T-6.CO.5** `claude` missing → `skipped (claude not installed)`.

---

## 23. Application — logs (`T-6.LG.*`)

- **T-6.LG.1** Picks the lexicographically last `cupola.*` file in `.cupola/logs/`.
- **T-6.LG.2** Default mode prints last 20 lines (streamed, no full-file buffer).
- **T-6.LG.3** `-f` follow mode polls every 200ms and rolls over to a newer `cupola.*` file.
- **T-6.LG.4** Errors when log dir missing or no `cupola.*` file present.

---

## 24. Adapter — GitHub (`T-4.GH.*`)

- **T-4.GH.1** `find_open_pr_by_head(branch)` returns `Some(pr_number)` when an open PR exists, `None` otherwise; explicitly excludes closed PRs.
- **T-4.GH.2** `list_review_threads(pr)` returns trusted-only threads filtered by `author_association ∈ trusted_associations`.
- **T-4.GH.3** `check_runs(sha)` aggregates conclusions; `failure|timed_out → Failure`, `cancelled|null → Unknown`, all-success → Ok.
- **T-4.GH.4** `comment_on_issue` is idempotent at the API call boundary (best-effort wrapper handles errors).
- **T-4.GH.5** `close_issue` is idempotent on already-closed.
- **T-4.GH.6** `remove_label` returns Ok even when label is not present.

---

## 25. Adapter — Worktree (`T-4.WT.*`)

- **T-4.WT.1** `fetch_and_merge(default_branch)` runs git fetch + git merge; on conflict returns Ok (merge state left for Claude to resolve).
- **T-4.WT.2** `checkout(branch)` followed by `pull` works on clean worktree.
- **T-4.WT.3** `delete_branch(name)` is idempotent on missing branch.

---

## 26. Adapter — claude_code_process (`T-4.CC.*`)

- **T-4.CC.1** Spawn produces stdout/stderr readers that don't block on full pipe buffers.
- **T-4.CC.2** structured_output JSON parser extracts `pr_title`, `pr_body` (design/impl) and `threads[]` (fixing).
- **T-4.CC.3** Parse failure falls back to documented default strings.

---

## 27. Integration scenarios (`T-7.IT.*`)

Each scenario lives under `tests/scenarios/` and uses in-memory SQLite + mock GitHub + mock Claude runner. They mirror docs `## シナリオ例`.

- **T-7.IT.normal** Idle → … → Completed (full happy path).
- **T-7.IT.recovery** Restart mid-flow with PR already open → InitializeRunning smart-routes to DesignReviewWaiting.
- **T-7.IT.fixing_merge** DesignFixing while reviewer merges → ImplementationRunning.
- **T-7.IT.cancel_close** Issue closed mid-flow → Cancelled + PostCancelComment + CloseIssue.
- **T-7.IT.cancel_retry** Process keeps failing → Cancelled + PostRetryExhaustedComment.
- **T-7.IT.reopen** Cancelled + close_finished + Issue reopened → Idle → InitializeRunning (next cycle, not same cycle).
- **T-7.IT.pr_closed** PR closed (not merged) in DesignReviewWaiting → DesignRunning → new PR created with idempotency check.
- **T-7.IT.ci_limit** ReviewWaiting + repeated CI failure → ci_fix_count = max → PostCiFixLimitComment → no further fixing.
- **T-7.IT.review_resets_ci** CI count maxed, then trusted reviewer adds comment → DesignFixing + ci_fix_count reset.
- **T-7.IT.orphan_recovery** Startup with running process_runs → all marked failed.

---

## Test execution gates

- Phase 1 done: `cargo test domain::` green.
- Phase 2 done: `cargo test domain:: adapter::outbound::sqlite_` green.
- Phase 3 done: `cargo test application::polling:: application::session_manager:: application::init_task_manager::` green.
- Phase 4 done: `cargo test adapter::` green.
- Phase 5 done: `cargo test application::start_use_case` green.
- Phase 6 done: `cargo test application::cleanup_use_case application::status application::doctor application::stop application::init application::compress` green.
- Phase 7 done: `cargo test` (all) + `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check`.

Each phase MUST be green before the next phase begins.
