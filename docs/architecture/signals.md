# シグナル定義

各シグナルは原子的な情報単位。生成元はフェーズによって2種類ある。

- **Resolve 生成**: なし。Resolve は DB / runtime state を更新するだけで、シグナル化は行わない
- **Collect 生成**: GitHub API・DB の純粋観測から生成

Apply フェーズはこれら全シグナルの和集合を受け取って遷移を決定する。

## シグナル一覧

### `ReadyIssueObserved`

- **観測元**: GitHub Issues API
- **条件**: `agent:ready` ラベルが付いた open な Issue が存在し、ラベル付与者が trusted actor である
- **チェック対象状態**: `Idle`、`Cancelled`
- **trusted 判定**: ラベル付与者の author association が `Owner` / `Member` / `Collaborator` のいずれか（`TrustedAssociations::All` 設定時はスキップ）

---

### `UntrustedReadyIssueObserved`

- **観測元**: GitHub Issues API
- **条件**: `agent:ready` ラベルが付いた open な Issue が存在するが、ラベル付与者が非 trusted actor である
- **チェック対象状態**: `Idle`、`Cancelled`
- **用途**: `RejectUntrustedReadyIssue` effect を発火し、ラベル除去と警告コメント投稿を行う

---

### `IssueOpenObserved`

- **生成元**: Collect フェーズ（GitHub Issues API）
- **条件**: Issue の state が `open`
- **チェック対象状態**: 全状態
- **発行頻度**: 毎サイクル観測・発行される。Issue がクローズされるまで発行され続ける
- **用途**: 主に Execute フェーズの CloseIssue トリガー。全状態で観測することで、同サイクルで `Completed` / `Cancelled` に遷移した直後も CloseIssue を発火できる

---

### `IssueClosedObserved`

- **観測元**: GitHub Issues API
- **条件**: Issue の state が `closed`
- **チェック対象状態**: 全ての非終端状態

---

### `InitializationSucceeded`

- **生成元**: Collect フェーズ（DB の `initialized` 観測）
- **条件**: `initialized = true`
- **チェック対象状態**: `InitializeRunning`

---

### `ProcessSucceeded`

- **生成元**: Collect フェーズ（DB の `last_process_exit` 観測）
- **条件**: `last_process_exit = Succeeded`
- **チェック対象状態**: `InitializeRunning` 以外のプロセス実行中状態（`DesignRunning`、`DesignFixing`、`ImplementationRunning`、`ImplementationFixing`）

---

### `ProcessFailed`

- **生成元**: Collect フェーズ（DB の `last_process_exit` 観測）
- **条件**: `last_process_exit = Failed`
- **チェック対象状態**: `InitializeRunning` を含むプロセス実行中の全状態
- **備考**: `retry_count` をインクリメントした上で発行。`RetryExhausted` と同時に発行されることがある

---

### `RetryExhausted`

- **生成元**: Resolve フェーズ（内部カウンタ）
- **前提メタデータ**: `retry_count`（インクリメント後の値で判定）
- **条件**: `ProcessFailed` 発行時に `retry_count >= max_retries`
- **備考**: 単独では発行されず、必ず `ProcessFailed` と同時に発行される

---

### `WeightHeavyObserved`

- **観測元**: GitHub Issues API
- **条件**: Issue ラベルに `weight:heavy` が付いている
- **チェック対象状態**: 全状態

---

### `WeightLightObserved`

- **観測元**: GitHub Issues API
- **条件**: Issue ラベルに `weight:light` が付いている
- **チェック対象状態**: 全状態

---

### `WeightMediumObserved`

- **観測元**: GitHub Issues API
- **条件**: `weight:heavy` / `weight:light` のどちらも付いていない
- **チェック対象状態**: 全状態

---

### `DesignPrOpen`

- **観測元**: GitHub Pull Requests API
- **前提メタデータ**: `design_pr_number` が設定済み（null の場合は観測不可）
- **条件**: PR が open（未マージ）
- **チェック対象状態**: `InitializeRunning`、`DesignRunning`（プロセス終了時）

---

### `DesignPrMerged`

- **観測元**: GitHub Pull Requests API
- **前提メタデータ**: `design_pr_number` が設定済み（null の場合は観測不可）
- **条件**: PR がマージ済み
- **チェック対象状態**: `InitializeRunning`、`DesignReviewWaiting`、`DesignFixing`

---

### `ImplPrOpen`

- **観測元**: GitHub Pull Requests API
- **前提メタデータ**: `impl_pr_number` が設定済み（null の場合は観測不可）
- **条件**: PR が open（未マージ）
- **チェック対象状態**: `InitializeRunning`、`ImplementationRunning`（プロセス終了時）

---

### `ImplPrMerged`

- **観測元**: GitHub Pull Requests API
- **前提メタデータ**: `impl_pr_number` が設定済み（null の場合は観測不可）
- **条件**: PR がマージ済み
- **チェック対象状態**: `InitializeRunning`、`ImplementationReviewWaiting`、`ImplementationFixing`

---

### `DesignReviewCommentFound`

- **観測元**: GitHub Pull Requests API（Review Threads）
- **前提メタデータ**: `design_pr_number` が設定済み（null の場合は観測不可）
- **条件**: unresolved な review thread が存在する（trusted actor のコメントのみ対象）
- **trusted 判定**: コメント投稿者の author association を確認し、非 trusted の場合はそのスレッドを無視する

---

### `DesignCiFailureFound`

- **観測元**: GitHub Check Runs API
- **前提メタデータ**: `design_pr_number` が設定済み（null の場合は観測不可）
- **条件**: CI check-run の conclusion が `failure`
- **備考**: `ci_fix_count >= max_ci_fix_cycles` の場合は `CiFixExhausted` と同時に発行される

---

### `DesignConflictFound`

- **観測元**: GitHub Pull Requests API（`mergeable` フィールド）
- **前提メタデータ**: `design_pr_number` が設定済み（null の場合は観測不可）
- **条件**: PR の `mergeable` が `false`（`null` の場合は GitHub が計算中のため観測不可）
- **備考**: `ci_fix_count >= max_ci_fix_cycles` の場合は `CiFixExhausted` と同時に発行される

---

### `ImplReviewCommentFound`

- **観測元**: GitHub Pull Requests API（Review Threads）
- **前提メタデータ**: `impl_pr_number` が設定済み（null の場合は観測不可）
- **条件・trusted 判定**: `DesignReviewCommentFound` と同一

---

### `ImplCiFailureFound`

- **観測元**: GitHub Check Runs API
- **前提メタデータ**: `impl_pr_number` が設定済み（null の場合は観測不可）
- **条件・備考**: `DesignCiFailureFound` と同一

---

### `ImplConflictFound`

- **観測元**: GitHub Pull Requests API（`mergeable` フィールド）
- **前提メタデータ**: `impl_pr_number` が設定済み（null の場合は観測不可）
- **条件・備考**: `DesignConflictFound` と同一

---

### `CiFixExhausted`

- **観測元**: 内部カウンタ
- **前提メタデータ**: `ci_fix_count`
- **条件**: `DesignCiFailureFound` / `DesignConflictFound` / `ImplCiFailureFound` / `ImplConflictFound` 発行時に `ci_fix_count >= max_ci_fix_cycles`
- **備考**: 単独では発行されず、必ず上記シグナルと同時に発行される。上限到達時（`ci_fix_count == max_ci_fix_cycles`）に1回のみコメントを投稿する
