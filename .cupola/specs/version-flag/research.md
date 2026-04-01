# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary
- **Feature**: `version-flag`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - clap 4.x の `#[command(version)]` アトリビュートで `--version` / `-V` フラグが自動追加される
  - `env!("CARGO_PKG_VERSION")` によりコンパイル時に `Cargo.toml` の `version` フィールドを取得できる
  - 変更箇所は `src/adapter/inbound/cli.rs` の `Cli` 構造体の `#[command(...)]` アトリビュート1行のみ

## Research Log

### clap `#[command(version)]` の挙動

- **Context**: `--version` / `-V` フラグを clap で自動追加する方法を確認
- **Sources Consulted**: `Cargo.toml`（clap = { version = "4", features = ["derive"] }）、clap 公式ドキュメント
- **Findings**:
  - `#[command(version)]` を付与すると clap が自動的に `--version` と `-V` を登録する
  - バージョン文字列は `env!("CARGO_PKG_VERSION")` が自動適用される
  - 出力形式は `cupola 0.1.0` となる（`<name> <version>` 形式）
  - clap 4.x では `version` キーワードのみで十分（明示的に `env!` を書く必要なし）
- **Implications**: 実装コスト最小。既存コードへの影響はアトリビュート1行の追加のみ。

### 既存 CLI 構造の確認

- **Context**: 既存の `Cli` 構造体の `#[command(...)]` アトリビュートを確認
- **Sources Consulted**: `src/adapter/inbound/cli.rs`
- **Findings**:
  - 現在: `#[command(name = "cupola", about = "GitHub Issue-driven automation agent")]`
  - 変更後: `#[command(name = "cupola", about = "GitHub Issue-driven automation agent", version)]`
  - 既存のサブコマンドやフラグへの影響なし
- **Implications**: 破壊的変更なし。テストは `Cli::try_parse_from(["cupola", "--version"])` を使い、`ErrorKind::DisplayVersion` が返ることを確認する（`parse_from` は `process::exit` を呼ぶためテストには不適切）。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| `#[command(version)]` 自動追加 | clap のアトリビュートで自動処理 | 実装コスト最小、バージョン同期が自動 | なし | 採用 |
| 手動フラグ定義 | `--version` を `#[arg(long, short = 'V')]` で手動定義 | カスタマイズ可能 | 不要な複雑性、clap 標準から外れる | 不採用 |

## Design Decisions

### Decision: `#[command(version)]` アトリビュートによる自動対応

- **Context**: `--version` / `-V` フラグの実装方法の選択
- **Alternatives Considered**:
  1. `#[command(version)]` — clap に全て委譲
  2. 手動フラグ定義 + `env!("CARGO_PKG_VERSION")` 出力
- **Selected Approach**: `#[command(version)]` アトリビュートを `Cli` 構造体に追加
- **Rationale**: clap 4.x の標準機能であり、保守コストが最小。Issue の技術的コンテキストにも明記されている。
- **Trade-offs**: カスタマイズ不可だが、標準形式（`cupola 0.1.0`）で十分
- **Follow-up**: `cargo test` で `--version` / `-V` パースが exit code 0 で終了することを確認

## Risks & Mitigations

- clap が `--version` / `-V` のフラグ名を予約するため、将来これらを別用途で使用不可 — 現時点で競合なし、問題なし

## References

- clap derive マクロ公式ドキュメント: `#[command(version)]` の動作仕様
