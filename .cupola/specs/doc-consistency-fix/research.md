# Research & Design Decisions

---
**Purpose**: Capture discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary

- **Feature**: `doc-consistency-fix`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - 実装済みの CLIサブコマンド (`start` / `stop` / `doctor`) が `steering/tech.md` に未反映
  - `CHANGELOG.md` に start/stop デーモン機能の記述が存在しない
  - `nix` クレート (`cfg(unix)`) 依存により Windows 非対応だが README に明記なし
  - `gh-token` クレート利用で `.env` にトークン保存の可能性があるが `.gitignore` 未追加

## Research Log

### CLI サブコマンドの現状確認

- **Context**: `steering/tech.md` の記述と実装の乖離を確認
- **Sources Consulted**: steering/tech.md、Issue #110
- **Findings**:
  - 現在の `tech.md` は `run / init / status` を記載
  - 実装は `start / stop / init / status / doctor` に変更済み
  - `cargo run -- run` の例示も古い
- **Implications**: ドキュメント更新のみ。コード変更不要

### CHANGELOG の記載漏れ

- **Context**: v0.1 プレリリース監査での発見
- **Findings**:
  - `cupola start --daemon` と `cupola stop` がリリースノートに未記載
- **Implications**: Added セクションへの追記のみ

### プラットフォーム対応制約

- **Context**: `nix` クレートの `cfg(unix)` 依存に起因
- **Findings**:
  - Windows 向けビルドターゲットなし
  - 利用者が誤解しないよう README に明記が必要
- **Implications**: README.md / README.ja.md の要件セクション更新のみ

### セキュリティ: .env の gitignore 未設定

- **Context**: `gh-token` クレートが `.env` からトークンを読む可能性
- **Findings**:
  - `.gitignore` に `.env` が存在しない
- **Implications**: `.gitignore` への1行追加のみ

## Architecture Pattern Evaluation

本フィーチャーはすべてドキュメント・設定ファイルの変更であり、アーキテクチャ上の選択肢評価は不要。

## Design Decisions

### Decision: 単一 PR での一括修正

- **Context**: 4件の修正はすべて独立した軽微変更
- **Alternatives Considered**:
  1. 個別 PR — 変更ごとにレビューサイクルが発生
  2. 単一 PR — まとめてレビュー・マージ可能
- **Selected Approach**: 単一 PR で一括対応
- **Rationale**: 変更規模が小さく相互依存なし。レビューコストを最小化
- **Trade-offs**: 変更理由が混在するが README に説明コメントで対応可能
- **Follow-up**: 各ファイルの変更後に `cargo clippy` / `cargo test` 不要（ドキュメントのみ）

## Risks & Mitigations

- CHANGELOG のバージョン番号が未確定 — `[Unreleased]` セクションまたは `v0.1.0` セクションに追記する方針で統一
- README への Unix only 明記が過度に強調される可能性 — 要件セクションに1行追記する程度に留める

## References

- Issue #110: v0.1リリース前のドキュメント整合性修正
- `.cupola/steering/tech.md` — 現行 tech スタック記述
