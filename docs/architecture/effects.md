# エフェクト定義

状態遷移または状態チェックに伴って実行される副作用の一覧。

## 実行モデル

Apply フェーズはシグナルから次の状態と実行すべきエフェクト配列を決定する。エフェクトの実行は全て Execute フェーズが担当する。

**エフェクトは Issue ごとに以下のグローバル優先度で順次実行される。** 前のエフェクトが失敗した場合、後続のエフェクトはスキップされる。best-effort なエフェクトはエラーをログのみ残して `Ok` を返し、チェーンを止めない。

`Completed` では cleanup を実行する。`Cancelled` では worktree・branch を保持し、再起動時に再利用できるようにする。

## グローバル優先度

```
1. 遷移トリガーエフェクト  （Apply がキュー積み）
2. Initialize              （状態トリガー）
3. SwitchToImplBranch      （状態トリガー）
4. SpawnProcess            （状態トリガー）
5. CloseIssue              （状態トリガー）
```

## トリガーの種類

- **遷移トリガー**: Apply が状態遷移（または `—`）時にエフェクトキューへ積む
- **状態トリガー**: Execute が毎サイクル、現在の状態・シグナル・メタデータを見て判定する

状態トリガーは、トリガー条件が成立している限り毎サイクル再試行される。失敗しても次サイクルで自動的に再実行される。

## 実行方法

| 実行方法 | 意味 |
|---------|------|
| `tokio::spawn` | バックグラウンドで実行。完了は次サイクル Resolve で検知 |
| 同期実行 | Execute フェーズ内で await。失敗したらチェーン中断 |
| 同期実行（best-effort） | Execute フェーズ内で await。失敗してもログのみで継続 |

---

## 遷移トリガーエフェクト

### PostComment（遷移時コメント）

各遷移に対応するコメントを GitHub Issue に投稿する。全て best-effort。

| トリガー | コメント内容 |
|---------|------------|
| `→ Completed` | "all_completed" |
| `→ Cancelled`（`IssueClosedObserved`） | "cancelled" |
| `→ Cancelled`（`RetryExhausted`） | "retry_exhausted"（`retry_count` と `error_message` を含む） |
| `CiFixExhausted` で遷移なし（`—`） | CI/Conflict 修正上限到達 |

**メタデータ更新（PostCiFixLimitComment のみ）**: `ci_fix_count = max_ci_fix_cycles + 1`（再投稿防止）

---

## 状態トリガーエフェクト

### Initialize

| 項目 | 内容 |
|------|------|
| **トリガー** | `InitializeRunning` かつ `initialized = false` かつ init handle なし |
| **実行方法** | tokio::spawn |
| **処理内容** | fetch / worktree 作成 / `cupola/{n}/main` branch 作成・push / `cupola/{n}/design` branch 作成・push |
| **完了検知** | `JoinHandle` を `InitTaskManager` で管理。次サイクル Resolve で結果を取り出す |
| **メタデータ更新** | Resolve が担当。成功時に `worktree_path` と `initialized = true` を DB に書く |
| **失敗時** | Resolve が `last_process_exit = Failed` を保存 → 次サイクル Collect が `ProcessFailed` を生成 → retry フローへ |

---

### SwitchToImplBranch

| 項目 | 内容 |
|------|------|
| **トリガー** | `ImplementationRunning` かつ `current_pid = null`（SpawnProcess の前処理として順次実行） |
| **実行方法** | 同期実行（fail-fast） |
| **処理内容** | worktree で `cupola/{n}/main` branch に checkout → pull |
| **メタデータ更新** | なし（`ci_fix_count = 0` は Apply の遷移時に書く） |
| **失敗時** | チェーン中断。次サイクルで再試行 |

---

### SpawnProcess

| 項目 | 内容 |
|------|------|
| **トリガー** | プロセス実行中状態（`DesignRunning`、`DesignFixing`、`ImplementationRunning`、`ImplementationFixing`）かつ `current_pid = null` かつ `CiFixExhausted` シグナルなし |
| **実行方法** | 同期実行（claude_runner.spawn） |
| **前提メタデータ** | `worktree_path`、`weight`、`fixing_causes`（Fixing 時） |
| **処理内容** | input files 生成（`.cupola/inputs/`）→ モデル選択（`weight × state-key`）→ Claude Code 起動 |
| **Fixing 時の追加処理** | fetch + merge `origin/{default_branch}` を実行。ローカルのマージコンフリクトは Claude Code が自力で解決する |
| **メタデータ更新** | `current_pid` をセット |
| **完了検知** | SessionManager で終了検知 → Resolve が `last_process_exit` を保存 → 次サイクル Collect が `ProcessSucceeded` / `ProcessFailed` を生成 |

---

### CloseIssue

| 項目 | 内容 |
|------|------|
| **トリガー** | `Completed` または `Cancelled` かつ `IssueOpenObserved` シグナルあり |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | GitHub Issue をクローズ |

---

### PostCompletedComment

| 項目 | 内容 |
|------|------|
| **トリガー** | `→ Completed` 遷移時 |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | 完了コメントを投稿 |

---

### CleanupWorktree

| 項目 | 内容 |
|------|------|
| **トリガー** | `Completed` かつ `worktree_path != null` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | worktree cleanup を実行 |
| **メタデータ更新** | cleanup 成功時に `worktree_path` をクリア |

---

### RejectUntrustedReadyIssue

| 項目 | 内容 |
|------|------|
| **トリガー** | `UntrustedReadyIssueObserved` |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | `agent:ready` ラベルを削除し、trusted actor が必要である旨のコメントを投稿 |

---

### CancelWithoutCleanup

| 項目 | 内容 |
|------|------|
| **トリガー** | `IssueClosedObserved` による `→ Cancelled` 遷移時 |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | cancel コメント投稿のみ。cleanup は行わない |

---

### CancelRetryExhausted

| 項目 | 内容 |
|------|------|
| **トリガー** | `RetryExhausted` による `→ Cancelled` 遷移時 |
| **実行方法** | 同期実行（best-effort） |
| **処理内容** | retry exhausted コメント投稿のみ。cleanup は行わない |

---

## エフェクトキューの例

### InitializeRunning（正常・初回）

```
Resolve:  なし
Collect:  なし（initialized = false、handle なし）
Apply:    遷移なし、キュー = []
Execute:  状態トリガー判定
  → Initialize 発火（tokio::spawn で init タスク起動）
```

### DesignRunning（ProcessFailed リトライ）

```
Resolve:  ProcessFailed 検知 → retry_count++、current_pid クリア
Collect:  ProcessFailed シグナル生成
Apply:    DesignRunning → DesignRunning（遷移なし）、キュー = []
Execute:  状態トリガー判定
  → SpawnProcess 発火（DesignRunning + current_pid = null + CiFixExhausted なし）
```

### DesignReviewWaiting（CiFixExhausted）

```
Resolve:  なし
Collect:  DesignCiFailureFound + CiFixExhausted シグナル生成
Apply:    — 行マッチ（遷移なし）、キュー = [PostComment(ci_fix_limit)]
Execute:  遷移トリガー実行
  → PostComment 投稿（best-effort）、ci_fix_count = max+1
  状態トリガー判定
  → SpawnProcess: CiFixExhausted シグナルあり → スキップ
```

### ImplementationReviewWaiting → Completed

```
Resolve:  なし
Collect:  ImplPrMerged シグナル生成
Apply:    → Completed 遷移、キュー = [PostCompletedComment]
Execute:  遷移トリガー実行
  → PostCompletedComment 実行
  状態トリガー判定
  → CleanupWorktree（worktree_path があれば）
  → CloseIssue（IssueOpenObserved あり → GitHub Issue をクローズ）
```

### Any → Cancelled（IssueClosedObserved）

```
Resolve:  なし（または current_pid クリア）
Collect:  IssueClosedObserved シグナル生成
Apply:    → Cancelled 遷移、キュー = [PostComment(cancelled)]
Execute:  遷移トリガー実行
  → PostComment 投稿（best-effort）
  状態トリガー判定
  → CloseIssue: IssueOpenObserved なし（すでに closed）→ スキップ
  ※ worktree・branch は保持
```

### Any → Cancelled（RetryExhausted）

```
Resolve:  ProcessFailed 検知 → retry_count++（= max）、current_pid クリア
Collect:  ProcessFailed + RetryExhausted シグナル生成
Apply:    → Cancelled 遷移、キュー = [PostComment(retry_exhausted)]
Execute:  遷移トリガー実行
  → PostComment 投稿（retry_count と error_message を含む）（best-effort）
  状態トリガー判定
  → CloseIssue（IssueOpenObserved あり → GitHub Issue をクローズ）
  ※ worktree・branch は保持（再起動時に再利用可能）
```
