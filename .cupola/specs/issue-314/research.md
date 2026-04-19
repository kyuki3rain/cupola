# リサーチ & 設計決定記録

---
**Feature**: issue-314 — panic 時の graceful exit (panic hook)
**Discovery Scope**: Extension（既存システムへの機能拡張）
**Key Findings**:
- `install_panic_hook` は既に PID ファイル削除とログ記録を行っているが、子プロセスのシャットダウンが欠落している
- `SessionManager` は `std::process::Child` を保持しており、panic hook（`'static` クロージャ）から直接参照できない
- `Arc<Mutex<HashSet<u32>>>` を使った PID 共有レジストリで安全に解決できる
- `std::sync::Mutex` の毒化は `PoisonError::into_inner` で回復可能であり、panic hook 内で安全に処理できる

---

## リサーチログ

### 既存 panic hook の実装分析

- **Context**: 現在の `install_panic_hook`（`src/bootstrap/app.rs:862-900`）の機能を把握する
- **Findings**:
  - `std::panic::take_hook()` で前のフックをキャプチャし、`set_hook` でチェーンする実装
  - PID ファイルを best-effort で削除（エラーは無視）
  - `tracing::error!` でパニックメッセージと位置を記録
  - **子プロセスシャットダウンは行っていない**
- **Implications**: 既存フックへの最小限の追加でよい。PID 削除より前に子プロセスシャットダウンを挿入する

### SessionManager のライフサイクル分析

- **Context**: panic hook（`'static` クロージャ）から SessionManager にアクセスする方法を探る
- **Findings**:
  - `SessionManager` は `PollingUseCase` フィールド `session_mgr: SessionManager` として保持される（`src/application/polling_use_case.rs:121`）
  - `polling.run()` はミュータブル参照でセッション管理を行う
  - `Child` は `Send` ではないためそのまま `Arc<Mutex<>>` に包めない（`Child` は `!Sync`）
  - **直接共有は不可能** → PID のみを別途共有する設計が必要
- **Implications**: `ChildProcessRegistry`（`Arc<Mutex<HashSet<u32>>>`）を SessionManager の外に切り出し、SessionManager はそれを副作用として更新する

### Mutex 毒化と panic hook の安全性

- **Context**: `Arc<Mutex<HashSet<u32>>>` が panic hook 実行時に毒化している場合の処理
- **Findings**:
  - Rust の `Mutex::lock()` は毒化時に `Err(PoisonError)` を返す
  - `PoisonError::into_inner()` で内部データを回収できる
  - panic hook は **パニックしたスレッドで実行される**。パニックしたスレッドが `ChildProcessRegistry` の Mutex を保持していた場合、`lock()` はデッドロックする（Mutex は再入不可）
  - **ただし**: `ChildProcessRegistry` の Mutex は `SessionManager` の操作とは独立している（SessionManager は `sessions: HashMap<i64, SessionEntry>` を直接フィールドとして保持）。パニックしたスレッドが `ChildProcessRegistry` の Mutex を保持するシナリオは存在しない（polling loop は SessionManager に直接アクセスするため）
  - `try_lock()` を使えばデッドロックリスクをさらに下げられるが、毒化時のデータ回収には `lock()` + `PoisonError::into_inner()` の方が適切
- **Implications**: panic hook 内では `lock()` して毒化の場合は `into_inner()` で回収するパターンを採用。`try_lock()` は失敗時にシャットダウンをスキップしてしまうためより危険

### 同期シャットダウン実装の選択肢

| アプローチ | 説明 | 利点 | 欠点 |
|-----------|------|------|------|
| A: SessionManager に sync API | `shutdown_all_sync(&self, timeout)` を SessionManager に追加 | 凝集性が高い | `Child` は `!Send`/`!Sync` のため `Arc<Mutex<SessionManager>>` でスレッド共有が困難 |
| B: PID 専用レジストリ | `Arc<Mutex<HashSet<u32>>>` を SessionManager の外に置く | スレッドセーフで panic hook から安全にアクセス可能 | SessionManager への副作用更新が必要 |
| C: グローバル静的 Mutex | `static PIDS: Mutex<HashSet<u32>>` | シンプル | テストが困難、複数インスタンス不可 |

**選択**: **アプローチ B** — `ChildProcessRegistry` 型を `Arc<Mutex<HashSet<u32>>>` のラッパーとして実装し、`Clone` を活かして bootstrap から panic hook と SessionManager の両方に渡す

### SIGTERM → SIGKILL シーケンスの実装

- **Findings**:
  - 既存の `NixSignalSender` (`src/adapter/outbound/nix_signal_sender.rs`) は `nix` クレートを使って SIGTERM/SIGKILL を送る
  - `nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGTERM)` で SIGTERM 送信
  - `nix::sys::signal::kill(Pid::from_raw(pid), None)` でプロセスの生存確認（kill(pid, 0)）
  - `std::thread::sleep` でポーリング間隔を待つ（`Duration::from_millis(100)` 程度）
  - 既存の `stop_use_case.rs` が同様のパターンを実装しているため参考にできる
- **Implications**: `ChildProcessRegistry::shutdown_sync` 内で `nix` クレートを直接使用。`SignalPort` トレイトは async トレイトのため panic hook では使えない

### PollingUseCase のビルダーパターン

- **Findings**:
  - `PollingUseCase` は `with_process_repo` と `with_pid_file` の builder メソッドを持つ（`src/application/polling_use_case.rs`）
  - `SessionManager::new()` はデフォルトで `PollingUseCase::new` 内部で生成される
  - 同様の `with_child_registry(registry: ChildProcessRegistry)` を追加し、SessionManager に転送するパターンが既存コードに合致する
- **Implications**: bootstrap の変更は最小限。`with_child_registry` 呼び出しを一行追加するだけ

### 統合テストのアプローチ

- **Findings**:
  - 既存の統合テスト (`tests/integration_test.rs`) は mock adapter を使って use case を検証
  - panic hook のテストは通常の非同期テストでは困難（panic はプロセス終了を伴う）
  - `std::panic::catch_unwind` を使えば panic を捕捉できるが、panic hook は `catch_unwind` を使っても実行される
  - 最もシンプルなアプローチ: panic hook 関数のユニットテストで、モックの子プロセスを登録した registry を持つフックを呼び出し、プロセスが消えることを確認する
  - SessionManager のユニットテストで `ChildProcessRegistry` との統合（PID の登録・削除）を検証
- **Implications**: 統合テストは「フック単体テスト + SessionManager ユニットテスト」の組み合わせで十分。真のデーモン起動テストはオーバーエンジニアリング

## 設計決定

### 決定: ChildProcessRegistry の配置場所

- **Context**: 新しい型をどのモジュールに置くか
- **Alternatives Considered**:
  1. `src/application/session_manager.rs` に同居 — SessionManager との密接な関係を表現
  2. `src/application/child_process_registry.rs` として独立 — 単一ファイル単一責任
  3. `src/bootstrap/app.rs` に置く — bootstrap 専用だが application 層が参照できない
- **Selected Approach**: `src/application/session_manager.rs` に同居（`pub struct ChildProcessRegistry` として定義）
- **Rationale**: SessionManager と密接に連携し、依存関係は application 層内で完結する。独立ファイルにするほど大きくない
- **Trade-offs**: session_manager.rs が少し大きくなるが、関連ロジックが一箇所にまとまる

### 決定: shutdown_sync の配置

- **Context**: SIGTERM → SIGKILL シーケンスのロジックをどこに置くか
- **Alternatives Considered**:
  1. `ChildProcessRegistry::shutdown_sync` メソッド — registry が自己完結
  2. `bootstrap/app.rs` のフリー関数 — bootstrap のロジックとして
  3. `session_manager.rs` のフリー関数 — session_manager に同居
- **Selected Approach**: `ChildProcessRegistry::shutdown_sync` メソッド
- **Rationale**: registry が「自分の PID に対して何をするか」を知るのが自然。`nix` クレートは既存 adapter 層で使われており、application 層での使用も問題ない（cross-cutting）
- **Trade-offs**: application 層に nix 依存が追加されるが、既に nix はワークスペース依存

## リスクと対策

- **シャットダウン中の新規登録**: panic hook 実行中に新たな `register` が呼ばれる可能性は tokio runtime 停止後はないため、実質的に問題なし
- **PID 再利用**: SIGTERM 後に別のプロセスが同じ PID を取得するレースコンディション。タイムアウト (3 秒) が短いため実用上問題は発生しない
- **ロギングの初期化前 panic**: `tracing::error!` はロガーが未初期化でも no-op になるため安全。PID ファイル削除は tracing より先に実行する

## 参照

- `src/bootstrap/app.rs:850-912` — 既存の `install_panic_hook` と `apply_pid_cleanup`
- `src/application/session_manager.rs` — SessionManager の全実装
- `src/adapter/outbound/nix_signal_sender.rs` — SIGTERM/SIGKILL 実装参考
- `src/application/stop_use_case.rs` — SIGTERM → SIGKILL タイムアウトパターン参考
- Rust Reference: `std::panic::set_hook`, `std::sync::PoisonError`
