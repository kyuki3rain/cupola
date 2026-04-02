# Research & Design Decisions

---
**Purpose**: `logs-command` フィーチャーの設計調査・決定記録

**Usage**: 設計フェーズにおける調査内容および設計上のトレードオフを記録する。
---

## Summary

- **Feature**: `logs-command`
- **Discovery Scope**: Extension（既存システムへの機能追加）
- **Key Findings**:
  - `tracing_appender::rolling::daily` が生成するファイル名は `cupola.YYYY-MM-DD` 形式で、辞書順ソートで最新ファイルを特定できる
  - 既存の `Command` enum と `bootstrap/app.rs` の dispatch パターンに沿って拡張する
  - tokio の `ctrl_c()` シグナルと `tokio::select!` でフォローモードの中断を実装する

## Research Log

### ログファイル命名規則の調査

- **Context**: `cupola logs` で「最新ログファイル」を特定するためのファイル命名規則を確認
- **Sources Consulted**: `src/bootstrap/logging.rs`（コードベース内）
- **Findings**:
  - `tracing_appender::rolling::daily(dir, "cupola")` を使用
  - 生成ファイル名: `cupola.YYYY-MM-DD`（例: `cupola.2026-04-02`）
  - 日付部分が ISO 8601 形式のため、辞書順降順ソートで最新ファイルが先頭に来る
- **Implications**: ファイル名の辞書順ソートで最新ファイルを選択できる。正規表現マッチ不要。

### 既存 CLI 拡張パターンの調査

- **Context**: `Command` enum への新サブコマンド追加方法の確認
- **Sources Consulted**: `src/adapter/inbound/cli.rs`（コードベース内）
- **Findings**:
  - `#[derive(Subcommand)]` の `Command` enum に variant を追加するだけで clap が自動的にヘルプを生成
  - `--config` オプションは他コマンド（Stop, Doctor）と同一パターン（`default_value = ".cupola/cupola.toml"`）
  - `-f` / `--follow` は `#[arg(short = 'f', long)]` パターンで定義可能
  - `-n` / `--lines` は `#[arg(short = 'n', long, default_value_t = 20)]` パターンで定義可能
- **Implications**: 既存パターンに沿った実装で、学習コストなし。

### フォローモードのシグナルハンドリング

- **Context**: `Ctrl+C` で `-f` フォローモードを正常終了させる方法
- **Sources Consulted**: `src/bootstrap/app.rs`（`start_daemon` の実装パターン）、tokio 公式ドキュメント
- **Findings**:
  - tokio では `tokio::signal::ctrl_c()` で SIGINT をハンドリングできる
  - `tokio::select!` でフォローポーリングループと ctrl_c を同時待機できる
  - `CancellationToken`（`tokio_util` クレート）を使う方法もあるが、既存コードベースに依存なし
- **Implications**: `tokio::select!` と `ctrl_c()` の組み合わせが最もシンプルで既存パターンに馴染む。

### ポートトレイトのテスタビリティ設計

- **Context**: ファイルシステムアクセスのモック化によるユニットテスト実現
- **Sources Consulted**: `src/application/stop_use_case.rs`（`SignalPort` / `MockSignal` パターン）
- **Findings**:
  - `SignalPort` trait + `NixSignalSender` 実装 + `MockSignal` のパターンが確立済み
  - Use case は型パラメータ `<P: SomePort>` で注入を受け、テスト時はモックに差し替え
  - 実装例: `LogsUseCase<P: LogFilePort>` + `FsLogFilePort`（本番）+ `MockLogFilePort`（テスト）
- **Implications**: 同パターンで `LogFilePort` trait を定義し、`FsLogFilePort` で実装する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| シンプルポーリング (100ms) | `tokio::time::sleep` で定期ファイル読み取り | tokio のみ依存、クロスプラットフォーム、実装シンプル | 100ms レイテンシが生じる | ログ追跡用途では許容範囲内 |
| OS イベント通知 (`notify` クレート) | inotify / kqueue によるファイル変更検知 | リアルタイム性高い | 新規クレート依存、OS ごとの挙動差異 | オーバーエンジニアリングになる |

→ **シンプルポーリング（100ms）** を採用。ログ追跡用途では十分であり、既存依存のみで実装可能。

## Design Decisions

### Decision: `LogFilePort` の責務範囲

- **Context**: ファイル操作をどの粒度でポート化するか
- **Alternatives Considered**:
  1. ファイル読み込み全体を1メソッド（`read_tail + read_from_offset` を分割しない）
  2. `find_latest` / `read_tail` / `read_new_content` の3メソッドに分割
- **Selected Approach**: 3メソッド分割（`find_latest_log_file`, `read_tail`, `read_new_content`）
- **Rationale**: 各メソッドが単一責務を持ち、テスト時のモック実装が容易になる
- **Trade-offs**: インターフェースが若干増えるが、テスタビリティが向上する
- **Follow-up**: `read_new_content` は `(offset: u64) -> Result<(Vec<String>, u64), LogFileError>` のシグネチャで offset を返す

### Decision: フォローモードのシグナル処理の配置

- **Context**: `LogsUseCase` が直接 `tokio::signal` を呼ぶか、bootstrap 層で処理するか
- **Alternatives Considered**:
  1. `LogsUseCase` 内で `tokio::select!(ctrl_c => break)` を実行
  2. bootstrap 層で `ctrl_c` を受け取り、use case に cancellation を伝達
- **Selected Approach**: Option 1（use case 内で直接処理）
- **Rationale**: `StopUseCase` が `nix::signal` を直接使わず `SignalPort` で抽象化しているのに対し、`ctrl_c()` はターミナル操作に密接であり、ユースケースレベルで処理するほうがシンプル。テスト時は follow=false でのパスをカバー。
- **Trade-offs**: use case が tokio::signal に直接依存するが、ログ表示ユースケースとして許容範囲
- **Follow-up**: 単体テストは `follow=false` パスのみカバー、統合テストで follow 動作を検証

## Risks & Mitigations

- ログディレクトリのファイル数が多い場合にディレクトリ読み取りが遅くなる可能性 — 最新ファイル特定のみでリスト全体をソートする範囲を最小化する（`max_by` で1回走査）
- フォローモード中にデーモンがファイルをローテートした場合（翌日に日付変わりで新ファイル生成）— 現実装では既存のファイルを追跡し続ける（スコープ外; 将来改善）

## References

- `src/bootstrap/logging.rs` — `tracing_appender::rolling::daily` の使用箇所
- `src/adapter/inbound/cli.rs` — 既存 `Command` enum と clap パターン
- `src/application/stop_use_case.rs` — `SignalPort` / mock injection パターン
- `src/bootstrap/app.rs` — command dispatch と DI wiring パターン
