# 実装計画

- [x] 1. PidFilePort トレイトに ProcessMode 型と新しいメソッドを追加する
- [x] 1.1 ProcessMode 列挙型を定義する
  - `application/port/pid_file.rs` に `ProcessMode { Foreground, Daemon }` を追加する
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` を付与する
  - _Requirements: 1.1_

- [x] 1.2 PidFilePort トレイトに `write_pid_with_mode` を追加する
  - `write_pid_with_mode(&self, pid: u32, mode: ProcessMode) -> Result<(), PidFileError>` を追加する
  - 既存の `write_pid` は後方互換のために残す
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 1.3 PidFilePort トレイトに `read_pid_and_mode` を追加する
  - `read_pid_and_mode(&self) -> Result<Option<(u32, Option<ProcessMode>)>, PidFileError>` を追加する
  - 既存の `read_pid` は後方互換のために残す
  - _Requirements: 1.1, 1.4_

- [x] 2. PidFileManager に新しいトレイトメソッドを実装する
- [x] 2.1 `write_pid_with_mode` を実装する（2行フォーマット書き込み）
  - `create_new` モードでファイルを開き `{pid}\n{mode_string}\n` を書き込む
  - `ProcessMode::Foreground` → `"foreground"`, `ProcessMode::Daemon` → `"daemon"` に変換する
  - 既存の `write_pid` と同様に `AlreadyExists` / `Write` エラーを返す
  - _Requirements: 1.2, 1.3_

- [x] 2.2 `read_pid_and_mode` を実装する（2行＋レガシー1行の後方互換処理）
  - ファイル全体を読み込み、行分割してPIDとモードを解析する
  - 1行目をPIDとしてパースし、`read_pid` と同一の範囲バリデーション（`pid == 0 || pid > i32::MAX`）を適用する
  - 2行目が存在する場合はモード文字列を解釈する（`"foreground"` / `"daemon"` / その他 `None`）
  - 2行目が存在しない（レガシー1行フォーマット）場合は `tracing::info!` を出力して `(pid, None)` を返す
  - ファイルが存在しない場合は `Ok(None)` を返す
  - _Requirements: 1.4, 1.5, 1.6, 1.7_

- [x] 3. 起動時の呼び出しサイトを `write_pid_with_mode` に更新する
- [x] 3.1 foreground 起動の呼び出しサイトを更新する
  - `bootstrap/app.rs` の foreground 起動パス（`~434` 行）で `write_pid(my_pid)` を `write_pid_with_mode(my_pid, ProcessMode::Foreground)` に変更する
  - `ProcessMode` のインポートを追加する
  - _Requirements: 1.2_

- [x] 3.2 daemon child 起動の呼び出しサイトを更新する
  - `bootstrap/app.rs` の daemon child 起動パス（`~595` 行）で `write_pid(my_pid)` を `write_pid_with_mode(my_pid, ProcessMode::Daemon)` に変更する
  - _Requirements: 1.3_

- [x] 4. status コマンドの出力ロジックを修正する
- [x] 4.1 `handle_status` のプロセス状態表示を `Process:` プレフィックスとモード表示に修正する
  - `read_pid()` を `read_pid_and_mode()` に切り替える
  - `"Daemon: running (pid={pid})"` → `"Process: running ({mode_str}, pid={pid})"` に変更する（`mode_str` は `"foreground"` / `"daemon"` / `"unknown"`）
  - stale判定のTOCTOU対策箇所（2回目の読み取り）も `read_pid_and_mode()` に切り替え、PIDのみで等価比較する
  - `"Daemon: not running ..."` 系のメッセージをすべて `"Process: not running ..."` に変更する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

- [x] 4.2 セッション数ラベルを `Claude sessions:` に修正する
  - `"Running: {running_count}/{max}"` → `"Claude sessions: {running_count}/{max}"` に変更する
  - `"Running: {running_count}"` → `"Claude sessions: {running_count}"` に変更する
  - _Requirements: 3.1_

- [x] 5. 新機能のテストを追加する
- [x] 5.1 (P) `write_pid_with_mode` の書き込み系ユニットテストを追加する
  - `test_write_and_read_pid_with_mode_foreground` — foreground 書き込み・読み取りの往復テスト
  - `test_write_and_read_pid_with_mode_daemon` — daemon 書き込み・読み取りの往復テスト
  - `test_write_pid_with_mode_already_exists` — ファイル既存時に `AlreadyExists` を返すことを確認
  - _Requirements: 1.2, 1.3_

- [x] 5.2 (P) `read_pid_and_mode` の読み取り・後方互換ユニットテストを追加する
  - `test_read_pid_and_mode_legacy_single_line` — 1行ファイルを `(pid, None)` として読み取ることを確認
  - `test_read_pid_and_mode_returns_none_when_absent` — ファイル不在時に `Ok(None)` を返すことを確認
  - `test_read_pid_and_mode_invalid_pid` — 不正PID（0・範囲外）を `InvalidContent` エラーとして返すことを確認
  - _Requirements: 1.4, 1.5, 1.6, 1.7_

- [x] 5.3 (P) `handle_status` の出力に関するユニットテストを追加する
  - `test_status_output_foreground_mode` — foreground モード起動中に正しいプレフィックスとモードが表示されることを確認
  - `test_status_output_daemon_mode` — daemon モード起動中に正しい出力が表示されることを確認
  - `test_status_output_unknown_mode` — レガシーPIDファイルの場合に `(unknown, pid=...)` が表示されることを確認
  - `test_status_output_not_running` — PIDファイル不在時に `Process: not running` が表示されることを確認
  - `test_status_output_session_label_with_max` — `Claude sessions: {alive}/{max}` が正しく表示されることを確認
  - `test_status_output_session_label_no_max` — `Claude sessions: {alive}` が正しく表示されることを確認
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 3.1_
