# Research & Design Decisions

## Summary
- **Feature**: `license-setup`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - Apache License 2.0 の全文は SPDX 標準テキストを使用する
  - Cargo.toml の license フィールドは SPDX 識別子 `Apache-2.0` を使用する
  - README.md には既に `## License` セクションが存在し、TBD となっている

## Research Log

### Apache License 2.0 の標準テキスト
- **Context**: LICENSE ファイルに含めるべき正確なテキストの確認
- **Sources Consulted**: Apache Software Foundation 公式サイト、SPDX License List
- **Findings**:
  - Apache License 2.0 の全文は約 200 行
  - 著作権表示には `Copyright [yyyy] [name of copyright owner]` の形式を使用
  - APPENDIX セクションにボイラープレート通知を含む
- **Implications**: LICENSE ファイルは標準テキストをそのまま使用し、著作権行のみカスタマイズする

### Cargo.toml の license フィールド
- **Context**: Rust エコシステムでのライセンス表記方法の確認
- **Sources Consulted**: Cargo Book、crates.io ポリシー
- **Findings**:
  - `license` フィールドは SPDX 2.1 ライセンス式を使用
  - Apache 2.0 の SPDX 識別子は `Apache-2.0`
  - `license-file` フィールドは `license` フィールドと併用不要（Apache-2.0 は標準ライセンスのため）
- **Implications**: `license = "Apache-2.0"` を `[package]` セクションに追加するだけでよい

### README.md の現状
- **Context**: 既存の README.md 構造の確認
- **Findings**:
  - `## License` セクションが既に存在
  - 現在は「License is TBD. A link will be added here once the LICENSE file is created.」と記載
  - Table of Contents にも `[License](#license)` が含まれている
- **Implications**: 新規セクション追加は不要、既存セクションの内容を更新するだけでよい

## Design Decisions

### Decision: ライセンス表記形式
- **Context**: Apache License 2.0 の著作権表示をどのように記載するか
- **Alternatives Considered**:
  1. `Copyright 2026 cupola contributors` — コントリビューター全体を指す
  2. `Copyright 2026 kyuki3rain` — リポジトリオーナー個人を指す
- **Selected Approach**: `Copyright 2026 cupola contributors` を使用
- **Rationale**: OSS プロジェクトとして複数のコントリビューターを想定しており、個人名よりプロジェクト全体を代表する表記が適切
- **Trade-offs**: 個人の著作権主張は弱まるが、OSS の慣例に沿う
- **Follow-up**: 実装時にリポジトリオーナーの意向を確認可能

## Risks & Mitigations
- リスクなし（静的ファイルの追加・編集のみ）

## References
- Apache License 2.0 全文: https://www.apache.org/licenses/LICENSE-2.0
- SPDX License List: https://spdx.org/licenses/
- Cargo Book - manifest: https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields
