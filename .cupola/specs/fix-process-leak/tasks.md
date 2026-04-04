# Implementation Plan

- [ ] 1. `graceful_shutdown` のループ終了判定を修正する
- [ ] 1.1 ループ継続条件を `session_mgr.count() != 0` に変更する
  - `collect_exited()` の戻り値が空かどうかではなく、残存セッション数が 0 になったかどうかをループ終了条件とする
  - `collect_exited()` の呼び出し自体は維持し、副作用（セッション回収）を継続させる
  - `let _ = self.session_mgr.collect_exited();` として戻り値を明示的に無視する形に変更する
  - デッドライン判定・PID ファイル削除の既存ロジックは変更しない
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [ ] 2. `is_process_alive` 自由関数を実装し `recover_on_startup` を拡張する
- [ ] 2.1 Unix シグナル 0 によるプロセス生存確認関数を実装する
  - `polling_use_case.rs` 内に `is_process_alive(pid: u32) -> bool` をモジュールレベル自由関数として追加する
  - `nix::sys::signal::kill(Pid::from_raw(pid as i32), None)` で確認し、`Ok(())` なら `true`、`Err(ESRCH)` なら `false` を返す
  - その他のエラー（`EPERM` 等）は保守的に `true` を返す
  - nix の `use` 宣言（`nix::sys::signal::{Signal, kill}` 等）を追加する（`stop_use_case.rs` の既存パターンを踏襲）
  - `#[cfg(unix)]` で Unix 専用実装にする
  - _Requirements: 2.1, 2.5_

- [ ] 2.2 `recover_on_startup` に孤児プロセス kill 処理を追加する
  - `current_pid` が `Some(pid)` の場合に `is_process_alive(pid)` を呼び出してプロセスの生存を確認する
  - プロセスが生存している場合は `kill(Pid::from_raw(pid as i32), Signal::SIGKILL)` で終了させ、`info` ログに PID と issue_id を記録する
  - SIGKILL 失敗時は `warn` ログを出力し、処理を継続する（起動を妨げない）
  - プロセスが既に存在しない場合は kill せずに `current_pid` のクリアのみ行う（既存動作を維持）
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [ ] 3. テストを追加してすべての要件を検証する
- [ ] 3.1 `graceful_shutdown` のループ判定テストを追加する
  - `collect_exited()` が最初に空を返し、2回目に count が 0 になるシナリオでループが正常終了することを検証するテストを追加する
  - モックの SessionManager を用いて `count()` の返却値をシーケンシャルに制御し、`collect_exited()` の呼び出し回数を確認する
  - デッドライン到達時に `kill_all()` が呼ばれてループが終了することを検証するテストを追加する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 3.2 (P) `is_process_alive` のユニットテストを追加する
  - 存在しない PID（例: `u32::MAX`）に対して `false` が返ることを確認するテストを追加する
  - 自プロセスの PID（`std::process::id()`）に対して `true` が返ることを確認するテストを追加する
  - _Requirements: 2.5_

- [ ] 3.3 (P) `recover_on_startup` での生存プロセス kill テストを追加する
  - `current_pid = Some(pid)` かつプロセスが生存している場合に SIGKILL が呼ばれ、`current_pid` が `None` にクリアされることを検証するテストを追加する
  - _Requirements: 2.1, 2.2_

- [ ] 3.4 (P) `recover_on_startup` での死亡済みプロセスクリアテストを追加する
  - `current_pid = Some(pid)` かつプロセスが存在しない場合に SIGKILL が呼ばれず、`current_pid` のみクリアされることを検証するテストを追加する
  - kill 失敗時でも `current_pid` のクリアが継続されることを確認するテストを追加する
  - _Requirements: 2.3, 2.4_

- [ ] 3.5 ビルドと静的解析を通過させる
  - `cargo test` で全テストがパスすることを確認する
  - `cargo clippy -- -D warnings` で警告が発生しないことを確認する
  - _Requirements: 3.4, 3.5_
