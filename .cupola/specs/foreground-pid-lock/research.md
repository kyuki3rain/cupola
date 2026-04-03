# リサーチ & 設計決定ログ

---
**Purpose**: foreground-pid-lock フィーチャーの調査結果および設計根拠を記録する。

---

## Summary
- **Feature**: `foreground-pid-lock`
- **Discovery Scope**: Extension（既存システムへの機能追加）
- **Key Findings**:
  - `start_daemon` に PID チェックロジックが既実装済みで再利用可能
  - `apply_pid_cleanup` ヘルパーが既存で、foreground でもそのまま利用できる
  - `PollingUseCase::with_pid_file` が SIGTERM/SIGINT 時のグレースフルな PID 削除を担う

## Research Log

### 既存 PID ファイル実装の調査

- **Context**: foreground と daemon で同じチェックロジックを共有するため、既存コードを把握する
- **Sources Consulted**: `src/bootstrap/app.rs`、`src/adapter/outbound/pid_file_manager.rs`、`src/application/port/pid_file.rs`
- **Findings**:
  - `start_daemon` (L420-436): PID ファイル読み取り → 生存確認 → エラー or ゾンビ削除のロジックが実装済み
  - `start_daemon_child` (L471-557): PID 書き込み → ポーリング実行 → `apply_pid_cleanup` で確実削除
  - `apply_pid_cleanup` (L561-568): 正常・エラー問わず PID ファイルを best-effort 削除し、元の結果を返す
  - `PollingUseCase::with_pid_file` (L85-88): SIGTERM/SIGINT グレースフルシャットダウン時に PID ファイルを削除
  - `start_foreground` (L336-393): PID ファイル未使用（コメント `// no PID file in foreground mode` が明示）
- **Implications**: `start_daemon` のチェックロジックを共有ヘルパー関数へ抽出することで重複を排除できる

### PID ファイルパス設計

- **Context**: foreground と daemon が同一 PID ファイルを共有するかを確認
- **Findings**:
  - daemon は `config_dir.join("cupola.pid")` を使用 (`<config_dir>/cupola.pid`)
  - foreground も同じパスを使うべき（相互排他のため）
  - config_dir は `config.parent().unwrap_or_else(|| Path::new("."))` で算出
- **Implications**: foreground に `config_dir` 計算を追加し、同一パスを使用する

### グレースフルシャットダウン経路の分析

- **Context**: foreground モードの終了時に PID ファイルが確実に削除されるか確認
- **Findings**:
  - `PollingUseCase::with_pid_file` は SIGTERM/SIGINT の graceful_shutdown() 内で delete_pid を呼ぶ
  - `apply_pid_cleanup` は polling.run() の返却値（Ok/Err）に関わらず best-effort で削除
  - この二重の保護が daemon_child では実装されている
- **Implications**: foreground にも同じ二重保護パターンを適用する（`with_pid_file` + `apply_pid_cleanup`）

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: 直接コピー | `start_daemon` のチェックロジックを `start_foreground` にそのまま複製 | シンプル、変更ファイル最小 | コード重複、将来のメンテナンス負荷 | 小規模変更では許容できるが非推奨 |
| B: 共有ヘルパー抽出 | チェックロジックを `check_and_clean_pid_file` として抽出し両者から呼ぶ | DRY、Issue 要件「同じロジックを共有」に合致 | 若干のリファクタリングが必要 | **選択** |

## Design Decisions

### Decision: `check_and_clean_pid_file` の共有ヘルパー化

- **Context**: daemon と foreground で同一の PID チェック/ゾンビ削除ロジックが必要
- **Alternatives Considered**:
  1. `start_daemon` のコードを `start_foreground` に複製
  2. `app.rs` にプライベートヘルパー関数として抽出
- **Selected Approach**: `fn check_and_clean_pid_file(pid_file_manager: &PidFileManager) -> Result<()>` を `app.rs` 内プライベート関数として定義し、`start_daemon` と `start_foreground` の両方から呼ぶ
- **Rationale**: Issue 仕様「同じチェックロジックを共有」に直接対応。clean architecture の bootstrap 層で完結するため層跨ぎなし
- **Trade-offs**: `start_daemon` 側のインラインコードをヘルパー呼び出しに置き換える追加変更が必要
- **Follow-up**: `start_daemon` のチェックロジックをヘルパー呼び出しに置き換えてテスト

### Decision: `start_foreground` の PID 書き込みタイミング

- **Context**: PID 書き込みは logging 初期化の前後どちらで行うか
- **Alternatives Considered**:
  1. ロギング初期化前に書き込む（`start_daemon_child` 方式）
  2. ロギング初期化後に書き込む
- **Selected Approach**: `start_daemon_child` と同じく、ロギング初期化前に書き込む
- **Rationale**: PID 書き込み失敗は即座にエラーとすべきであり、ロギング不要。daemon_child との一貫性を保つ
- **Trade-offs**: ログに「PID 書き込み成功」を記録できないが、実用上問題なし

## Risks & Mitigations
- **競合状態 (TOCTOU)**: PID チェックと書き込みの間に他プロセスが起動する可能性 — PID ファイルはアトミック書き込みではないが、ポーリング間隔（秒単位）とユーザー操作の特性上、実用上のリスクは低い
- **ゾンビ PID ファイル残存**: `apply_pid_cleanup` と `with_pid_file` の二重保護で対応済み
- **foreground 起動時の設定ファイルなし**: 既存の `load_toml` でエラー処理済み

## References
- `src/bootstrap/app.rs` — 既存 daemon/daemon_child 実装
- `src/adapter/outbound/pid_file_manager.rs` — PidFileManager 実装
- `src/application/port/pid_file.rs` — PidFilePort トレイト定義
