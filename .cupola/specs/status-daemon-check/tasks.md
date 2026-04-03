# Implementation Plan

## Task Format Template

- `(P)` はタスクが並列実行可能であることを示す
- `- [x]*` はMVP後に対応可能な任意テスト項目を示す

---

- [x] 1. daemon起動状態の判定・表示ロジックを実装する
- [x] 1.1 statusコマンドにPIDファイル読み取りとdaemon生存確認ロジックを追加する
  - `.cupola/cupola.pid` を参照する `PidFileManager` を初期化する
  - `read_pid()` でPIDファイルを読み取る
  - PIDが取得できた場合は `is_process_alive(pid)` でプロセス生存を確認する
  - 生存中の場合は `Daemon: running (pid=<PID>)` を出力する
  - PIDファイルが存在しない（`None`）場合は `Daemon: not running` を出力する
  - PIDが取得できてもプロセスが存在しない場合は `delete_pid()` でPIDファイルを削除し、`Daemon: not running (stale PID file cleaned)` を出力する
  - `read_pid()` がエラーを返した場合は `Daemon: not running` として扱う（エラーをユーザーに露出しない）
  - daemon状態の出力はissue一覧の出力より前に行う
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [x] 2. issueごとのプロセス生存確認とRunning:カウント更新を実装する
- [x] 2.1 Running:カウントの算出をプロセス実際の生存状態ベースに変更する
  - 現在の `current_pid.is_some()` 判定を `current_pid.is_some() && is_process_alive(pid)` に変更する
  - `needs_process()` 条件は引き続き維持する
  - カウント算出時に1.1で初期化した `PidFileManager` を再利用する
  - _Requirements: 2.4_

- [x] 2.2 issue一覧の各行にプロセス生存状態を付加する
  - 各issueの `current_pid` が `Some(pid)` の場合、`is_process_alive(pid)` を呼び出す
  - プロセスが生存していれば `pid:<PID> (alive)` を行末に付加する
  - プロセスが存在しなければ `pid:<PID> (dead)` を行末に付加する
  - `current_pid` が `None` の場合は付加情報なし（既存フォーマットを維持）
  - 既存の表示項目（state、PR番号、retry_count、worktree_path、error_message）は変更しない
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2_

- [x] 3. テストを実装する
- [x] 3.1 daemon状態表示の各ケースのユニットテストを追加する
  - PIDファイルが存在しプロセスが生存している場合: `Daemon: running (pid=<PID>)` が出力されることを検証する
  - PIDファイルが存在しない場合: `Daemon: not running` が出力されることを検証する
  - stale PIDファイルの場合: PIDファイルが削除され `Daemon: not running (stale PID file cleaned)` が出力されることを検証する
  - daemon状態の出力がissue一覧より前に現れることを検証する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 3.2 (P) issueプロセス生存確認の各ケースのユニットテストを追加する
  - `current_pid` が alive なissueの行に `(alive)` が含まれることを検証する
  - `current_pid` が dead なissueの行に `(dead)` が含まれることを検証する
  - `current_pid` が `None` のissueの行にPID情報が含まれないことを検証する
  - Running:カウントが実際に alive なプロセスのみをカウントすることを検証する
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [x]* 3.3 (P) 既存のstatusテストが引き続きパスすることを確認する
  - `status_with_no_active_issues` テストが変更後も通ることを確認する
  - `status_with_active_issues` テストが変更後も通ることを確認する
  - _Requirements: 3.3, 3.4_
