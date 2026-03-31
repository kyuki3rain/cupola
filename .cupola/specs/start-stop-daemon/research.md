# Research & Design Decisions

---
**Purpose**: start-stop-daemon フィーチャーの調査・設計根拠を記録する。

---

## Summary

- **Feature**: `start-stop-daemon`
- **Discovery Scope**: Extension（既存システムへの拡張）
- **Key Findings**:
  - `tokio::signal::unix` は `tokio = { features = ["full"] }` に含まれており、新規依存なしで SIGTERM 受信が可能
  - デーモン化（fork + setsid）には `nix` クレートが必要。現状 Cargo.toml に未追加
  - `graceful_shutdown()` はすでに `PollingUseCase` に実装済み。PID ファイル削除をここに統合できる
  - `.gitignore` 管理は `InitFileGenerator::append_gitignore_entries()` が担っており、`GITIGNORE_ENTRIES` 定数に `.cupola/cupola.pid` を追加するだけでよい

## Research Log

### SIGTERM ハンドリング

- **Context**: デーモンは `Ctrl+C` を受信しないため SIGTERM での終了が必要
- **Sources Consulted**: tokio ドキュメント（`tokio::signal::unix`）
- **Findings**:
  - `tokio::signal::unix::signal(SignalKind::terminate())` で SIGTERM を非同期待機できる
  - `tokio = { features = ["full"] }` に含まれるため追加依存不要
  - 現在の polling ループは `signal::ctrl_c()` のみ待機しており、SIGTERM を追加するだけでよい
- **Implications**: `PollingUseCase::run()` の `tokio::select!` ブランチに SIGTERM 受信を追加する

### デーモン化（fork / setsid）

- **Context**: `cupola start -d` でターミナル非依存のバックグラウンドプロセスを起動する
- **Sources Consulted**: Unix デーモン化ベストプラクティス、`nix` クレート docs
- **Findings**:
  - Unix デーモン化の標準手順: `fork()` → 親終了 → 子で `setsid()` → stdin を `/dev/null` にリダイレクト
  - `nix::unistd::fork()` と `nix::unistd::setsid()` が最も直接的
  - `std::process::Command` で自分自身を `--daemon-child` フラグ付きで再実行する方法もあるが、引数管理が煩雑
  - `daemonize` クレート（0.5.x）は一行で済むが、追加依存となりメンテナンス状況に懸念
- **Implications**: `nix` クレートを新規追加し、`bootstrap/app.rs` で fork を実行する

### SIGTERM 送信（cupola stop）

- **Context**: PID ファイルから読んだプロセスに SIGTERM を送る
- **Sources Consulted**: `nix::sys::signal` docs、POSIX kill(2)
- **Findings**:
  - `nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGTERM)` が最も型安全
  - `std::process::Command::new("kill").arg("-TERM").arg(pid)` でも可能だが外部プロセス依存
  - `kill(pid, 0)` でプロセス生存確認ができる（`nix::sys::signal::kill(pid, None)` 相当）
- **Implications**: `nix` を採用し、PID ファイル管理アダプターから利用する

### PID ファイルの配置と競合状態

- **Context**: デーモン二重起動防止のために PID ファイルを使う
- **Findings**:
  - PID ファイルのパスは設定ファイル（`--config`）が指す `.cupola/` ディレクトリに依存
  - 競合状態（PID ファイル存在チェック → 書き込みの間に別プロセス起動）は許容範囲内（ユーザー操作のため）
  - プロセス異常終了時に残留するステールな PID ファイルは、次回起動時に上書き（プロセス生死確認後）
- **Implications**: PID ファイルパスは `config` から解決。初期化時にディレクトリ作成を保証する

### .gitignore 統合

- **Context**: `cupola.pid` がコミットされるのを防ぐ
- **Findings**:
  - `src/adapter/outbound/init_file_generator.rs` の `GITIGNORE_ENTRIES` 定数に `.cupola/cupola.pid` を追加するだけでよい
  - `cupola init` 実行時に自動で `.gitignore` に追記される
- **Implications**: `InitFileGenerator` の定数変更のみ。新規コンポーネント不要

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク | 備考 |
|-----------|------|------|--------|------|
| nix::fork | Unix ネイティブ fork + setsid | 軽量・標準的 | Unix 専用（Windows 非対応） | 現プロジェクトは macOS/Linux 専用のため問題なし |
| std::Command 再実行 | --daemon-child フラグ付きで自身を再起動 | 追加依存なし | 引数の完全な引き渡しが複雑 | 実装が煩雑になる |
| daemonize クレート | `Daemonize::new().start()` の一行 | シンプル | 追加依存・メンテ状況不明 | nix で十分なため採用見送り |

## Design Decisions

### Decision: `nix` クレートの採用

- **Context**: デーモン化（fork/setsid）と SIGTERM 送信に Unix システムコールが必要
- **Alternatives Considered**:
  1. `nix` クレート — Rust の Unix システムコールラッパー（型安全）
  2. `std::process::Command("kill")` — 外部コマンド経由でシグナル送信
  3. `libc::kill` — C バインディング（unsafe が必要）
- **Selected Approach**: `nix` クレートを `[target.'cfg(unix)'.dependencies]` で条件付き追加
- **Rationale**: 型安全な API、`unsafe` 不要、`fork()`/`setsid()`/`kill()` を一貫して提供
- **Trade-offs**: 追加依存となるが、`nix` は Rust エコシステムで標準的な Unix バインディング
- **Follow-up**: Windows 対応は不要（プロジェクトは Unix 専用）

### Decision: PID ファイル管理をアダプター層に配置

- **Context**: PID ファイルは外部 I/O（ファイルシステム）なのでアダプター層が適切
- **Alternatives Considered**:
  1. アダプター層（`adapter/outbound/pid_file_manager.rs`）
  2. ブートストラップ層（`app.rs` に直接記述）
- **Selected Approach**: `PidFilePort` トレイトを `application/port/` に定義し、`PidFileManager` を `adapter/outbound/` に実装
- **Rationale**: Clean Architecture の遵守。`StopUseCase` がポートに依存することでテスト容易性を確保
- **Trade-offs**: 小規模なのにレイヤーが増えるが、既存パターンとの一貫性を優先

### Decision: PollingUseCase への SIGTERM 統合

- **Context**: デーモンプロセスは SIGTERM で停止する必要があるが、現在は SIGINT のみ対応
- **Alternatives Considered**:
  1. `PollingUseCase::run()` に SIGTERM ハンドリングを追加
  2. bootstrap 層で SIGTERM をキャッチし、チャンネル経由で use case に通知
- **Selected Approach**: `PollingUseCase::run()` の `tokio::select!` に `signal::unix::signal(SignalKind::terminate())` ブランチを追加
- **Rationale**: 最小変更でシンプル。SIGTERM と SIGINT を同等に扱う（graceful shutdown を共有）
- **Trade-offs**: use case が Unix シグナルに依存するが、tokio 経由なので抽象化されている

### Decision: PID ファイル削除のタイミング

- **Context**: プロセス終了時に確実に PID ファイルを削除する必要がある
- **Selected Approach**: `PollingUseCase::graceful_shutdown()` の末尾で PID ファイルを削除。daemon モード時のみ有効
- **Rationale**: graceful_shutdown は SIGTERM/SIGINT どちらの経路でも呼ばれるため、削除漏れが防げる
- **Trade-offs**: SIGKILL で強制終了された場合はステールファイルが残るが、次回起動時に自動処理される

## Risks & Mitigations

- **ステール PID ファイル** — 次回起動時にプロセス生死確認後に上書きすることで対処
- **fork 後のロック競合（SQLite）** — 親プロセスは fork 後すぐに exit する。子プロセスが DB を開く前に親の接続が閉じられるため問題なし
- **double-fork の省略** — 単純な fork（setsid あり）で十分。セッションリーダーから制御端末を再割当てされるリスクは実用上無視できる

## References

- [tokio::signal::unix - tokio docs](https://docs.rs/tokio/latest/tokio/signal/unix/index.html)
- [nix::unistd::fork - nix docs](https://docs.rs/nix/latest/nix/unistd/fn.fork.html)
- [nix::sys::signal::kill - nix docs](https://docs.rs/nix/latest/nix/sys/signal/fn.kill.html)
- [POSIX Daemon Programming](https://www.freedesktop.org/software/systemd/man/daemon.html)
