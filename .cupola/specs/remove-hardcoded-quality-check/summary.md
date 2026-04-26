# remove-hardcoded-quality-check サマリ

## Feature
`src/application/prompt.rs` の 3 プロンプトビルダーから Rust 固有コマンド (`cargo fmt` / `cargo clippy -- -D warnings` / `cargo test`) を除去し、「AGENTS.md / CLAUDE.md 記載の品質チェックに従え」という汎用指示に置き換え。cupola を Rust 以外のプロジェクトにも適用可能にする。本仕様は直前の `pre-commit-quality-check` の反省から生まれた一般化リファクタ。

## 要件サマリ
- `build_design_prompt` / `build_implementation_prompt` (feature_name 有無両方) / `build_fixing_prompt` から `cargo fmt/clippy/test` 文字列を完全除去。
- 統一の汎用指示「commit 前に AGENTS.md / CLAUDE.md に記載された品質チェックを実行し、全てパスしてから commit すること。失敗した場合は修正して再チェックすること。」を 3 関数に適用。
- commit/push 手順、`Closes #N`、causes 別修正指示など他の指示は維持。
- 既存の「cargo コマンド存在アサート」テスト 4 件を削除し、汎用指示の存在を検証する 4 件に置換。
- リポジトリルートに `AGENTS.md` を新規作成し、cupola プロジェクト (Rust) 向けの具体的な品質チェックコマンドを記載（Claude Code が自動読み込み）。

## アーキテクチャ決定
- **共通文字列をモジュールレベル `const` `GENERIC_QUALITY_CHECK_INSTRUCTION` として切り出し** (採用): 3 関数で同一文言を使うため DRY、文言変更が 1 箇所で完結。インライン埋め込み案は重複で却下。
- **文言は Issue 要求の長めの文章をそのまま採用** (採用): 「AGENTS.md/CLAUDE.md の指示に従え」の短縮案より Claude への指示明確性が上がる。実装者の解釈ブレも防ぐ。
- **言語別コマンドのハードコードは非採用**: 他言語の品質チェックを prompt 側に持ち込むのは同じ罠の繰り返し。Claude Code が自動読み込みする AGENTS.md/CLAUDE.md に委譲する設計で言語非依存性を実現。
- **関数シグネチャ・呼び出し側は変更しない**: 純粋な文字列置換と定数追加のみ。

## コンポーネント
- `src/application/prompt.rs`:
  - `GENERIC_QUALITY_CHECK_INSTRUCTION` 定数を新規追加。
  - `build_design_prompt` ステップ 6 を定数参照に置換（commit は step 7 のまま）。
  - `build_implementation_prompt` `quality_check_step` ブロックを定数参照に置換（`push_step` 計算ロジックは不変）。
  - `build_fixing_prompt` ステップ 3 を定数参照に置換（commit は step 4 のまま）。
  - `#[cfg(test)] mod tests`: 旧 `*_contains_quality_check` 4 テストを削除、`*_generic_quality_check` 4 テストを追加（`cargo fmt` 不在 + `AGENTS.md` 存在の 2 軸アサート）。
- `AGENTS.md`（新規、リポジトリルート）: 本プロジェクト向けの具体的な cargo コマンド列挙をこちらに移動。

## 主要インターフェース
```rust
const GENERIC_QUALITY_CHECK_INSTRUCTION: &str =
    "commit 前に AGENTS.md / CLAUDE.md に記載された品質チェックを実行し、全てパスしてから commit すること。失敗した場合は修正して再チェックすること。";
```
関数シグネチャ変更なし。

## 学び / トレードオフ
- 前機能 `pre-commit-quality-check` で 3 関数にハードコードした直後に「多言語対応するには間違いだった」と反省し、責務を AGENTS.md/CLAUDE.md に委譲する形で一般化。責務配置の学びが良い例。
- テストアサートが具体コマンド文字列に依存していたため、汎用化で 4 テストを丸ごと置換する必要があった（将来の文言変更は定数+テスト 1 箇所ずつで済む形に改善）。
- 長めの日本語文言は Claude への明確性を優先。言語非依存のはずの指示に日本語を使う点は、現状 cupola 自身の `language: ja` 設定と整合しておりトレードオフとして許容。
