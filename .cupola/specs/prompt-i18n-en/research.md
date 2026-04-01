# Research & Design Decisions

---
**Purpose**: 本フィーチャーの設計判断と調査結果を記録する。

---

## Summary

- **Feature**: `prompt-i18n-en`
- **Discovery Scope**: Extension（既存コードの文字列リファクタリング）
- **Key Findings**:
  - 変更対象は `src/application/prompt.rs` の定数・関数・テストと `AGENTS.md` の文字列のみ。新コンポーネントや新アーキテクチャは不要。
  - `{language}`, `{quality_check}`, `{feature_instruction}`, `{instructions_text}` フォーマットパラメータ、コマンド名、パス参照、JSON プロパティ名は変更禁止であり、それ以外の日本語テキストのみが対象。
  - テストの assertion 文字列（日本語リテラル）が英語化後のプロンプトと不一致になるため、16件のテストも同時に更新が必要。

## Research Log

### 対象コードの調査

- **Context**: どのコードが変更対象で、何が保護対象かを明確にするために既存実装を調査した。
- **Sources Consulted**: `src/application/prompt.rs`（全体）、`AGENTS.md`
- **Findings**:
  - `PR_CREATION_SCHEMA`（L16）: `description` フィールドが日本語（"PR のタイトル", "PR の body（Markdown 形式）", "cc-sdd の feature name..."）
  - `FIXING_SCHEMA`（L18）: `description` フィールドが日本語（"対応した review thread の ID...", "review thread への返信内容", "この thread を resolve するか"）
  - `GENERIC_QUALITY_CHECK_INSTRUCTION`（L20）: "commit 前に AGENTS.md / CLAUDE.md に記載された品質チェック..."
  - `build_design_prompt`（L74-123）: プロンプト本文全体が日本語
  - `build_implementation_prompt`（L125-173）: プロンプト本文全体が日本語（`feature_name` 有無の分岐含む）
  - `build_fixing_prompt`（L175-243）: プロンプト本文 + 動的文字列3件（L185, L189, L193）+ フォールバック（L197）が日本語
  - テスト: `"自動設計エージェント"`, `"自動実装エージェント"`, `"レビュー対応エージェント"`, `"base ブランチ"` の4種の日本語リテラルを assertion に使用
  - `AGENTS.md`: 品質チェック指示が日本語
- **Implications**: 変更範囲は文字列リテラルのみ。関数シグネチャ・ロジック・依存関係は一切変更不要。

### トークン削減の効果

- **Context**: 英語化によるトークン削減量を見積もる
- **Findings**:
  - 日本語1文字は UTF-8 で3バイト（3トークン相当）になる場合があるのに対し、英語は概ね1文字1トークン
  - `build_design_prompt` / `build_implementation_prompt` / `build_fixing_prompt` はそれぞれ数十〜数百トークンの削減が見込まれる
  - `GENERIC_QUALITY_CHECK_INSTRUCTION` は複数のプロンプトに展開されるため、英語化の効果が乗算的に効く
- **Implications**: 機能要件の変更なしに継続的なコスト削減が実現できる。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| インプレース文字列置換 | 各定数・関数の日本語テキストを英語に直接置き換える | 変更最小化、差分が明確 | なし | 選択 |
| 定数ファイル分離 | 言語別に定数ファイルを分離する | 多言語対応に柔軟 | オーバーエンジニアリング、現要件範囲外 | 不採用 |

## Design Decisions

### Decision: インプレース文字列置換を採用

- **Context**: 内部プロンプトの英語化をシンプルかつ低リスクで実現する方法を選択する必要がある。
- **Alternatives Considered**:
  1. インプレース置換 — 各定数・関数内の日本語テキストを英語に直接書き換える
  2. i18n フレームワーク導入 — gettext 等による多言語切り替え
- **Selected Approach**: インプレース文字列置換
- **Rationale**: 変更範囲が文字列リテラルのみであり、新たな抽象化やファイル分割は不要。既存の Clean Architecture を維持したまま最小変更で目標を達成できる。
- **Trade-offs**: 将来的に再度日本語に戻す場合は git で追跡可能だが手動変更が必要。多言語への柔軟な拡張は困難（ただし現要件外）。
- **Follow-up**: `cargo test` で全16件のテストが通過することを確認する。

### Decision: テスト assertion の英語化対応

- **Context**: テストが日本語リテラルを直接参照しているため、プロンプト英語化に合わせて assertion も更新が必要。
- **Alternatives Considered**:
  1. テスト assertion を英語に更新
  2. テストにプロンプトの特定部分のみ参照させる（定数に切り出し）
- **Selected Approach**: assertion を英語に更新
- **Rationale**: `GENERIC_QUALITY_CHECK_INSTRUCTION` を参照する4件のテストはすでに定数参照のため自動追従する。残り4種の日本語リテラル（エージェント識別子・"base ブランチ"）のみ英語に対応すればよい。定数への切り出しは追加の複雑化になるため不採用。
- **Trade-offs**: assertion を英語文字列に変更することで、将来プロンプトを変更した際のテストメンテナンスコストは変わらない。

## Risks & Mitigations

- 保護対象パラメータの誤変更リスク — `{language}`, `{quality_check}` 等のフォーマットパラメータを誤って翻訳してしまう可能性 → 変更前後で format パラメータが完全一致するか diff で確認する
- テスト assertion の見落とし — 日本語リテラルが残存しテストが失敗する → `cargo test` で全16件の通過を必須とする
- コマンド名の変更 — `/kiro:spec-*` コマンドを翻訳してしまう可能性 → コマンド名は変更対象外として設計書に明記する

## References

- `src/application/prompt.rs` — 変更対象ソースファイル
- `AGENTS.md` — 変更対象ドキュメント
- Issue #81 — 本フィーチャーの要件元
