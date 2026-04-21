# 調査ログ

---
**Feature**: `issue-320` — Issue 本文の改変検知 (hash 固定)
**Discovery Scope**: Extension（既存のポーリング実行パイプラインへの拡張）
**Key Findings**:
  - `prepare_init_handle` と `prepare_process_spawn` の両関数がすでに `github.get_issue()` を呼び出している。ハッシュ計算の自然な挿入点がすでに存在する
  - `MetadataUpdates` / `update_state_and_metadata` のスパース更新パターンが確立済みであり、`body_hash` の追加はそのまま踏襲できる
  - `run_add_column_migration` パターンにより後方互換マイグレーションが標準化されている
  - `RejectUntrustedReadyIssue` 効果の実装が「GitHub API 副作用 → Ok(()) 返却」パターンの先例として存在する

---

## 調査ログ

### 既存の実行パイプライン

- **Context**: SpawnInit と SpawnProcess のどこにハッシュ計算を挿入するか調査
- **Findings**:
  - `prepare_init_handle`（`execute.rs` 約 400 行目）: `github.get_issue()` を呼び出し `detail.body` を `issue_body` として使用。この直後がハッシュ保存の挿入点
  - `prepare_process_spawn`（`execute.rs` 約 614 行目）: `github.get_issue()` を呼び出し `detail` を取得。この直後がハッシュ比較の挿入点
  - `execute_one` の `Effect::SpawnProcess` ブランチ: `spawn_process` → `prepare_process_spawn` のエラーを受け取る。`BodyTamperedError` のダウンキャストはここで実施
- **Implications**: 新規の GitHub API 呼び出しは不要。既存の `get_issue()` 呼び出しに便乗できる

### MetadataUpdates / IssueRepository パターン

- **Context**: body_hash の保存方法を確定するための調査
- **Findings**:
  - `MetadataUpdates` は `Option<Option<T>>` パターンで nullable フィールドのスパース更新を表現（例: `worktree_path: Option<Option<String>>`）
  - `update_state_and_metadata` は動的 SQL でパラメータを組み立てており、新フィールド追加が容易
  - `persist_decision` は `MetadataUpdates` の全フィールドが `None` の場合に DB 書き込みをスキップする早期リターンを持つ
- **Implications**: `body_hash: Option<Option<String>>` を `MetadataUpdates` に追加するだけで永続化フローに組み込める。`persist.rs` の早期リターン条件にも `body_hash.is_none()` を追加する必要あり

### SQLite マイグレーションパターン

- **Context**: 既存 DB への後方互換追加方法を確認
- **Findings**:
  - `run_add_column_migration` ヘルパーが `"duplicate column name"` エラーを無視し冪等性を保証
  - `last_pr_review_submitted_at` の追加時も同パターンを使用
  - `CREATE TABLE IF NOT EXISTS` には含めず、migration ブロックでのみ追加
  - SELECT クエリでは末尾に列を追加し、`row_to_issue` で列インデックス 12 として取得
- **Implications**: `run_add_column_migration(&conn, "body_hash TEXT")` を追加するだけで対応可能

### SHA-256 ライブラリ選定

- **Context**: Cargo.toml に現在 SHA-256 ライブラリが存在しないため選定が必要
- **Findings**:
  - 現在の依存: `sha2` も `ring` も `hex` も存在しない
  - Rust エコシステムの標準: `sha2 = "0.10"` (RustCrypto プロジェクト)
  - hex エンコード: `format!("{:x}", digest)` で追加クレートなしに対応可能
  - `sha2::Sha256::digest(data.as_bytes())` → `format!("{:x}", result)` のパターンで完結
- **Implications**: `Cargo.toml` に `sha2 = "0.10"` のみ追加。`hex` クレートは不要

### エラー型と応答処理の設計

- **Context**: `BodyTamperedError` を検知した後の処理フローの設計
- **Findings**:
  - `execute_one` は `issue: &mut Issue` を保持するため、インメモリ `Issue.state` の更新が可能
  - `execute_one` は `github`, `issue_repo` 等のポートへのアクセスを持つ
  - `MergeConflictError` の先例: `prepare_process_spawn` 内でダウンキャストして best-effort 処理
  - `RejectUntrustedReadyIssue` の先例: label 削除 → コメント → `Ok(())` のパターン
  - `BodyTamperedError` は `prepare_process_spawn` から返し、`execute_one` の `Effect::SpawnProcess` ブランチでキャッチする設計が最もクリーン
- **Implications**: `spawn_process` のシグネチャ変更は最小限。`execute_one` での `downcast_ref::<BodyTamperedError>()` によるキャッチで対応

### 再 approve フローの確認

- **Context**: Cancelled 状態からの復帰フローが既存で対応可能かを確認
- **Findings**:
  - `prepare_init_handle` の `is_resume` フラグ: `worktree.exists(&wt_path)` で既存 worktree を検出し、resume コメントを投稿するパスが存在する
  - これは Cancelled → 再ラベル付与 → InitializeRunning → SpawnInit のフローを示している
  - SpawnInit は body_hash を毎回上書きするため、再 approve 時には自然に新しいハッシュが保存される
- **Implications**: 再 approve フロー用の特別な実装は不要。SpawnInit でのハッシュ保存（要件 2）を正しく実装すれば、要件 5 は自動的に満たされる

---

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク / 制限 | 採否 |
|-----------|------|------|---------------|------|
| 案 A: Execute 相でのインライン検証 | `prepare_process_spawn` でハッシュを比較し、`execute_one` でエラーをキャッチして応答 | 既存の実行フローを最小限に変更、`execute_one` が全ポートへのアクセスを持つ | execute.rs の責務が少し増える | **採用** |
| 案 B: Decide 相に新 Effect を追加 | `WorldSnapshot` に改変フラグを持たせ、Decide が `DetectBodyTamper` Effect を生成 | 関心分離がクリーン | Decide は副作用なしの pure function のはずが、現実には GitHub API 呼び出し前に本文を取得できない | **不採用** |
| 案 C: 専用の Port trait を追加 | `BodyHashPort` trait を定義してハッシュ管理を抽象化 | テスタビリティ向上 | 過剰設計。既存の `IssueRepository` で十分 | **不採用** |

---

## 設計上の決定

### Decision: BodyTamperedError のスコープ

- **Context**: `BodyTamperedError` をどのレイヤーに配置するか
- **Alternatives Considered**:
  1. `domain/error.rs` に配置
  2. `application/polling/execute.rs` 内にプライベートに定義
- **Selected Approach**: `application/polling/execute.rs` 内に定義（プライベートまたは pub(crate)）
- **Rationale**: ドメイン層は純粋ロジックのみを持ち、外部からのエラー型（GitHub fetch エラーに関連するもの）を含めるべきでない。エラーは execute 相の実装詳細
- **Trade-offs**: execute.rs が若干肥大するが、テストは同ファイル内で容易に書ける

### Decision: ハッシュ保存タイミング

- **Context**: `prepare_init_handle` の戻り値変更 vs. 別途 API 呼び出し
- **Alternatives Considered**:
  1. `prepare_init_handle` が `(JoinHandle, String)` を返し、呼び出し元 `spawn_init_task` が保存
  2. `prepare_init_handle` 内で直接 `issue_repo` を呼び出して保存
- **Selected Approach**: Option 1 — 戻り値でハッシュを返し、`spawn_init_task` が保存
- **Rationale**: `prepare_init_handle` は「IO の準備」関数として定義されており、DB 書き込みを追加するより戻り値を拡張する方が責務が明確

---

## リスクと緩和策

- `last_pr_review_submitted_at` と同様に `persist.rs` の早期リターン条件への追加漏れ → タスク 1.3 として明示的に対応
- `row_to_issue` の列インデックスずれ → `body_hash` を末尾（インデックス 12）に追加し、テストでラウンドトリップ検証
- 既存テストの `Issue` 構造体リテラルのコンパイルエラー → `body_hash: None` を追加する必要あり（コンパイルエラーで検出可能）

---

## 参考

- `execute.rs` の `prepare_init_handle` / `prepare_process_spawn` / `execute_one` — 変更対象の実装詳細
- `sqlite_issue_repository.rs` の `row_to_issue` / `update_state_and_metadata` — DB パターンの参考
- `sqlite_connection.rs` の `run_add_column_migration` — マイグレーションパターンの参考
- `association_guard.rs` — ラベル削除 + コメント通知の先例パターン
- [sha2 crate (RustCrypto)](https://crates.io/crates/sha2) — SHA-256 実装
