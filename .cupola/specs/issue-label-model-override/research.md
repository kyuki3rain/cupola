# リサーチ & 設計判断ログ

---
**Purpose**: ディスカバリフェーズで得た調査知見・アーキテクチャ検討・意思決定の根拠を記録する。

---

## Summary

- **Feature**: `issue-label-model-override`
- **Discovery Scope**: Extension（既存システムへの機能追加）
- **Key Findings**:
  - `ClaudeCodeRunner::spawn` は既に `model: &str` パラメータを受け取る設計になっており、spawn 側の変更は最小限で済む
  - `PollingUseCase::step7_spawn_processes` は現在 `&self.config.model` のみを使用しており、Issue レベルのモデル上書き機能を追加するには Issue レコードの `model` フィールドを参照するよう変更が必要
  - `GitHubIssueDetail` は `labels: Vec<String>` を持つが、`GitHubIssue`（list_ready_issues の戻り値）はラベルを持たない。非終端 Issue のラベル動的更新のため、新しい `get_issue_labels` メソッドが必要

## Research Log

### 既存コードの統合ポイント分析

- **Context**: 機能追加の影響範囲を特定するための既存コード調査
- **Sources Consulted**: `src/` 全体のコード調査
- **Findings**:
  - `src/domain/issue.rs`: `Issue` struct に `model: Option<String>` フィールドが存在しない。追加が必要。
  - `src/adapter/outbound/sqlite_connection.rs` (L44-57): issues テーブルに `model` カラムが存在しない。`ALTER TABLE` または CREATE TABLE 更新が必要。
  - `src/application/port/github_client.rs`: `GitHubIssue` は `number` と `title` のみ。`GitHubIssueDetail` は `labels: Vec<String>` を持つ。ラベル取得には `get_issue()` を使うか、新メソッドを追加するか検討が必要。
  - `src/application/polling_use_case.rs` (L532): `self.config.model` を直接 spawn に渡している。`issue.model` との優先順位制御が必要。
  - `src/application/port/claude_code_runner.rs`: `spawn(&self, prompt: &str, working_dir: &Path, json_schema: Option<&str>, model: &str)` — `model` パラメータは既に存在する。
- **Implications**: domain・adapter・application の 3 層にまたがる変更が必要。bootstrap 層の変更は不要。

### ラベル取得方式の検討

- **Context**: polling サイクルごとに非終端 Issue のラベルを再確認する方式を決定するための調査
- **Findings**:
  - 既存の `get_issue(issue_number)` は `GitHubIssueDetail`（labels 含む）を返すが、title・body など余分なフィールドも取得する
  - `list_ready_issues` の戻り値 `GitHubIssue` には labels が含まれないため、新規検出時のラベル初期化にはそのまま使えない
  - GitHub REST API には `GET /repos/{owner}/{repo}/issues/{issue_number}/labels` エンドポイントがあり、ラベルのみを軽量に取得可能
- **Implications**: 新しい `get_issue_labels(issue_number: u64) -> Result<Vec<String>>` メソッドを `GitHubClient` トレイトに追加することで、step 1 での軽量なラベル再確認が可能になる

### SQLite スキーマ変更方針

- **Context**: 既存 DB との後方互換性を確保しつつ `model` カラムを追加する方式の検討
- **Findings**:
  - 既存コードでは `CREATE TABLE IF NOT EXISTS` でスキーマを定義しており、既存 DB には新カラムが追加されない
  - `ALTER TABLE issues ADD COLUMN model TEXT` は SQLite でサポートされており、既存行には `NULL` が入る
  - cupola の init コマンド（`cargo run -- init`）がスキーマ初期化を担当しており、マイグレーション戦略の検討が必要
- **Implications**: `sqlite_connection.rs` の CREATE TABLE 定義に `model TEXT` を追加し、既存 DB には `ALTER TABLE` マイグレーションを適用する設計が必要

## Architecture Pattern Evaluation

| Option | 説明 | 利点 | リスク / 制限 | 採用 |
|--------|------|------|---------------|------|
| A: get_issue() 流用 | 非終端 Issue のラベル確認に既存の `get_issue()` を使用 | 新メソッド不要 | title・body も取得するため無駄な転送量が増える | 不採用 |
| B: GitHubIssue に labels 追加 | `list_ready_issues` の戻り値にラベルを含める | 新規 Issue 検出時に即座にモデル初期化可能 | 非終端 Issue のラベル更新には対応できない（別途仕組みが必要） | 部分採用（新規 Issue 初期化は list_ready_issues の labels で行う） |
| C: 新メソッド get_issue_labels 追加 | ラベルのみを取得する専用 API メソッド | 軽量、用途が明確 | トレイト・アダプタ両方の実装が必要 | **採用**（非終端 Issue の動的更新に使用） |

## Design Decisions

### Decision: GitHubClient に get_issue_labels を追加

- **Context**: ポーリングサイクルごとに非終端 Issue のラベルを再確認する要件への対応
- **Alternatives Considered**:
  1. `get_issue()` 流用 — 余分なデータ取得あり
  2. `GitHubIssue` に labels フィールドを追加し `list_ready_issues` で取得 — 非終端 Issue の更新には対応できない
  3. `get_issue_labels()` 新メソッド追加 — 専用・軽量
- **Selected Approach**: `get_issue_labels(issue_number: u64) -> Result<Vec<String>>` を `GitHubClient` トレイトに追加し、step 1 の非終端 Issue ループ内で呼び出す
- **Rationale**: 用途が明確で、将来の GitHub API 実装でも適切なエンドポイント（`GET /issues/{n}/labels`）を使用できる
- **Trade-offs**: トレイト・アダプタ両方に実装が必要だが、テスト容易性が向上する
- **Follow-up**: 統合テスト用モックへの実装を忘れずに追加すること

### Decision: model 優先順位を polling_use_case の step7 で解決する

- **Context**: Issue モデル・設定ファイルモデル・デフォルトの 3 段階優先順位の実装場所
- **Alternatives Considered**:
  1. 優先順位解決ロジックをドメイン層（Issue struct）に持つ
  2. アプリケーション層（PollingUseCase）で解決
  3. Config に解決メソッドを追加
- **Selected Approach**: `PollingUseCase::step7_spawn_processes` 内で `issue.model.as_deref().unwrap_or(&self.config.model)` として解決する
- **Rationale**: モデル選択はユースケース固有のオーケストレーションロジックであり、アプリケーション層が適切。ドメイン層は純粋であるべき。
- **Trade-offs**: PollingUseCase が変更されるが、責務は適切に分離されている

### Decision: モデル名の validation は実施しない

- **Context**: `model:opus` の `opus` 部分のバリデーション方針
- **Selected Approach**: ラベルから抽出したモデル名をそのまま `--model` フラグに渡す。バリデーションは Claude Code プロセス側に委ねる。
- **Rationale**: Claude がサポートするモデル名は変化する可能性があり、Cupola 側でハードコードするとメンテナンスコストが高い。
- **Trade-offs**: 誤ったモデル名を指定した場合は Claude Code の起動失敗で検知される（retry_count が増加）

## Risks & Mitigations

- **既存 DB へのマイグレーション漏れ** — `cargo run -- init` がべき等に `ALTER TABLE IF NOT EXISTS` を実行するか、または起動時に自動マイグレーションを行う仕組みを追加する
- **ラベル取得 API レート制限** — `get_issue_labels` を非終端 Issue ごとに毎サイクル呼び出すため、Issue 数が多い場合は GitHub API レート制限に達する可能性がある。現時点では許容範囲内と判断するが、将来的には `list_ready_issues` のレスポンスにラベル情報を含めてリクエスト数を削減することが可能
- **model:* ラベルが複数存在する場合の順序不定** — ラベルリストの先頭に見つかったものを使用するという仕様で対応。ドキュメントに明記する。

## References

- GitHub REST API - List labels for an issue: `GET /repos/{owner}/{repo}/issues/{issue_number}/labels`
- `src/application/port/claude_code_runner.rs` — 既存の `spawn` シグネチャ（`model: &str` 引数あり）
- `src/adapter/outbound/claude_code_process.rs` — `--model` フラグの実際の組み立て箇所
- `src/application/polling_use_case.rs` L532 — 現在のモデル指定箇所
