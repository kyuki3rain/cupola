# Research & Design Decisions

---
**Purpose**: SIGHUP ハンドリング方針の調査と設計根拠の記録。

---

## Summary

- **Feature**: `issue-263` — SIGHUP ハンドリング
- **Discovery Scope**: Simple Addition（既存シグナルハンドリングへの追加）
- **Key Findings**:
  - `PollingUseCase::run()` の `tokio::select!` に SIGTERM / SIGINT ハンドラーがあり、SIGHUP も同じパターンで追加できる
  - `tokio::signal::unix::signal(SignalKind::hangup())` で SIGHUP を購読できる
  - daemon は `setsid()` で端末から切り離されるため、端末切断による SIGHUP は発生しないが、ユーザーまたは管理ツールが `kill -HUP <pid>` で送信するケースに対応する必要がある

## Research Log

### 既存シグナルハンドリングの調査

- **Context**: SIGHUP の追加箇所を特定するため
- **Findings**:
  - `src/application/polling_use_case.rs` の `run()` 関数内の `tokio::select!` ブロックで SIGTERM と SIGINT を処理している
  - SIGTERM: `signal::unix::signal(SignalKind::terminate())` → `sigterm.recv()`
  - SIGINT: `signal::ctrl_c()`
  - 追加パターン: `signal::unix::signal(SignalKind::hangup())` で購読し、受信時に SIGTERM と同様のログ出力と `break` を行う
- **Implications**: 既存パターンをそのまま踏襲できる。新たなトレイトや型の追加は不要

### tokio シグナルハンドリングの確認

- **Context**: `SignalKind::hangup()` の可用性と使用方法の確認
- **Findings**:
  - `tokio::signal::unix::SignalKind::hangup()` は tokio の Unix シグナルサポートに含まれる
  - `signal::unix::signal()` は `UnixStream` を内部で使用し、非同期で安全にシグナルを受信できる
  - 既存のコードで `use tokio::signal::unix::SignalKind;` がすでにインポートされている
- **Implications**: 追加インポートなし。既存の `use` 文を再利用できる

### daemon の SIGHUP 受信シナリオ

- **Context**: SIGHUP が実際に送信されるケースの把握
- **Findings**:
  - daemon mode: `setsid()` 呼び出しにより端末との関連付けが切れるため、端末切断による SIGHUP は不発生
  - foreground mode: 端末切断または意図的な `kill -HUP` で受信しうる
  - `logrotate` 等の外部ツールが config reload の慣例として SIGHUP を送信することがある
- **Implications**: Option B（グレースフルシャットダウン）はどのモードでも安全に機能する

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations |
|--------|-------------|-----------|---------------------|
| A: Config reload | SIGHUP で `cupola.toml` を再読み込み | Unix 慣例に沿う | 設定変更の整合性確保が複雑。polling_interval 変更には ticker の再作成が必要。`Config` が複数のコンポーネントに渡っており、`Arc<RwLock<Config>>` への移行が必要 |
| B: Graceful shutdown | SIGHUP を SIGTERM 相当として扱う | 実装が単純・安全。既存パターンの踏襲 | Unix 慣例（reload）からの逸脱。ドキュメントへの明記が必要 |

## Design Decisions

### Decision: Option B — グレースフルシャットダウン

- **Context**: SIGHUP 受信時の動作として Config reload か、グレースフルシャットダウンかを選択する
- **Alternatives Considered**:
  1. Option A — `cupola.toml` の動的再読み込み
  2. Option B — SIGTERM 相当のグレースフルシャットダウン
- **Selected Approach**: Option B を採用。SIGHUP 受信時は SIGTERM と同じグレースフルシャットダウンを実行し、config reload は行わない
- **Rationale**: Issue #263 での推奨事項と一致。Config の動的変更は `polling_interval`、`max_concurrent_sessions` などのフィールドが複数コンポーネントに伝播しており、整合性を保つ実装コストが高い。現時点での使用頻度も低い
- **Trade-offs**: Unix 慣例（SIGHUP = reload）に完全には沿わないが、動作をドキュメントで明示することで運用上の混乱を防ぐ
- **Follow-up**: 将来的に config reload が必要になった場合は、`Config` を `Arc<RwLock<Config>>` に変更する Option A として別 Issue で実装する

## Risks & Mitigations

- `logrotate` 等が SIGHUP を送信した場合にシャットダウンしてしまう — `docs/commands/start.md` への明記と、将来的な Option A 実装で対応
- foreground mode で端末から `Ctrl-\` (SIGQUIT) が来た場合は未ハンドリングのまま（本 Issue のスコープ外）

## References

- [tokio::signal::unix](https://docs.rs/tokio/latest/tokio/signal/unix/index.html) — Unix シグナルの非同期ハンドリング
- [POSIX setsid(2)](https://man7.org/linux/man-pages/man2/setsid.2.html) — セッション切り離し（daemon mode での SIGHUP 無効化の根拠）
