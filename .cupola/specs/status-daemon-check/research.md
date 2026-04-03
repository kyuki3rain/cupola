# Research & Design Decisions

---
**Purpose**: Discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary

- **Feature**: `status-daemon-check`
- **Discovery Scope**: Extension（既存statusコマンドへの拡張）
- **Key Findings**:
  - `PidFileManager` は `app.rs` に既にインポート済みであり、再利用可能
  - `Command::Status` は現在 `config` パラメータを持たないが、DB パスと同様に `.cupola/cupola.pid` をハードコードする方針で整合性が取れる
  - `Issue` ドメインエンティティの `current_pid: Option<u32>` フィールドが既に存在し、プロセス生存確認に直接利用できる

## Research Log

### 既存 PidFileManager の調査

- **Context**: daemon 状態表示に `PidFileManager` を再利用できるか確認
- **Sources Consulted**: `src/adapter/outbound/pid_file_manager.rs`、`src/application/port/pid_file.rs`
- **Findings**:
  - `PidFilePort` トレイトに `read_pid()`, `delete_pid()`, `is_process_alive()` が定義済み
  - `is_process_alive()` は `kill(pid, Signal 0)` で生存確認を行い、`EPERM` も alive として扱う
  - `PidFileManager::new(PathBuf)` でインスタンス化し、パスは呼び出し元で指定
- **Implications**: 新しいコードを書かずに既存の実装を利用できる

### Command::Status の既存実装調査

- **Context**: 変更箇所を特定し、影響範囲を把握
- **Sources Consulted**: `src/bootstrap/app.rs` (l.270-333)
- **Findings**:
  - 現在の Running カウントは `i.state.needs_process() && i.current_pid.is_some()` で判定している（プロセスの実際の生存は未確認）
  - `Command::Stop` では `config_dir.join("cupola.pid")` でPIDファイルパスを構築
  - `Command::Status` ではDBパス (`.cupola/cupola.db`) をハードコードしている
  - `PidFileManager` は `app.rs` に既にインポート済み
- **Implications**: PIDファイルパスも `.cupola/cupola.pid` でハードコードするのが一貫性のある設計

### PIDファイルパス決定

- **Context**: `Command::Status` に `config` パラメータを追加するか、ハードコードするかの判断
- **Sources Consulted**: `src/adapter/inbound/cli.rs`、`src/bootstrap/app.rs`
- **Findings**:
  - `Command::Status` は現在 `Status` のみでパラメータなし
  - `Command::Stop` は `config: PathBuf` パラメータを受け取り、そこからconfigディレクトリを導出
  - `.cupola/cupola.db` がハードコードされているのと同様に `.cupola/cupola.pid` もハードコードして整合性を保つ
- **Implications**: 変更を `Command::Status` のアームに限定できる。CLIシグネチャの変更不要

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| ハードコードPIDパス | `.cupola/cupola.pid` を直接使用 | 既存の `.cupola/cupola.db` パターンと一致、CLIシグネチャ変更不要 | config ディレクトリを変更している環境では動作しない | 現状の `status` コマンドの設計と一致 |
| config パラメータ追加 | `Command::Status` に `config: PathBuf` 追加 | 設定ディレクトリのカスタマイズに対応 | CLIの後方互換性に影響、変更範囲が広がる | 現在の `status` コマンドの設計思想と合わない |

**選択**: ハードコードPIDパス（`.cupola/cupola.pid`）

## Design Decisions

### Decision: PIDファイルパスの決定方法

- **Context**: `Command::Status` でdaemon PIDファイルのパスをどう取得するか
- **Alternatives Considered**:
  1. `config` パラメータ追加 — `Command::Stop` と同様の方式
  2. ハードコード — `.cupola/cupola.pid` を直接使用
- **Selected Approach**: ハードコード（`.cupola/cupola.pid`）
- **Rationale**: 現在の `Command::Status` はDBパス (`.cupola/cupola.db`) をハードコードしており、同じパターンに従う。CLIシグネチャの変更を避けることで後方互換性を維持できる
- **Trade-offs**: 標準以外のconfigディレクトリには対応しないが、現在のユースケースでは問題ない
- **Follow-up**: 将来的にconfig管理が統一される際に合わせてリファクタリングを検討

### Decision: stale PIDファイルの削除タイミング

- **Context**: stale PIDファイルを検出した際に自動削除するか、表示のみにするか
- **Alternatives Considered**:
  1. 自動削除 — Issueの仕様通りに削除と通知メッセージ表示
  2. 表示のみ — ファイルは残し、警告メッセージのみ表示
- **Selected Approach**: 自動削除（仕様通り）
- **Rationale**: `Daemon: not running (stale PID file cleaned)` という表示形式がIssueで明示されている
- **Trade-offs**: 削除は副作用を伴うが、stale PIDファイルの放置は次回起動時の誤検知につながるリスクがある
- **Follow-up**: 削除エラー（例: パーミッション不足）時の挙動を実装時に確認

## Risks & Mitigations

- PIDファイル削除失敗（パーミッションエラー等）— エラーを無視して `not running` として扱う、ログ出力で可視化
- プロセス生存確認の競合状態（確認直後にプロセスが終了）— 許容範囲内。瞬間的なレースコンディションの影響は軽微
- `current_pid` がDBに残ったまま `dead` となっているissueが多数存在する場合のパフォーマンス — signal 0 は軽量な操作であり問題なし

## References

- `src/application/port/pid_file.rs` — `PidFilePort` トレイト定義
- `src/adapter/outbound/pid_file_manager.rs` — `PidFileManager` 実装（is_process_alive等）
- `src/bootstrap/app.rs` (l.44-60) — `Command::Stop` での `PidFileManager` 利用パターン
- `src/bootstrap/app.rs` (l.270-333) — 現在の `Command::Status` 実装
