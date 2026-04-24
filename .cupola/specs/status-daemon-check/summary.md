# status-daemon-check サマリ

## Feature
`cupola status` コマンドに (1) daemon プロセスの実起動確認（PID ファイル + signal 0 チェック）と (2) 各 Issue の `current_pid` プロセス生存確認を追加。DB の PID 残骸で「Running」と誤表示される問題を解消し、実態に即した状態表示を提供する。

## 要件サマリ
- 出力先頭に daemon 状態を表示:
  - PID alive: `Daemon: running (pid=<PID>)`
  - PID ファイルなし: `Daemon: not running`
  - stale PID (ファイルあるがプロセスなし): `delete_pid()` 実行後 `Daemon: not running (stale PID file cleaned)`
  - PID ファイル読み取りエラー: `Daemon: not running`（ユーザーにエラー露出しない）
  - 削除失敗: `warn!` ログ + `Daemon: not running (stale PID file exists, but cleanup failed)`
- Issue ごとに `current_pid` が `Some(pid)` なら `is_process_alive(pid)` を呼び、行末に `pid:<PID> (alive)` / `pid:<PID> (dead)` を付加。`None` は既存フォーマットを維持。
- `Running:` カウントを `current_pid.is_some() && is_process_alive(pid)` に基づいて算出（従来は PID 有無のみ）。
- 既存の表示項目（state, PR 番号, retry_count, worktree_path, error_message, `No active issues.`, DB なしエラー）は全て維持。

## アーキテクチャ決定
- **bootstrap 層 (`src/bootstrap/app.rs`) の `Command::Status` ブランチのみ変更** (採用): 既存 `PidFileManager` (`PidFilePort`) が再利用可能で、application/adapter/domain への波及なし。
- **PID ファイルパスはハードコード `.cupola/cupola.pid`** (採用): 既存 `.cupola/cupola.db` のハードコードと同じパターンに沿い、`Command::Status` への `config: PathBuf` 引数追加を回避。CLI シグネチャ不変で後方互換性を維持。
- **stale PID は自動削除** (採用): 表示のみの代替案より、次回起動時の誤検知防止を優先。削除失敗時は warn + 別メッセージで副作用の有無を正直に表示。
- **`read_pid()` エラーは `not running` として扱う**: 破損 PID ファイルでユーザー体験を悪化させないため、エラー詳細は露出しない。
- **プロセス生存確認の競合状態は許容**: signal 0 チェックと表示の間にプロセスが終了するレースは軽微と判断。
- Windows 対応は既存 `nix` クレート制約のまま非スコープ。

## コンポーネント
- `src/bootstrap/app.rs` `Command::Status` ハンドラ:
  - `PidFileManager::new(Path::new(".cupola/cupola.pid").to_path_buf())` で初期化。
  - daemon 状態を issue 一覧より前に出力。
  - `Running:` カウント算出ロジックに `is_process_alive` 呼び出しを組み込む。
  - 各 issue 行に PID 生存情報を付加。
- `src/adapter/outbound/pid_file_manager.rs` (既存 `PidFileManager`): 変更なし。既存 `read_pid()` / `delete_pid()` / `is_process_alive()` をそのまま再利用。
- `src/application/port/pid_file.rs` (`PidFilePort` trait): 変更なし。
- ユニットテスト（`#[cfg(test)]` @ `app.rs`）: 7 ケース（PID alive/なし/stale、issue alive/dead/None、Running カウント）を `PidFilePort` mock で検証。既存 `status_with_no_active_issues` / `status_with_active_issues` は継続パス。

## 主要インターフェース
既存 `PidFilePort` を活用（新規シグネチャなし）:
```rust
trait PidFilePort {
    fn read_pid(&self) -> Result<Option<u32>>;
    fn delete_pid(&self) -> Result<()>;
    fn is_process_alive(&self, pid: u32) -> bool; // signal 0 (EPERM も alive 扱い)
}
```

## 学び / トレードオフ
- `PidFileManager` が既に application/port + adapter/outbound に分離されており、bootstrap からの再利用だけで機能完結できた（既存設計の恩恵）。
- `.cupola/*` パスのハードコードは技術的負債（config ディレクトリカスタマイズ非対応）だが、本機能では DB パスと整合を取ることを優先し将来のリファクタに委ねる。
- stale PID の自動削除は「status は read-only であるべき」原則との衝突があるが、ユーザー体験（次回誤検知防止）と Issue 仕様の明示的要求を優先して副作用を許容、代わりに削除成功/失敗でメッセージを出し分け透明性を担保。
- プロセス生存確認は Issue 数 × signal 0 syscall となるが、`kill(pid, None)` は軽量でパフォーマンス問題なし。
