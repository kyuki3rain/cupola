# 実装タスク

## タスク概要

panic 時の graceful exit 実装を 4 つの主要タスクに分割する。タスク 1 が基盤（ChildProcessRegistry）を提供し、タスク 2・3 は基盤の上に並行して実装できる。タスク 4（テスト）は全実装完了後に実施する。

---

- [ ] 1. ChildProcessRegistry の実装と SessionManager への統合

- [ ] 1.1 ChildProcessRegistry 型を実装する
  - `src/application/session_manager.rs` に `ChildProcessRegistry` 構造体を追加する
  - `Arc<Mutex<HashSet<u32>>>` で内部状態を保持し、`Clone` を派生させる
  - `new()` / `register(pid: u32)` / `unregister(pid: u32)` メソッドを実装する
  - `shutdown_sync(sigterm_timeout: Duration)` を実装する：全 PID に SIGTERM → 100ms ポーリングで生存確認 → タイムアウト後に SIGKILL
  - `shutdown_sync` 内で Mutex が毒化している場合は `PoisonError::into_inner` でデータを回収して処理を続行する
  - 全操作でエラーは best-effort（`let _ = ...`）で無視する
  - ユニットテストを追加：register/unregister が HashSet を正しく更新すること
  - _Requirements: 1.1, 1.4, 2.1, 2.2, 2.3, 2.4, 2.5_

- [ ] 1.2 SessionManager に ChildProcessRegistry を統合する
  - `SessionManager` に `registry: Option<ChildProcessRegistry>` フィールドを追加する
  - `with_registry(registry: ChildProcessRegistry) -> Self` builder メソッドを追加する
  - `register()` で `registry.register(child.id())` を呼ぶよう変更する
  - `collect_exited()` で回収した各 PID を `registry.unregister(pid)` で削除する
  - `kill(issue_id)` 実行時、対象 PID を unregister する（kill 前に PID を取得して保存する）
  - `kill_all()` 実行時、全 PID を収集してから kill し、その後 unregister する
  - `registry` が `None` のときは既存動作と同一であることをユニットテストで確認する
  - `with_registry` を使った場合に register/collect_exited/kill/kill_all が registry を正しく更新することをユニットテストで確認する
  - _Requirements: 1.2, 1.3, 1.5_

- [ ] 2. (P) PollingUseCase への registry 転送を実装する
  - `PollingUseCase` に `with_child_registry(registry: ChildProcessRegistry) -> Self` builder メソッドを追加する
  - 内部実装は `self.session_mgr = self.session_mgr.with_registry(registry)` のみ
  - タスク 1 完了後に着手可能
  - _Requirements: 4.3_

- [ ] 3. (P) install_panic_hook を拡張する
  - `install_panic_hook` のシグネチャを `(pid_path: PathBuf, child_registry: ChildProcessRegistry)` に変更する
  - フック内の処理順序を「子プロセスシャットダウン → PID ファイル削除 → tracing::error! → 前のフック呼び出し」に更新する
  - `child_registry.shutdown_sync(Duration::from_secs(3))` を PID ファイル削除より前に呼ぶ
  - `start_foreground` で `ChildProcessRegistry::new()` を作成し、`install_panic_hook(pid_path.clone(), registry.clone())` に更新する
  - `start_daemon_child` で同様に更新する
  - 両関数で `PollingUseCase::with_child_registry(registry)` の呼び出しを追加する
  - タスク 1 完了後に着手可能（タスク 2 と並行実施可）
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.4_

- [ ] 4. テストを実装する

- [ ] 4.1 (P) ChildProcessRegistry と SessionManager のユニットテスト（拡張）
  - `shutdown_sync` のテスト: `sleep` コマンドを子プロセスとして起動し、registry に登録後に `shutdown_sync` を呼んで SIGTERM で終了することを確認する
  - `shutdown_sync` の SIGKILL テスト: SIGTERM を無視するプロセス（存在すれば）または `shutdown_sync` を短いタイムアウトで呼んで SIGKILL が送られることを確認する
  - registry が空の状態で `shutdown_sync` を呼んでもパニックしないことを確認する
  - _Requirements: 5.5_

- [ ] 4.2 (P) panic hook の統合テスト
  - `tests/` ディレクトリに panic hook シナリオのテストを追加する
  - テスト手順：(a) モック子プロセス（`sleep`）を起動し PID を取得する、(b) registry に PID を登録する、(c) panic hook クロージャを直接呼び出す（`std::panic::catch_unwind` 外から）、(d) `kill(pid, 0)` でプロセスが消えていることを確認する
  - PID ファイルが削除されていることを確認する
  - テスト全体が `SIGTERM_TIMEOUT (3 秒) + マージン (2 秒)` 以内に完了することを確認する
  - Mutex 毒化シナリオ（別スレッドで意図的に Mutex を毒化した後に hook を呼ぶ）でも子プロセスが回収されることを確認する
  - _Requirements: 5.1, 5.2, 5.3, 5.4_
