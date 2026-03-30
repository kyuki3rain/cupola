# Research & Design Decisions

## Summary
- **Feature**: `cupola-doctor`
- **Discovery Scope**: Extension（既存 CLI への新サブコマンド追加）
- **Key Findings**:
  - 既存の `Command` enum に `Doctor` バリアントを追加し、`app.rs` で分岐するだけで統合可能
  - 外部コマンド実行は `std::process::Command` で同期的に行うのが最適（既存の `claude_code_process.rs` と同じパターン）
  - チェック項目間に依存関係がある（gh 認証成功時のみラベルチェック実行）ため、順次実行モデルが適切

## Research Log

### 既存 CLI 拡張ポイント
- **Context**: 新サブコマンド `doctor` の追加方法を調査
- **Sources Consulted**: `src/adapter/inbound/cli.rs`, `src/bootstrap/app.rs`
- **Findings**:
  - `Command` enum（clap derive）に新バリアントを追加するだけで CLI 拡張可能
  - `app::run()` の match 分岐に `Command::Doctor` を追加
  - `Init` や `Status` と同様、引数なしのシンプルなサブコマンド
- **Implications**: 既存パターンに完全に従える。新規ファイルは最小限

### 外部コマンド実行パターン
- **Context**: `git --version`, `claude --version`, `gh auth status` の実行方法
- **Sources Consulted**: `src/adapter/outbound/claude_code_process.rs`, Rust `std::process::Command` ドキュメント
- **Findings**:
  - `std::process::Command` で同期実行し、`output()` で結果取得
  - 終了コードと stdout/stderr の両方を確認可能
  - `doctor` は短時間で完了するため、非同期は不要
- **Implications**: `std::process::Command` を直接使用し、tokio の spawn_blocking は不要

### 設定ファイルチェックパターン
- **Context**: `cupola.toml` の存在確認と必須項目検証の方法
- **Sources Consulted**: `src/bootstrap/config_loader.rs`, `src/domain/config.rs`
- **Findings**:
  - `config_loader::load_toml()` が既に TOML 読み込みとパースを行っている
  - `CupolaToml` の `owner`, `repo`, `default_branch` は必須フィールド（Option ではない）
  - TOML パースに失敗すれば必須項目の欠落を検出可能
- **Implications**: ファイル存在確認 + `load_toml()` の再利用で設定チェックを実現可能

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| application 層に DoctorUseCase を配置 | チェックロジックを use case として分離 | テスト容易、Clean Architecture 準拠 | やや過剰な抽象化 | 採用：外部コマンド実行をポート経由で抽象化すればテスト可能 |
| bootstrap/app.rs に直接実装 | dispatch 内にチェックロジックを記述 | シンプル、ファイル追加なし | テスト困難、責務混在 | 不採用：既存パターンと一貫性がない |

## Design Decisions

### Decision: チェック項目のモデリング
- **Context**: 7 つのチェック項目を統一的に扱う必要がある
- **Alternatives Considered**:
  1. 各チェックを個別関数で実装し、結果を手動で集約
  2. `CheckItem` 構造体で統一的にモデリング
- **Selected Approach**: `CheckResult` 構造体（名前、成功/失敗/スキップ、修正手順）で統一
- **Rationale**: 結果表示ロジックを共通化でき、チェック項目の追加も容易
- **Trade-offs**: 若干の構造体定義が必要だが、保守性が向上

### Decision: チェック実行の責務分離
- **Context**: 外部コマンド実行とファイルシステム確認を分離する方法
- **Alternatives Considered**:
  1. すべてを DoctorUseCase 内に直接実装
  2. ポート（trait）を定義してアダプターに委譲
- **Selected Approach**: `CommandRunner` ポート（trait）を定義し、外部コマンド実行を抽象化。ファイルシステムチェックは `std::fs` を直接使用
- **Rationale**: 外部コマンド実行のモック化によりユニットテストが可能。ファイルシステムは tempdir で十分テスト可能
- **Trade-offs**: trait 定義のオーバーヘッドがあるが、テスタビリティを確保

## Risks & Mitigations
- `gh label list` の出力フォーマットが変更される可能性 — `--json` フラグで JSON 出力を使用し安定性を確保
- 外部コマンドがパスにない場合のエラーハンドリング — `Command::output()` の `Err` をキャッチして失敗として扱う
- `cupola.toml` のパスがワーキングディレクトリ依存 — 既存の `config_loader` と同じパス解決を使用
