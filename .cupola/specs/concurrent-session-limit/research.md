# Research & Design Decisions

## Summary
- **Feature**: concurrent-session-limit
- **Discovery Scope**: Extension（既存システムへの機能追加）
- **Key Findings**:
  - SessionManager は HashMap<i64, SessionEntry> でセッションを管理しており、`sessions.len()` で実行数を取得可能
  - step7_spawn_processes は for ループ内で逐次起動しており、ループ冒頭にカウントチェックを追加するだけで実装可能
  - Config は domain 層の値オブジェクトで、CupolaToml（bootstrap 層）からマッピングされる既存パターンに従えばよい

## Research Log

### SessionManager の内部構造
- **Context**: count() メソッド追加の実現可能性を確認
- **Sources Consulted**: `src/application/session_manager.rs`
- **Findings**:
  - `sessions: HashMap<i64, SessionEntry>` で issue_id をキーにセッションを管理
  - 既存メソッド: `register()`, `collect_exited()`, `is_running()`, `kill()`, `kill_all()`, `find_stalled()`
  - `collect_exited()` で終了済みプロセスを回収し HashMap から除去するため、`sessions.len()` は常に実行中プロセス数を反映
- **Implications**: `pub fn count(&self) -> usize { self.sessions.len() }` で正確なカウントを返せる

### step7_spawn_processes のフロー
- **Context**: 上限チェックの挿入箇所を特定
- **Sources Consulted**: `src/application/polling_use_case.rs` (lines 448-513)
- **Findings**:
  - `find_needing_process()` で needs_process な Issue 一覧を取得
  - for ループで各 Issue に対して `is_running()` チェック → input 準備 → spawn → register
  - ループ内で毎回チェックすることで、1 サイクル内で上限に達した場合も即座にスキップ可能
- **Implications**: for ループの冒頭（`is_running` チェックの前）にカウントチェックを追加。上限到達時は break でループを抜ける

### Config と CupolaToml のマッピング
- **Context**: 新規設定項目の追加パターンを確認
- **Sources Consulted**: `src/domain/config.rs`, `src/bootstrap/config_loader.rs`
- **Findings**:
  - CupolaToml は serde Deserialize でフラットなフィールドを持つ（`[log]` セクションのみネスト）
  - `into_config()` で CupolaToml → Config へ変換、CLI オーバーライドを適用
  - Optional フィールドは `Option<T>` + `#[serde(default)]` パターンで後方互換性を維持
- **Implications**: `max_concurrent_sessions: Option<u32>` をトップレベルフィールドとして追加する既存パターンに従う

### Status コマンドの現状
- **Context**: 実行状態表示の拡張箇所を特定
- **Sources Consulted**: `src/bootstrap/app.rs` (lines 89-126)
- **Findings**:
  - 現在は DB から active issues を取得して表示するのみ
  - SessionManager にアクセスしていない（status コマンドは polling ループ外で実行される）
  - DB の `current_pid` フィールドから実行中かどうかを推定可能
- **Implications**: SessionManager は status コマンドからアクセスできないため、DB ベースでカウントする必要がある。そのために polling ループ側で「プロセス起動時に DB の `current_pid` を `Some(pid)` に更新し、`collect_exited()` で終了を検知したタイミングで `None` にクリアする」更新フローを設計・実装タスク化する。Status コマンドでは active issues のうち `needs_process()` かつ `current_pid.is_some()` のものを `running_count` としてカウントする

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| ループ内カウントチェック | for ループ冒頭で毎回 count() と上限を比較 | 最小変更、既存フロー維持 | なし | Issue の設計方針と完全一致 |
| 事前フィルタリング | ループ前に起動可能数を計算し needing をスライス | ループ内チェック不要 | 一括計算のため柔軟性が低い | 過剰設計 |

## Design Decisions

### Decision: ループ内カウントチェック方式
- **Context**: step7 で上限を超えないようにする方法の選択
- **Alternatives Considered**:
  1. ループ冒頭で毎回 `session_mgr.count()` を呼び出し、上限以上なら break
  2. ループ前に残り起動可能数を計算し、カウントダウンで制御
- **Selected Approach**: Option 1（ループ内カウントチェック）
- **Rationale**: コードが最も単純で意図が明確。`count()` は O(1) のため性能影響なし。ループ中に他のプロセスが終了する可能性は無視できる（同期処理のため）
- **Trade-offs**: 毎回 count() を呼ぶが HashMap::len() は O(1) のため問題なし

### Decision: Status コマンドでの DB ベースカウント
- **Context**: status コマンドは polling ループ外で実行されるため SessionManager にアクセスできない
- **Alternatives Considered**:
  1. DB の active issues から needs_process かつ current_pid ありのものをカウント
  2. SessionManager を status コマンドからもアクセス可能にする（アーキテクチャ変更）
- **Selected Approach**: Option 1（DB ベースカウント）
- **Rationale**: 既存のアーキテクチャを変更せず、DB 情報で十分正確な近似値を提供できる
- **Trade-offs**: プロセスが異常終了した直後は DB とのずれが生じうるが、次の polling サイクルで修正される

## Risks & Mitigations
- リスク: status コマンドでの実行中プロセス数が DB ベースのため若干不正確になりうる → polling サイクルで自動修正されるため実用上問題なし
- リスク: max_concurrent_sessions=0 の場合に全プロセス起動不可 → 0 以下の値はバリデーションエラーとして拒否する（requirements の「正の整数」と整合）
