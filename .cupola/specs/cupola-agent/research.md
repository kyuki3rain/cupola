# 調査・設計判断の記録

## Summary
- **Feature**: `cupola-agent`
- **Discovery Scope**: New Feature（greenfield — Rust で新規構築）
- **Key Findings**:
  - Clean Architecture 4 レイヤー構成が要件の責務分離（GitHub API vs git 操作）と一致する
  - `std::process::Child` + `try_wait()` による同期的プロセス管理が polling ループの設計と自然に統合可能
  - output-schema（`--json-schema` フラグ）による構造化出力が Claude Code CLI でサポートされており、PR 作成・fixing の両方で活用可能

## Research Log

### Claude Code CLI フラグと output-schema

- **Context**: Claude Code を非対話モードで起動し、構造化された出力を取得する方法の確認
- **Sources Consulted**: Claude Code CLI ドキュメント
- **Findings**:
  - `-p <prompt>`: 非対話モード（print mode）
  - `--output-format json`: JSON 出力（session_id, result, structured_output を含む）
  - `--json-schema <schema>`: structured_output のスキーマ指定
  - `--dangerously-skip-permissions`: 許可確認スキップ（自動実行に必須）
  - stdout に JSON が出力され、result フィールドにテキスト出力、structured_output にスキーマ準拠の構造化データが含まれる
- **Implications**: PR 作成用（pr_title, pr_body）と fixing 用（threads 配列）の 2 種類の output-schema を使い分ける設計が成立する

### GitHub GraphQL API — review thread 操作

- **Context**: PR の review thread 取得・返信・resolve に必要な API の確認
- **Sources Consulted**: GitHub GraphQL API ドキュメント
- **Findings**:
  - `pullRequest.reviewThreads` で thread 一覧取得（`isResolved` フィルタ可能）
  - `addPullRequestReviewThreadReply` で既存 thread に返信（`addPullRequestReviewComment` は新スレッド作成用で不適）
  - `resolveReviewThread` で thread を resolve
  - pagination は `first: 100` + `pageInfo { hasNextPage, endCursor }` で対応
- **Implications**: REST API では review thread の resolve ができないため、GraphQL API が必須。`reqwest` で直接 POST する方式が適切

### rusqlite と tokio の統合

- **Context**: 同期 API の rusqlite を非同期ランタイム（tokio）内で安全に使用する方法
- **Sources Consulted**: rusqlite ドキュメント、tokio ドキュメント
- **Findings**:
  - `Arc<Mutex<Connection>>` で共有アクセス（`std::sync::Mutex` を使用）
  - 全 DB 操作を `tokio::task::spawn_blocking` でラップ
  - WAL モード + `busy_timeout` で読み取り並行性を確保
  - `.await` 中に Mutex ロックを保持しないことを保証する設計が重要
- **Implications**: DB 操作は全て spawn_blocking 内で完結させるアダプター設計が必要

### std::process vs tokio::process

- **Context**: Claude Code 子プロセス管理に同期 API と非同期 API のどちらを使うか
- **Sources Consulted**: tokio::process ドキュメント、std::process ドキュメント
- **Findings**:
  - `std::process::Child::try_wait()` は非ブロッキングで終了確認可能
  - tokio::process は非同期 wait に便利だが、polling ループとの統合では try_wait で十分
  - stdout/stderr の読み取りは別スレッドで非同期的に行い、パイプバッファ（64KB）の満杯によるブロックを防止
- **Implications**: `std::process` を採用し、stdout/stderr は `std::thread::spawn` で読み取りスレッドを起動する設計

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Clean Architecture（4 レイヤー） | domain / application / adapter / bootstrap | 依存ルールが明確、テスタビリティ高い、責務分離と一致 | レイヤー間のボイラープレートが増える | 要件の責務分離（GitHub API vs git）と自然にマッチ。CLAUDE.md の clean-architecture ルールにも準拠 |
| Hexagonal Architecture | Ports & Adapters | Clean Architecture と本質的に同じ | 命名が異なるだけ | Clean Architecture として統一 |
| Simple Layered | controller / service / repository | シンプル | ドメインロジックとインフラの境界が曖昧になりやすい | ステートマシンのような複雑なドメインロジックには不向き |

**選択**: Clean Architecture（4 レイヤー）。ステートマシン駆動のドメインロジックを純粋に保ちつつ、GitHub API・SQLite・Claude Code プロセス等の外部依存をアダプターとして隔離する。

## Design Decisions

### Decision: プロセス管理方式

- **Context**: Claude Code 子プロセスの起動・終了確認・stdout 読み取りの方式選定
- **Alternatives Considered**:
  1. `tokio::process::Command` — 非同期 API でプロセス管理
  2. `std::process::Command` + `try_wait()` — 同期 API + polling
- **Selected Approach**: `std::process::Command` + `try_wait()` + 読み取りスレッド
- **Rationale**: polling ループとの統合が自然。try_wait は非ブロッキングで、polling サイクルごとに終了確認するパターンに合致。stdout/stderr は別スレッドで蓄積し、パイプバッファ枯渇を防止
- **Trade-offs**: tokio の非同期 wait に比べてコードが若干冗長だが、SessionManager の設計がシンプルになる
- **Follow-up**: 読み取りスレッドの JoinHandle の管理とエラーハンドリング

### Decision: GitHub API クライアント構成

- **Context**: REST API と GraphQL API の使い分けをアプリケーション層にどう見せるか
- **Alternatives Considered**:
  1. REST 用と GraphQL 用で別 trait を定義
  2. 単一の `GitHubClient` trait に統合
- **Selected Approach**: 単一の `GitHubClient` trait に統合
- **Rationale**: アプリケーション層は REST/GraphQL の区別を関知しない。実装の内部で操作ごとに適切な API を選択する
- **Trade-offs**: trait が大きくなるが、呼び出し側のシンプルさを優先
- **Follow-up**: trait のメソッド数が増えすぎた場合は sub-trait への分割を検討

### Decision: イベント収集 → バッチ適用方式

- **Context**: polling サイクル内でのイベント処理順序
- **Alternatives Considered**:
  1. イベントを検知した時点で即時適用
  2. サイクル内で全イベントを収集し、最後にバッチ適用
- **Selected Approach**: バッチ適用方式（Initialized 復旧のみ即時適用の例外あり）
- **Rationale**: 同一 Issue に複数イベントが発生した場合の優先制御（IssueClosed 最優先）が可能。状態遷移の一貫性を確保
- **Trade-offs**: 1 サイクル分の遅延が発生するが、正確性を優先
- **Follow-up**: Initialized の即時適用が例外的なため、コード上のコメントで設計意図を明記

## Risks & Mitigations

- **野良プロセスリスク** — Cupola 再起動時に前回の Claude Code プロセスが残存する可能性。current_pid による kill は PID 再利用リスクがあるため、現時点では許容。stall timeout で自然回収を期待
- **git index.lock 競合** — 野良プロセスとの競合を防止するため、プロセス起動前に `.git/index.lock` を削除する。ただし TOCTOU リスクは残る
- **GraphQL API pagination** — review thread が 100 件を超える場合の pagination 実装漏れ。初期実装では 100 件上限で対応し、超過時はログ警告を出力

## References

- [Claude Code CLI ドキュメント](https://docs.anthropic.com/en/docs/claude-code) — CLI フラグ、output-schema の仕様
- [GitHub GraphQL API — PullRequestReviewThread](https://docs.github.com/en/graphql/reference/objects#pullrequestreviewthread) — review thread の取得・操作
- [rusqlite](https://docs.rs/rusqlite/) — SQLite バインディング、WAL モード
- [octocrab](https://docs.rs/octocrab/) — GitHub REST API クライアント
