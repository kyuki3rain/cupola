# エフェクト定義

状態遷移または観測値に基づいて実行される副作用の一覧。

## 実行モデル

Decide フェーズがエフェクトリストを決定し、Execute フェーズが順次実行する。Execute はリストを受け取って実行するだけで、自ら判断しない。

### エフェクト条件の参照モデル

各エフェクトの発行条件は `prev` / `next` / `snap` のいずれかを参照する。`next` を参照するエフェクト（持続性エフェクト等）は、state 解決後の値で評価される。実装上は、state 解決の途中で確定したエフェクトを先に積む等の最適化は許容されるが、意味論としての参照先はこのテーブルが正本。

| 名前空間 | 意味 | 例 |
|---------|------|-----|
| `prev` | サイクル開始時の Issue（state + metadata） | `prev.state`, `prev.ci_fix_count`, `prev.close_finished` |
| `next` | 解決後の Issue（state + metadata_updates 適用済み） | `next.state`, `next.ci_fix_count` |
| `snap` | 外部観測値（WorldSnapshot） | `snap.design_pr.has_review_comments`, `snap.processes.design_fix.state` |

### エフェクト実行優先度

**エフェクトは Issue ごとに以下のグローバル優先度で順次実行される。** 前のエフェクトが失敗した場合、後続のエフェクトはスキップされる。best-effort なエフェクトはエラーをログのみ残して `Ok` を返し、チェーンを止めない。

```
1. 遷移時エフェクト
2. イベント時エフェクト
3. SpawnInit
4. SwitchToImplBranch → SpawnProcess
5. SpawnProcess
6. CleanupWorktree
7. CloseIssue
```

## エフェクト条件テーブル

全エフェクトの発行条件を `prev` / `next` / `snap` で表現する。このテーブルが正本。

### 遷移時エフェクト

`prev.state ≠ next.state` を前提とする。遷移が発生しない限り発行されない。

| 条件 | エフェクト | 実行方法 |
|------|-----------|---------|
| `next.state == Completed` | PostCompletedComment | best-effort |
| `next.state == Cancelled && snap.github_issue.state == closed` | PostCancelComment | best-effort |
| `next.state == Cancelled && snap.processes.*.consecutive_failures >= cfg.max_retries` | PostRetryExhaustedComment | best-effort |

### イベント時エフェクト

状態遷移の有無を問わず、条件成立時に発行される。条件が成立している間は毎サイクル発行されるため、**冪等に設計する**こと。

| 条件 | エフェクト | 実行方法 |
|------|-----------|---------|
| `snap.github_issue.has_ready_label && !snap.github_issue.ready_label_trusted` | RejectUntrustedReadyIssue | best-effort |
| `next.state ∈ {DesignReviewWaiting, ImplementationReviewWaiting} && (snap.pr.ci_status == failure ∥ snap.pr.has_conflict) && prev.ci_fix_count == cfg.max_ci_fix_cycles` | PostCiFixLimitComment | best-effort |

**PostCiFixLimitComment の一回性**: Decide は同時に `next.ci_fix_count = max+1` を metadata_updates に積み、Persist が書き込む。次サイクル以降は `prev.ci_fix_count != max_ci_fix_cycles` となるため再発行されない。トリガー条件に `== max` を使うのはこのため（`>= max` では毎サイクル発火してしまう）。PostCiFixLimitComment 自体は best-effort で、投稿失敗時も ci_fix_count は既に max+1 になっているため再試行しない。**既知の運用リスク**: GitHub 一時障害等で投稿が失敗した場合、利用者への通知なしに自動修正が停止する。

### 持続性エフェクト

`next.state` を基準に毎サイクル評価する。遷移サイクルでも発行される（遷移先の状態で評価するため、遷移直後の初回サイクルで即座に発火する）。

| 条件 | エフェクト | 実行方法 |
|------|-----------|---------|
| `next.state == InitializeRunning && snap.processes.init.state ∉ {running}` | SpawnInit | tokio::spawn |
| `next.state == DesignRunning && snap.processes.design.state ∉ {running, pending}` | SpawnProcess(Design, pending_run_id=None) | claude_runner.spawn |
| `next.state == DesignRunning && snap.processes.design.state == pending` | SpawnProcess(Design, pending_run_id=Some) | claude_runner.spawn |
| `next.state == ImplementationRunning && snap.processes.impl.state ∉ {running, pending}` | SwitchToImplBranch → SpawnProcess(Impl, pending_run_id=None) | fail-fast |
| `next.state == ImplementationRunning && snap.processes.impl.state == pending` | SpawnProcess(Impl, pending_run_id=Some) | claude_runner.spawn |
| `next.state == DesignFixing && snap.processes.design_fix.state ∉ {running, pending}` | SpawnProcess(DesignFix, pending_run_id=None) | claude_runner.spawn |
| `next.state == DesignFixing && snap.processes.design_fix.state == pending` | SpawnProcess(DesignFix, pending_run_id=Some) | claude_runner.spawn |
| `next.state == ImplementationFixing && snap.processes.impl_fix.state ∉ {running, pending}` | SpawnProcess(ImplFix, pending_run_id=None) | claude_runner.spawn |
| `next.state == ImplementationFixing && snap.processes.impl_fix.state == pending` | SpawnProcess(ImplFix, pending_run_id=Some) | claude_runner.spawn |
| `next.state == Completed && prev.worktree_path != null` | CleanupWorktree | best-effort |
| `next.state ∈ {Completed, Cancelled} && !prev.close_finished` | CloseIssue | best-effort |

**process snapshot が None の場合**: `∉ {running, pending}` に該当するため SpawnProcess を発行する。

`Completed` では CleanupWorktree を持続性エフェクトとして実行し、`worktree_path` がクリアされるまで毎サイクル再試行する。`Cancelled` では polling loop は worktree・branch を保持し、再起動時に再利用できるようにする（`cupola cleanup` コマンドを実行した場合は明示的に削除される）。

## 各エフェクトの詳細

### PostCompletedComment

| 項目 | 内容 |
|------|------|
| **条件** | `prev.state ≠ next.state && next.state == Completed` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | 完了コメントを GitHub Issue に投稿 |

---

### PostCancelComment

| 項目 | 内容 |
|------|------|
| **条件** | `prev.state ≠ next.state && next.state == Cancelled && snap.github_issue.state == closed` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | cancel コメント投稿のみ。cleanup は行わない |

---

### PostRetryExhaustedComment

| 項目 | 内容 |
|------|------|
| **条件** | `prev.state ≠ next.state && next.state == Cancelled && snap.processes.*.consecutive_failures >= cfg.max_retries` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | retry exhausted コメント投稿（連続失敗数と最後の error_message を含む）。cleanup は行わない |

---

### RejectUntrustedReadyIssue

| 項目 | 内容 |
|------|------|
| **条件** | `snap.github_issue.has_ready_label && !snap.github_issue.ready_label_trusted` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | `agent:ready` ラベルを削除し、trusted actor が必要である旨のコメントを投稿 |
| **スパム防止** | エフェクト内部で fail-fast。ラベル削除が失敗した場合はコメント投稿もスキップして Ok を返す（外部チェーンは止めない）。次サイクルでラベル削除から再試行される |

---

### PostCiFixLimitComment

| 項目 | 内容 |
|------|------|
| **条件** | `next.state ∈ {DesignReviewWaiting, ImplementationReviewWaiting} && (snap.pr.ci_status == failure ∥ snap.pr.has_conflict) && prev.ci_fix_count == cfg.max_ci_fix_cycles` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | CI/Conflict 修正上限到達コメントを投稿 |
| **メタデータ更新** | なし（`next.ci_fix_count = max+1` は Persist が担当） |

---

### SpawnInit

| 項目 | 内容 |
|------|------|
| **条件** | `next.state == InitializeRunning && snap.processes.init.state ∉ {running}` |
| **実行方法** | `tokio::spawn` |
| **処理内容** | ProcessRun(type=init) INSERT → fetch / worktree 作成 / `cupola/{feature_name}/main` branch 作成・push / `cupola/{feature_name}/design` branch 作成・push / `.cupola/specs/{feature_name}/` 作成（`spec.json` + `requirements.md` 雛形生成）|
| **完了検知** | `JoinHandle` を `InitTaskManager` で管理。次サイクル Resolve で結果を取り出す |
| **メタデータ更新** | Resolve が担当。成功時に `worktree_path` をセット、ProcessRun.state=succeeded |
| **失敗時** | Resolve が ProcessRun.state=failed を保存 → 次サイクル Collect が `processes.init.state==failed` を観測 → Decide が retry フローへ |

---

### SwitchToImplBranch

| 項目 | 内容 |
|------|------|
| **条件** | `next.state == ImplementationRunning && snap.processes.impl.state ∉ {running, pending}`（SpawnProcess の前処理として順次実行） |
| **実行方法** | 同期実行（fail-fast） |
| **処理内容** | worktree で `cupola/{feature_name}/main` branch に checkout → pull |
| **失敗時** | チェーン中断。次サイクルで再試行 |

**既知の運用リスク**: SwitchToImplBranch の失敗（権限不足・ネットワーク障害・ロック競合等）は ProcessRun を生成しないため `consecutive_failures` にカウントされず、`max_retries` による Cancelled 遷移が発動しない。継続失敗時は `ImplementationRunning` で無限に再試行し続けるため、環境側の問題解決が必要。

---

### SpawnProcess

| 項目 | 内容 |
|------|------|
| **条件** | 持続性エフェクト条件テーブル参照 |
| **実行方法** | 同期実行（claude_runner.spawn） |
| **パラメータ** | `type_: ProcessRunType`、`causes: Vec<FixingProblemKind>`、`pending_run_id: Option<i64>`。`snap.processes.*.state == pending` の場合に `pending_run_id = Some(run_id)` をセットする |
| **前提メタデータ** | `worktree_path`、`weight`、`causes`（Decide が WorldSnapshot から毎回決定。直前の ProcessRun の causes はあくまで監査用記録） |
| **処理内容（新規: pending_run_id=None）** | セッション枠チェック → 枠あり: ProcessRun INSERT（state=running） + input files 生成 + モデル選択 + Claude Code 起動。枠なし: ProcessRun INSERT（state=pending）で次サイクルに持ち越す |
| **処理内容（再試行: pending_run_id=Some）** | セッション枠チェック → 枠あり: 既存レコードを state=running に UPDATE + input files 生成 + モデル選択 + Claude Code 起動。枠なし: pending のまま維持 |
| **Fixing 時の追加処理** | fetch + merge `origin/{default_branch}` を実行。ローカルのマージコンフリクトは Claude Code が自力で解決する |
| **完了検知** | SessionManager で終了検知 → Resolve が ProcessRun を更新 → 次サイクル Collect が `processes.*.state` を観測 |

---

### CleanupWorktree

| 項目 | 内容 |
|------|------|
| **条件** | `next.state == Completed && prev.worktree_path != null` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | worktree cleanup を実行 |
| **メタデータ更新** | cleanup 成功時に `worktree_path` をクリア。以降サイクルでは条件不成立となり再発行されない |

---

### CloseIssue

| 項目 | 内容 |
|------|------|
| **条件** | `next.state ∈ {Completed, Cancelled} && !prev.close_finished` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | GitHub Issue をクローズ |
| **メタデータ更新** | 成功時に `close_finished = true` をセット（Completed・Cancelled 両方） |

cupola が GitHub Issue を**能動的にクローズする**責務（GitHub close API 呼び出し）は **CloseIssue のみ**が担当する。人手クローズは Collect が `snap.github_issue.state == closed` として観測し Decide に渡すが、API 呼び出し自体は行わない。コメント投稿（遷移時・イベント時）と Issue クローズ（持続性）は独立した責務であり、両者は同一サイクル内で並立して実行される。

`close_finished = true` になると次サイクル以降は CloseIssue が発行されない。GitHub 上で既にクローズ済みの場合も API 呼び出しは冪等に成功する。

**前提要件**: cupola の bot アカウントは GitHub Issue をクローズする権限を持つこと。権限不足で CloseIssue が恒常的に失敗すると `close_finished` が永遠に `false` のままとなり、`Cancelled → Idle` 遷移が発生しなくなる。

---

## エフェクト実行例

Decide 欄の effects リストおよび Execute 欄の実行順序はグローバル優先度リストに従う。順序の正本はグローバル優先度リスト。

### InitializeRunning（正常・初回）

```
Resolve:  なし
Collect:  snap.processes.init = None
Decide:   next.state = InitializeRunning（遷移なし）
          エフェクト評価: next.state == InitializeRunning && snap.processes.init = None → [SpawnInit]
Persist:  変更なし
Execute:  SpawnInit 発火（tokio::spawn で init タスク起動）
```

### DesignRunning（ProcessRun 失敗リトライ）

```
Resolve:  snap.processes.design.state = failed に UPDATE
Collect:  snap.processes.design = { state: failed, consecutive_failures: 1 }
Decide:   next.state = DesignRunning（遷移なし）
          エフェクト評価: next.state == DesignRunning && snap.processes.design.state == failed → [SpawnProcess]
Persist:  変更なし
Execute:  SpawnProcess 発火（新しい ProcessRun を INSERT してスポーン）
```

### DesignReviewWaiting → DesignFixing（レビューコメントあり）

```
Resolve:  なし
Collect:  snap.design_pr.has_review_comments = true、snap.processes.design_fix = { state: succeeded }（前回の fix）
Decide:   next.state = DesignFixing（遷移あり）
          エフェクト評価: next.state == DesignFixing && snap.processes.design_fix.state == succeeded（∉ {running, pending}） → [SpawnProcess]
Persist:  state = DesignFixing
Execute:  SpawnProcess 発火（新しい ProcessRun を INSERT してスポーン）
```

### DesignReviewWaiting（CiFixExhausted）

```
Resolve:  なし
Collect:  snap.design_pr.ci_status = failure、snap.ci_fix_exhausted = true（ci_fix_count == max_ci_fix_cycles）
Decide:   next.state = DesignReviewWaiting（遷移なし）、next.ci_fix_count = max+1
          エフェクト評価: prev.ci_fix_count == max_ci_fix_cycles → [PostCiFixLimitComment]
Persist:  ci_fix_count = max+1
Execute:  PostCiFixLimitComment 投稿（best-effort）
```

### ImplementationReviewWaiting → Completed

```
Resolve:  なし
Collect:  snap.impl_pr.state = merged
Decide:   next.state = Completed（遷移あり）
          エフェクト評価: prev.state ≠ next.state → [PostCompletedComment]
          next.state == Completed && prev.worktree_path != null → [CleanupWorktree]
          next.state ∈ {Completed} && !prev.close_finished → [CloseIssue]
Persist:  state = Completed
Execute:  PostCompletedComment 実行 → CleanupWorktree → CloseIssue
```

### Any → Cancelled（Issue クローズ）

```
Resolve:  なし（または現在の ProcessRun.state を failed に UPDATE）
Collect:  snap.github_issue.state = closed
Decide:   next.state = Cancelled（遷移あり）
          エフェクト評価: prev.state ≠ next.state && snap.github_issue.state == closed → [PostCancelComment]
          next.state ∈ {Cancelled} && !prev.close_finished → [CloseIssue]
Persist:  state = Cancelled
Execute:  PostCancelComment 投稿（best-effort） → CloseIssue
          ※ worktree・branch は保持
```

### Any → Cancelled（リトライ枯渇）

```
Resolve:  ProcessRun.state = failed に UPDATE
Collect:  snap.processes.*.consecutive_failures >= max_retries
Decide:   next.state = Cancelled（遷移あり）
          エフェクト評価: prev.state ≠ next.state && snap.processes.*.consecutive_failures >= max_retries → [PostRetryExhaustedComment]
          next.state ∈ {Cancelled} && !prev.close_finished → [CloseIssue]
Persist:  state = Cancelled
Execute:  PostRetryExhaustedComment 投稿 → CloseIssue
          ※ worktree・branch は保持（再起動時に再利用可能）
```
