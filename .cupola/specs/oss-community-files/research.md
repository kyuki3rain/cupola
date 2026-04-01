# Research & Design Decisions

---
**Purpose**: `oss-community-files` フィーチャーに関する調査結果・設計判断の記録

---

## Summary
- **Feature**: `oss-community-files`
- **Discovery Scope**: Simple Addition（既存リポジトリへのドキュメント・設定ファイルの追加）
- **Key Findings**:
  - `.github/workflows/ci.yml` が存在することを確認済み。CI バッジ URL に使用するパスが正確に合致する
  - ライセンスは Apache-2.0（`LICENSE` ファイル存在）。バッジ URL もこれに合わせる
  - README.md / README.ja.md ともに先頭行はタイトル `# Cupola` であり、バッジ挿入位置はタイトル直下に統一する

## Research Log

### CI ワークフロー名の確認
- **Context**: README バッジに埋め込む CI ワークフロー URL が正確かを確認する必要があった
- **Sources Consulted**: `.github/workflows/` ディレクトリ一覧
- **Findings**:
  - `ci.yml` と `release.yml` の 2 ファイルが存在
  - Issue に記載のバッジ URL `https://github.com/kyuki3rain/cupola/actions/workflows/ci.yml/badge.svg` と完全一致
- **Implications**: バッジ URL はそのまま Issue 記載のものを使用できる

### README 構造の確認
- **Context**: バッジをどこに挿入するかを判断するため、既存の README 構造を確認
- **Findings**:
  - `README.md` 先頭: `# Cupola` → `[日本語](./README.ja.md)` → 本文
  - バッジはタイトル `# Cupola` の直後（リンク行の前）に挿入するのが GitHub 標準慣行
- **Implications**: `# Cupola` 行の直後にバッジ行を挿入する

### Contributor Covenant v2.1 の構造
- **Context**: CODE_OF_CONDUCT.md の公式テキストの内容確認
- **Findings**:
  - Contributor Covenant v2.1 は英語の公式テキストを使用する
  - 連絡先（`[INSERT CONTACT METHOD]`）のプレースホルダーを適切に置き換える必要がある
  - GitHub Issues URL または GitHub の security/advisories を連絡先として設定する
- **Implications**: 連絡先は GitHub Issues（`https://github.com/kyuki3rain/cupola/issues`）を指定

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク | 備考 |
|-----------|------|------|--------|------|
| 静的ファイルの直接追加 | 全ファイルをリポジトリルートおよび `.github/` に配置 | シンプル、追加コードなし | なし | 本フィーチャーに最適 |
| テンプレートシステムの導入 | 動的生成の仕組みを作る | 将来の柔軟性 | 過剰設計 | 対象外 |

## Design Decisions

### Decision: CONTRIBUTING.md の言語選択
- **Context**: 国際的なコントリビュータ向けに英語が求められている
- **Alternatives Considered**:
  1. 英語のみ
  2. 日本語・英語の両方を1ファイルに
- **Selected Approach**: 英語のみで記述
- **Rationale**: 国際標準慣行に沿い、主要 OSS（ripgrep, bat, tokio）の CONTRIBUTING.md に倣う
- **Trade-offs**: 日本語話者には若干の障壁があるが、OSS としての開放性を優先

### Decision: CODE_OF_CONDUCT.md の言語選択
- **Context**: 公式テキスト（Contributor Covenant v2.1）の使用が要件として明示されている
- **Selected Approach**: 英語公式テキストを使用
- **Rationale**: Contributor Covenant の公式テキストは英語であり、翻訳による意図の歪みを避ける
- **Trade-offs**: 日本語訳は別途検討事項だが、現フェーズでは対象外

### Decision: dependabot.yml の groups 設定
- **Context**: PR 乱立防止のため groups 機能を使用する
- **Selected Approach**: Cargo の minor/patch を `rust-dependencies` グループ、GitHub Actions を `github-actions` グループとして束ねる
- **Rationale**: Issue に明示された要件どおり。major 更新は個別 PR で対応する
- **Trade-offs**: major 更新は個別 PR になるが、破壊的変更を含むため個別確認が適切

## Risks & Mitigations
- CONTRIBUTING.md の devbox セットアップ手順が実態と乖離するリスク — `devbox shell` を最初のステップとして明記し、`cargo build / test` の動作を確認済み手順として記述する
- CODE_OF_CONDUCT.md の連絡先が無効になるリスク — GitHub Issues URL を使用することで常に有効なリンクを保持する
- dependabot.yml の `groups` 機能は GitHub Enterprise の一部プランで利用不可の場合がある — cupola は GitHub.com を使用しているため問題なし

## References
- [Contributor Covenant v2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/) — CODE_OF_CONDUCT.md の公式テキスト
- [GitHub dependabot configuration](https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file) — dependabot.yml 設定リファレンス
- [GitHub Private Vulnerability Reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability) — SECURITY.md で参照する報告手段
- [ripgrep CONTRIBUTING.md](https://github.com/BurntSushi/ripgrep/blob/master/CONTRIBUTING.md) — CONTRIBUTING.md の参考実装
