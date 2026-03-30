# Research & Design Decisions

## Summary
- **Feature**: `info-log-improvement`
- **Discovery Scope**: Extension（既存システムへのログ追加）
- **Key Findings**:
  - 状態遷移の INFO ログは `TransitionUseCase::apply()` に 1 箇所追加するだけで全遷移をカバーできる
  - PR 作成・fixing 後処理・プロセス終了のログは `PollingUseCase` 内の既存処理フローに `tracing::info!` を追加する形で対応可能
  - 既存のログ基盤（tracing + 構造化ログ）をそのまま活用でき、新規依存は不要

## Research Log

### 既存ログパターンの調査
- **Context**: 現在の INFO ログの使用状況を把握し、追加ログのフォーマットを統一する必要がある
- **Sources Consulted**: `src/application/polling_use_case.rs`, `src/application/transition_use_case.rs`, `src/bootstrap/logging.rs`
- **Findings**:
  - `tracing::info!` マクロを使用した構造化ログが既存パターン
  - フィールド名は `snake_case`（例: `issue_id`, `feature_name`）
  - メッセージは英語（既存パターン: `"received SIGINT, shutting down..."`, `"cleared stale PID on startup"`）
- **Implications**: 新規ログも同じパターン（`tracing::info!` + 構造化フィールド + 英語メッセージ）に従う

### 状態遷移ログの挿入箇所
- **Context**: 全ての状態遷移を漏れなくカバーするための最適な挿入箇所を特定する
- **Sources Consulted**: `src/application/transition_use_case.rs`
- **Findings**:
  - `TransitionUseCase::apply()` が唯一の状態遷移エントリポイント
  - `old_state` と `new_state` が既にローカル変数として存在（L24）
  - `issue.github_issue_number` でイシュー番号にアクセス可能
- **Implications**: `apply()` 内に 1 行追加するだけで全状態遷移をカバーできる

### PR 作成ログの挿入箇所
- **Context**: PR 作成成功時と既存 PR スキップ時の両方をログに記録する
- **Sources Consulted**: `src/application/polling_use_case.rs` L539-607
- **Findings**:
  - `create_pr_from_output()` メソッド内に 3 つのパスが存在:
    1. DB に既存 PR 番号がある場合（L557-558）→ スキップ
    2. GitHub に既存 PR がある場合（L562-571）→ スキップ
    3. 新規 PR 作成（L587）→ 作成成功
  - `head`, `base`, `pr_number` が全てローカル変数として利用可能
- **Implications**: 3 箇所にそれぞれ `tracing::info!` を追加

### fixing 後処理ログの挿入箇所
- **Context**: スレッド返信・resolve の成功を記録する
- **Sources Consulted**: `src/application/polling_use_case.rs` L609-630
- **Findings**:
  - `process_fixing_output()` 内でスレッドごとにループ処理
  - 返信成功は `reply_to_thread` の Ok パス（現在はログなし）
  - resolve 成功は `resolve_thread` の Ok パス（現在はログなし）
  - 処理完了のサマリーログは関数末尾に追加可能
- **Implications**: ループ内に返信/resolve 成功ログ、ループ後にサマリーログを追加

### プロセス終了ログの挿入箇所
- **Context**: Claude Code プロセスの exit_code をログに記録する
- **Sources Consulted**: `src/application/polling_use_case.rs` L253-325
- **Findings**:
  - `step3_process_exit_check()` で `session.exit_status` を取得
  - 成功パス（L269）と失敗パス（L272）が分岐
  - `issue.github_issue_number` と `session.exit_status.code()` が利用可能
- **Implications**: 成功/失敗パスの分岐直後にそれぞれ INFO ログを追加

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 直接 tracing::info! 追加 | 既存コードの適切な箇所に tracing::info! を追加 | 最小変更、既存パターン準拠、新規依存なし | なし | 採用 |

## Design Decisions

### Decision: ログ追加方式
- **Context**: INFO レベルログの追加方法として、既存コードへの直接追加か、ロギング用のラッパー/ミドルウェアの導入かを検討
- **Alternatives Considered**:
  1. 既存コードへの直接 `tracing::info!` 追加
  2. ロギング用のラッパー trait / middleware の導入
- **Selected Approach**: 直接追加
- **Rationale**: ログ箇所が限定的（5-6 箇所）で、ラッパーを導入する複雑さに見合わない。既存パターンとの一貫性を維持できる
- **Trade-offs**: ログフォーマットの統一は開発者の注意に依存するが、箇所が少ないため問題にならない
- **Follow-up**: テストで構造化ログフィールドの存在を検証

## Risks & Mitigations
- ログ量増加によるパフォーマンス影響 — INFO レベルのイベントログは頻度が低く無視できる
- ログメッセージの一貫性 — 既存パターンに従い、構造化フィールドを統一する

## References
- tracing crate: 構造化ログの公式ドキュメント
- 既存コード: `src/application/polling_use_case.rs`, `src/application/transition_use_case.rs`
