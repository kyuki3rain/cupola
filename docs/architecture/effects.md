# エフェクト定義

状態遷移または観測値に基づいて実行される副作用の一覧。

## 実行モデル

Decide フェーズがエフェクトリストを決定し、Execute フェーズが順次実行する。Execute はリストを受け取って実行するだけで、自ら判断しない。

**エフェクトは Issue ごとに以下のグローバル優先度で順次実行される。** 前のエフェクトが失敗した場合、後続のエフェクトはスキップされる。best-effort なエフェクトはエラーをログのみ残して `Ok` を返し、チェーンを止めない。

`Completed` では CleanupWorktree を持続性エフェクトとして実行し、`worktree_path` がクリアされるまで毎サイクル再試行する。`Cancelled` では polling loop は worktree・branch を保持し、再起動時に再利用できるようにする（`cupola cleanup` コマンドを実行した場合は明示的に削除される）。

## グローバル優先度

```
1. 遷移時エフェクト   （Decide が遷移検出時にキュー積み）
2. イベント時エフェクト（Decide が観測条件成立時にキュー積み）
3. SpawnInit         （持続性）
4. SwitchToImplBranch → SpawnProcess  （持続性）
5. SpawnProcess      （持続性）
6. CleanupWorktree   （持続性）
7. CloseIssue        （持続性）
```

## エフェクトの種類

### 遷移時エフェクト

Decide が `prev_state → next_state` の遷移を検出した時のみ発行される。遷移が発生しない限り再発行されない。

| 遷移条件 | エフェクト | 実行方法 |
|---------|-----------|---------|
| `→ Completed` | PostCompletedComment | 同期（best-effort） |
| `→ Cancelled`（github_issue.state == closed 経由） | PostCancelComment | 同期（best-effort） |
| `→ Cancelled`（consecutive_failures >= max_retries 経由） | PostRetryExhaustedComment | 同期（best-effort） |

### イベント時エフェクト

Decide が特定の観測条件を検出した時に発行される。状態遷移の有無を問わない。条件が成立している間は毎サイクル発行されるため、**冪等に設計する**こと。

| 観測条件 | エフェクト | 実行方法 |
|---------|-----------|---------|
| `github_issue.has_ready_label && !ready_label_trusted` | RejectUntrustedReadyIssue | 同期（best-effort） |
| `(DesignReviewWaiting \| ImplementationReviewWaiting)` かつ `(ci_status == failure \| has_conflict)` かつ `ci_fix_count == max_ci_fix_cycles` | PostCiFixLimitComment | 同期（best-effort） |

**PostCiFixLimitComment の一回性**: Decide は同時に `ci_fix_count = max+1` を metadata_updates に積み、Persist が書き込む。次サイクル以降は `ci_fix_count != max_ci_fix_cycles` となるため再発行されない。トリガー条件に `ci_fix_exhausted`（`>= max`）ではなく `== max` を使うのはこのため（`> max` でも `ci_fix_exhausted = true` のままなので `>=` では毎サイクル発火してしまう）。PostCiFixLimitComment 自体は best-effort で、投稿失敗時も ci_fix_count は既に max+1 になっているため再試行しない。**既知の運用リスク**: GitHub 一時障害等で投稿が失敗した場合、利用者への通知なしに自動修正が停止する。

### 持続性エフェクト

Decide が WorldSnapshot を見て毎サイクル判断する。条件が成立している限り毎サイクル発行される。

cupola が GitHub Issue を**能動的にクローズする**責務（GitHub close API 呼び出し）は **CloseIssue のみ**が担当する。人手クローズは Collect が `github_issue.state == closed` として観測し Decide に渡すが、API 呼び出し自体は行わない。コメント投稿（遷移時・イベント時）と Issue クローズ（持続性）は独立した責務であり、両者は同一サイクル内で並立して実行される。

| 条件 | エフェクト | 実行方法 |
|------|-----------|---------|
| `InitializeRunning` + `processes.init` が None または `state != running` + init 未完了 | SpawnInit | `tokio::spawn` |
| `ImplementationRunning` + `processes.impl.state != running` | SwitchToImplBranch → SpawnProcess（順次） | 同期（fail-fast） |
| `DesignRunning` + `processes.design.state ∉ {running, pending}` | SpawnProcess（pending_run_id=None） | 同期（claude_runner.spawn） |
| `DesignRunning` + `processes.design.state == pending` | SpawnProcess（pending_run_id=Some） | 同期（claude_runner.spawn） |
| Fixing 状態 + 対応 `process.state ∉ {running, pending}` | SpawnProcess（pending_run_id=None） | 同期（claude_runner.spawn） |
| Fixing 状態 + 対応 `process.state == pending` | SpawnProcess（pending_run_id=Some） | 同期（claude_runner.spawn） |
| `Completed` + `worktree_path != null` | CleanupWorktree | 同期（best-effort） |
| `Completed` または `Cancelled` + `!close_finished` | CloseIssue | 同期（best-effort） |

持続性エフェクトは失敗しても次サイクルで自動的に再実行される。

## 実行方法

| 実行方法 | 意味 |
|---------|------|
| `tokio::spawn` | バックグラウンドで実行。完了は次サイクル Resolve で検知 |
| 同期実行 | Execute フェーズ内で await。失敗したらチェーン中断 |
| 同期実行（best-effort） | Execute フェーズ内で await。失敗してもログのみで継続 |

---

## 各エフェクトの詳細

### PostCompletedComment

| 項目 | 内容 |
|------|------|
| **トリガー** | `→ Completed` 遷移時 |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | 完了コメントを GitHub Issue に投稿 |

---

### CleanupWorktree

| 項目 | 内容 |
|------|------|
| **トリガー** | `Completed` かつ `worktree_path != null`（持続性。クリアされるまで毎サイクル実行） |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | worktree cleanup を実行 |
| **メタデータ更新** | cleanup 成功時に `worktree_path` をクリア。以降サイクルでは条件不成立となり再発行されない |

---

### PostCancelComment

| 項目 | 内容 |
|------|------|
| **トリガー** | `github_issue.state == closed` 経由で `→ Cancelled` 遷移時 |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | cancel コメント投稿のみ。cleanup は行わない |

---

### PostRetryExhaustedComment

| 項目 | 内容 |
|------|------|
| **トリガー** | `consecutive_failures >= max_retries` 経由で `→ Cancelled` 遷移時 |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | retry exhausted コメント投稿（連続失敗数と最後の error_message を含む）。cleanup は行わない |

---

### RejectUntrustedReadyIssue

| 項目 | 内容 |
|------|------|
| **トリガー** | `github_issue.has_ready_label && !ready_label_trusted` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | `agent:ready` ラベルを削除し、trusted actor が必要である旨のコメントを投稿 |
| **スパム防止** | エフェクト内部で fail-fast。ラベル削除が失敗した場合はコメント投稿もスキップして Ok を返す（外部チェーンは止めない）。次サイクルでラベル削除から再試行される |

---

### PostCiFixLimitComment

| 項目 | 内容 |
|------|------|
| **トリガー** | `(DesignReviewWaiting \| ImplementationReviewWaiting)` かつ `(ci_status == failure \| has_conflict)` かつ `ci_fix_count == max_ci_fix_cycles` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | CI/Conflict 修正上限到達コメントを投稿 |
| **メタデータ更新** | なし（`ci_fix_count = max+1` は Persist が担当） |

---

### SpawnInit

| 項目 | 内容 |
|------|------|
| **トリガー** | `InitializeRunning` かつ init ProcessRun が存在しないまたは `state != running` かつ init 未完了 |
| **実行方法** | `tokio::spawn` |
| **処理内容** | ProcessRun(type=init) INSERT → fetch / worktree 作成 / `cupola/{feature_name}/main` branch 作成・push / `cupola/{feature_name}/design` branch 作成・push / `.cupola/specs/{feature_name}/` 作成（`spec.json` + `requirements.md` 雛形生成）|
| **完了検知** | `JoinHandle` を `InitTaskManager` で管理。次サイクル Resolve で結果を取り出す |
| **メタデータ更新** | Resolve が担当。成功時に `worktree_path` をセット、ProcessRun.state=succeeded |
| **失敗時** | Resolve が ProcessRun.state=failed を保存 → 次サイクル Collect が `processes.init.state==failed` を観測 → Decide が retry フローへ |

---

### SwitchToImplBranch

| 項目 | 内容 |
|------|------|
| **トリガー** | `ImplementationRunning` かつ `processes.impl.state != running`（SpawnProcess の前処理として順次実行） |
| **実行方法** | 同期実行（fail-fast） |
| **処理内容** | worktree で `cupola/{feature_name}/main` branch に checkout → pull |
| **失敗時** | チェーン中断。次サイクルで再試行 |

**既知の運用リスク**: SwitchToImplBranch の失敗（権限不足・ネットワーク障害・ロック競合等）は ProcessRun を生成しないため `consecutive_failures` にカウントされず、`max_retries` による Cancelled 遷移が発動しない。継続失敗時は `ImplementationRunning` で無限に再試行し続けるため、環境側の問題解決が必要。

---

### SpawnProcess

| 項目 | 内容 |
|------|------|
| **トリガー** | `DesignRunning`・`DesignFixing`・`ImplementationFixing` かつ対応 `process.state != running`。`ImplementationRunning` は SwitchToImplBranch チェーン内で実行されるため対象外。`ci_fix_exhausted` はチェックしない（ReviewWaiting の入口で制御済みのため） |
| **実行方法** | 同期実行（claude_runner.spawn） |
| **パラメータ** | `type_: ProcessRunType`、`causes: Vec<FixingProblemKind>`、`pending_run_id: Option<i64>`。Decide は最新 ProcessSnapshot の state が `pending` の場合に `pending_run_id = Some(run_id)` をセットする |
| **前提メタデータ** | `worktree_path`、`weight`、`causes`（Decide が WorldSnapshot から毎回決定。直前の ProcessRun の causes はあくまで監査用記録） |
| **処理内容（新規: pending_run_id=None）** | セッション枠チェック → 枠あり: ProcessRun INSERT（state=running） + input files 生成 + モデル選択 + Claude Code 起動。枠なし: ProcessRun INSERT（state=pending）で次サイクルに持ち越す |
| **処理内容（再試行: pending_run_id=Some）** | セッション枠チェック → 枠あり: 既存レコードを state=running に UPDATE + input files 生成 + モデル選択 + Claude Code 起動。枠なし: pending のまま維持 |
| **Fixing 時の追加処理** | fetch + merge `origin/{default_branch}` を実行。ローカルのマージコンフリクトは Claude Code が自力で解決する |
| **完了検知** | SessionManager で終了検知 → Resolve が ProcessRun を更新 → 次サイクル Collect が `processes.*.state` を観測 |

---

### CloseIssue

| 項目 | 内容 |
|------|------|
| **トリガー** | `Completed` または `Cancelled` かつ `!close_finished` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | GitHub Issue をクローズ |
| **メタデータ更新** | 成功時に `close_finished = true` をセット（Completed・Cancelled 両方） |

`close_finished = true` になると次サイクル以降は CloseIssue が発行されない。GitHub 上で既にクローズ済みの場合も API 呼び出しは冪等に成功する。

**前提要件**: cupola の bot アカウントは GitHub Issue をクローズする権限を持つこと。権限不足で CloseIssue が恒常的に失敗すると `close_finished` が永遠に `false` のままとなり、`Cancelled → Idle` 遷移が発生しなくなる。

---

## エフェクト実行例

Decide 欄の effects リストおよび Execute 欄の実行順序はグローバル優先度リストに従う。順序の正本はグローバル優先度リスト。

### InitializeRunning（正常・初回）

```
Resolve:  なし
Collect:  processes.init = None
Decide:   遷移なし、effects = [SpawnInit]
Persist:  変更なし
Execute:  SpawnInit 発火（tokio::spawn で init タスク起動）
```

### DesignRunning（ProcessRun 失敗リトライ）

```
Resolve:  processes.design.state = failed に UPDATE
Collect:  processes.design = { state: failed, consecutive_failures: 1 }
Decide:   DesignRunning のまま（遷移なし）、effects = [SpawnProcess]
Persist:  変更なし
Execute:  SpawnProcess 発火（新しい ProcessRun を INSERT してスポーン）
```

### DesignReviewWaiting（CiFixExhausted）

```
Resolve:  なし
Collect:  design_pr.ci_status = failure、ci_fix_exhausted = true（ci_fix_count == max_ci_fix_cycles）
Decide:   — 行マッチ（遷移なし）、metadata_updates = { ci_fix_count: max+1 }、effects = [PostCiFixLimitComment]
Persist:  ci_fix_count = max+1
Execute:  PostCiFixLimitComment 投稿（best-effort。失敗時も ci_fix_count は既に max+1 のため再試行なし）
          SpawnProcess: DesignReviewWaiting は Running/Fixing 状態ではないためスキップ
```

### ImplementationReviewWaiting → Completed

```
Resolve:  なし
Collect:  impl_pr.state = merged
Decide:   → Completed 遷移、effects = [PostCompletedComment, CleanupWorktree, CloseIssue]
          （CleanupWorktree は持続性エフェクト。next_state=Completed + worktree_path != null の条件で同サイクル内に発火）
Persist:  state = Completed
Execute:  PostCompletedComment 実行
          CleanupWorktree（worktree_path があれば）
          CloseIssue（!close_finished → 実行、成功時に close_finished = true をセット）
```

### Any → Cancelled（Issue クローズ）

```
Resolve:  なし（または現在の ProcessRun.state を failed に UPDATE）
Collect:  github_issue.state = closed
Decide:   → Cancelled 遷移、effects = [PostCancelComment, CloseIssue]
Persist:  state = Cancelled
Execute:  PostCancelComment 投稿（best-effort）
          CloseIssue（!close_finished → 実行。既クローズ済みでも API 冪等に成功、close_finished = true をセット）
          ※ worktree・branch は保持
```

### Any → Cancelled（リトライ枯渇）

```
Resolve:  ProcessRun.state = failed に UPDATE
Collect:  processes.*.consecutive_failures >= max_retries
Decide:   → Cancelled 遷移、effects = [PostRetryExhaustedComment, CloseIssue]
Persist:  state = Cancelled
Execute:  PostRetryExhaustedComment 投稿（consecutive_failures と error_message を含む）
          CloseIssue（!close_finished → 実行、成功時に close_finished = true をセット）
          ※ worktree・branch は保持（再起動時に再利用可能）
```
