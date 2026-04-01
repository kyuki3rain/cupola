# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary

- **Feature**: `logs-command`
- **Discovery Scope**: Extension（既存 CLI への新サブコマンド追加）
- **Key Findings**:
  - `tracing-appender` の日次ローリングファイルは `{prefix}.{YYYY-MM-DD}` の命名規則を採用しており、辞書順ソートで最新ファイルを特定できる
  - tokio はすでに依存関係に含まれており、`tokio::signal::ctrl_c()` で SIGINT を安全に捕捉できる
  - 追加の外部クレートなしに polling ベースの `-f` 実装が可能（`tokio::time::sleep` + `tokio::fs`）

---

## Research Log

### CLI 拡張ポイントの調査

- **Context**: 既存 `Command` enum に `Logs` バリアントを追加する際の影響範囲を把握するため
- **Sources Consulted**: `src/adapter/inbound/cli.rs`（既存実装）
- **Findings**:
  - `Command` enum は clap の `#[derive(Subcommand)]` で定義されており、新バリアントの追加はパターンマッチの網羅性チェック（exhaustive match）によりコンパイルエラーで漏れを検出できる
  - `start` / `stop` / `doctor` コマンドはいずれも `config: PathBuf` を持ち、デフォルト値は `.cupola/cupola.toml`
  - `logs` コマンドも同様に config パスを受け取る設計が自然
- **Implications**: `app.rs` の `match cli.command` に `Command::Logs` ブランチを追加するだけで拡張できる

### ログファイル命名規則の調査

- **Context**: `log_dir` 内の「最新ログファイル」を定義するため
- **Sources Consulted**: `src/bootstrap/logging.rs`（`tracing-appender::rolling::daily` の使用箇所）
- **Findings**:
  - `tracing-appender::rolling::daily` は `{filename_prefix}.{YYYY-MM-DD}` の形式でファイルを作成する
  - プレフィックスは `"cupola"` を使用（`logging.rs` で確認済み）
  - ISO 8601 形式の日付サフィックスにより、辞書順ソート（`std::cmp::Ord`）で最新ファイルを特定可能
- **Implications**: ファイル選択ロジックは `read_dir` → フィルタ（プレフィックス `"cupola."` で始まる） → 辞書順で最後の要素を選択

### ファイル追跡（-f オプション）の実装方式評価

- **Context**: `tail -f` 相当のリアルタイム追跡をどう実装するか
- **Sources Consulted**: Cargo.toml（既存依存関係）、tokio ドキュメント
- **Findings**:
  - `notify` クレートは inotify/kqueue/FSEvents を抽象化するが、新規依存関係の追加が必要
  - tokio 1.x の `tokio::fs::File` はファイルの seek + read の非同期操作をサポート
  - `tokio::signal::ctrl_c()` が SIGINT を捕捉できる（Unix/Windows 両対応）
  - polling 間隔 200ms 程度であれば CPU 負荷は無視できる
- **Implications**: 既存依存のみで実装可能。新規クレート追加不要

---

## Architecture Pattern Evaluation

| オプション | 説明 | 利点 | 制限 | 判定 |
|-----------|------|------|------|------|
| polling（tokio::time::sleep） | 一定間隔でファイルを再読み込み | 依存追加なし、シンプル | 最大 200ms の表示遅延 | **採用** |
| notify クレート | FS イベント駆動の通知 | リアルタイム性が高い | 新規依存、クロスプラットフォーム差異 | 不採用 |
| tokio::fs::watch（非存在） | tokio ネイティブ watch | 統合的 | tokio に watch API なし | 非対応 |

---

## Design Decisions

### Decision: polling ベースの -f 実装

- **Context**: `tail -f` 相当のリアルタイム追跡方式の選択
- **Alternatives Considered**:
  1. `notify` クレート — inotify/kqueue ベースのイベント駆動
  2. polling — `tokio::time::sleep` + ファイル再読み込み
- **Selected Approach**: polling（200ms 間隔）
- **Rationale**: 既存依存のみで実装可能。デーモンログの追跡という用途では 200ms 遅延は許容範囲内
- **Trade-offs**: リアルタイム性がやや低い vs 実装シンプルさ・依存なし
- **Follow-up**: 将来的に `notify` への移行も可能（同一インターフェースで差し替え可）

### Decision: ログファイル配置を bootstrap 層に

- **Context**: ログ表示ロジックをどの層に配置するか
- **Alternatives Considered**:
  1. `app.rs` にインライン実装
  2. `src/bootstrap/logs_command.rs` として独立モジュール化
- **Selected Approach**: `src/bootstrap/logs_command.rs` として独立モジュール
- **Rationale**: `app.rs` は既に各コマンドのルーティングで肥大化しやすい。単一責任原則に従い分離する
- **Trade-offs**: ファイル数増加 vs コードの見通しの良さ

### Decision: config パスを Logs コマンドに含める

- **Context**: `log_dir` を取得するために config ファイルを参照する必要がある
- **Alternatives Considered**:
  1. `--config` オプションで明示的に渡す（他コマンドと統一）
  2. `--log-dir` オプションで直接指定させる
- **Selected Approach**: `--config` オプション（デフォルト `.cupola/cupola.toml`）
- **Rationale**: 既存コマンドとの一貫性。config から log.dir を読む設計がシンプル
- **Trade-offs**: config ファイルなしでは動作不可 vs 設計の一貫性

---

## Risks & Mitigations

- **ログファイルが存在しない** — `log_dir` にファイルがない場合は明確なエラーメッセージを表示して終了
- **ログファイルが増加中に -f で切り替わる可能性** — 今回のスコープでは日付境界でのファイルローテーション追跡は対象外。最新ファイルを起動時に特定してそれを追跡し続ける
- **大容量ログファイルでの tail 表示** — 末尾 20 行のみ読み込むため問題なし

---

## References

- `src/bootstrap/logging.rs` — tracing-appender 使用箇所、ファイル命名規則の確認
- `src/adapter/inbound/cli.rs` — 既存 Command enum 構造
- `src/bootstrap/app.rs` — コマンドルーティングパターン
- `Cargo.toml` — 既存依存関係の確認
