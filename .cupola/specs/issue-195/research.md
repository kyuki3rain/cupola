# Research & Design Decisions

---
**Purpose**: `cupola cleanup` デーモン稼働チェック修正における調査結果と設計判断の記録

---

## Summary

- **Feature**: `issue-195` — `cupola cleanup` のデーモン稼働中エラー終了修正
- **Discovery Scope**: Extension（既存システムへの最小限の修正）
- **Key Findings**:
  - 必要なインフラストラクチャ（`PidFilePort` トレイト・`PidFileManager` 実装）はすでに完備されている
  - `status` コマンドに正しいパターンの参照実装が存在する
  - 修正対象は `src/bootstrap/app.rs` の `Command::Cleanup` ハンドラのみ
  - 新規依存クレートの追加は不要

## Research Log

### 既存インフラストラクチャの確認

- **Context**: デーモン稼働チェックに必要な機能が既存コードに存在するか確認
- **Sources Consulted**: `src/adapter/outbound/pid_file_manager.rs`, `src/application/port/pid_file.rs`
- **Findings**:
  - `PidFilePort` トレイトに `read_pid() -> Result<Option<u32>, PidFileError>` および `is_process_alive(pid: u32) -> bool` が定義済み
  - `PidFileManager` が `nix::sys::signal::kill(pid, 0)` を使用してプロセス生存確認を実装済み
  - PID 0 および `i32::MAX` 超の値はバリデーションで弾かれる
  - `EPERM` エラーは「プロセスが存在するが権限がない」として生存扱いにする実装済み
- **Implications**: 新規実装は不要。`PidFileManager` をインスタンス化して既存メソッドを呼ぶだけでよい

### 参照実装（status コマンド）の確認

- **Context**: `status` コマンドが正しいデーモンチェックパターンを実装しているか確認
- **Sources Consulted**: `src/bootstrap/app.rs:672-708`（`handle_status` 関数）
- **Findings**:
  - `handle_status` 関数が `PidFilePort` トレイトを受け取り、`read_pid()` → `is_process_alive(pid)` の順でチェックする正しいパターンを実装している
  - PIDファイル読み取りエラーや `None` の場合は「非稼働」として扱うパターンも確認できる
  - `cleanup` ではエラー終了が必要であり、`status` のような情報表示ではなく `Err(...)` を返す必要がある
- **Implications**: `cleanup` ハンドラで同様のパターンを採用し、デーモン稼働確認後に `Err` で早期リターンすればよい

### 現行実装のバグ確認

- **Context**: `src/bootstrap/app.rs:204-237` の `Command::Cleanup` ハンドラを精査
- **Sources Consulted**: `src/bootstrap/app.rs`
- **Findings**:
  - 現在のコードは `println!("⚠️  daemon が動作中の場合は停止してから cleanup を実行してください")` を表示するだけでチェックを行わない
  - デーモン稼働確認なしに `CleanupUseCase::execute()` を呼び出している
  - `PidFileManager` のインスタンスを生成していない（`status` コマンドとは異なりDI引数も受け取っていない）
- **Implications**: `cleanup` ハンドラ内で `PidFileManager::new(pid_path)` を直接インスタンス化し、チェックを追加する必要がある

### PIDファイルパスの確認

- **Context**: `cleanup` コマンドでどのパスで `PidFileManager` をインスタンス化すべきか確認
- **Sources Consulted**: `src/bootstrap/app.rs:204-237`（`Command::Cleanup { config }` ハンドラ）
- **Findings**:
  - `config` パラメータは `.cupola/config.toml` などの設定ファイルパスを表す `PathBuf`
  - `db_path` は `config.parent().unwrap_or_else(|| Path::new(".")).join("cupola.db")` で構成されている
  - PIDファイルは同ディレクトリに `cupola.pid` として置かれる（他コマンドのパターンより確認）
  - `PidFileManager::new(config.parent().unwrap_or_else(|| Path::new(".")).join("cupola.pid"))` で正しいパスが得られる
- **Implications**: `db_path` と同様のパターンでPIDファイルパスを構成する

### エラーメッセージ仕様の確認

- **Context**: ドキュメントに定義されたエラーメッセージ形式の確認
- **Sources Consulted**: `docs/commands/cleanup.md`（Issue内容より）
- **Findings**:
  - ドキュメントは `"Error: cupola is running (pid=12345). Run \`cupola stop\` first."` を要求
  - 出力先は標準エラー出力（`eprintln!`）
  - 終了コードは非ゼロ（`Err(anyhow::anyhow!(...))` を返して `main()` で伝播）
- **Implications**: `eprintln!` でメッセージを出力し、`anyhow::anyhow!("daemon is running")` を返す

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| bootstrap 層で直接チェック | `Command::Cleanup` ハンドラ内で `PidFileManager` をインスタンス化してチェック | 既存パターンと一致、変更箇所最小 | bootstrap 層の責務だが許容範囲 | **採用** |
| CleanupUseCase にチェックを委譲 | ユースケース層にデーモンチェックロジックを追加 | 関心の分離 | CleanupUseCase が PidFilePort に依存する必要が生じ、過剰設計 | 不採用 |

## Design Decisions

### Decision: `bootstrap` 層での直接チェック採用

- **Context**: デーモン稼働チェックをどの層で実装するか
- **Alternatives Considered**:
  1. `CleanupUseCase` にポート経由でチェックを委譲 — アプリケーション層でのチェック
  2. `Command::Cleanup` ハンドラで直接チェック — bootstrap 層でのチェック
- **Selected Approach**: bootstrap 層（`app.rs`）の `Command::Cleanup` ハンドラ内で `PidFileManager` を直接使用
- **Rationale**: 既存の `Command::Cleanup` ハンドラは既に bootstrap 層で `db_path` を構成・検証してから処理を開始するパターンを取っている。同じパターンでPIDチェックを追加することが最小変更で一貫性がある。`CleanupUseCase` に委譲すると、ユースケース層が `PidFilePort` に依存し、ユースケース・DI設定の変更が必要になり過剰設計となる。
- **Trade-offs**: bootstrap 層にビジネス的チェックが入るが、他コマンド（`status`、`start`）でも同様のパターンを取っており一貫している
- **Follow-up**: 将来的に `CleanupUseCase` にポート経由で委譲する場合は、ApplicationLayer の設計変更が必要

## Risks & Mitigations

- ステールPIDファイル（デーモンはクラッシュしているがPIDファイルが残存）— `is_process_alive()` がプロセス不在を正しく `false` で返すため、誤ってエラー終了しない
- EPERM によるプロセス存在の誤判定 — 既存実装で `EPERM` を「プロセス存在」として扱っているため、別プロセスのPIDが誤って記録されていた場合でも安全側に倒される

## References

- `src/bootstrap/app.rs:672-708` — `handle_status` 関数（参照実装）
- `src/adapter/outbound/pid_file_manager.rs` — `PidFileManager` 実装
- `src/application/port/pid_file.rs` — `PidFilePort` トレイト定義
- `docs/commands/cleanup.md` — ドキュメント上の正しい仕様（エラーメッセージ形式含む）
