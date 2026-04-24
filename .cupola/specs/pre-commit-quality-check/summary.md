# pre-commit-quality-check サマリ

## Feature
Claude Code の設計・実装・レビュー対応各エージェント向けプロンプト (`build_design_prompt` / `build_implementation_prompt` / `build_fixing_prompt`) に、commit 直前の品質チェック手順 (`cargo fmt` / `cargo clippy -- -D warnings` / `cargo test`) を明示的に追加。CI で失敗する壊れた PR の push を未然に防ぐ。

## 要件サマリ
- 3 つのプロンプトすべてで commit の直前に独立した手順番号として品質チェックを実行させる。
- いずれか失敗時は「修正→再チェック」のループを指示、全パス後にのみ commit。
- `build_implementation_prompt` は `feature_name` 有無の両パスで適用。`push_step` に加え `quality_check_step` 変数を導入。
- 既存ユニットテストが全てパス、`output_schema`（`PrCreation` / `Fixing`）は不変。

## アーキテクチャ決定
- **プロンプト文字列への直接追加** (採用): 3 関数共通ヘルパー抽出案もあったが、設計/実装/修正で文脈が異なり将来個別カスタマイズの可能性があるため YAGNI で不採用。重複は許容。
- **commit 直前に独立した手順番号として挿入**: commit 手順内への折込みよりも可読性が高く、Claude に「commit の前に必ず実行」を明示できる。
- **コマンドはプロジェクト標準に完全準拠**: `.cupola/steering/tech.md` の Development Standards と整合。
- コマンドの実際の実行制御・CI 結果連携・品質チェックスキップ機能は非スコープ。

## コンポーネント
- `src/application/prompt.rs` のプライベート 3 関数のみ変更。
  - `build_design_prompt`: 既存手順6 (commit/push) を 7 に繰り下げ、新手順6 に品質チェック挿入。
  - `build_implementation_prompt`: `feature_name` ありで「実装1→品質2→push3」、なしで「spec確認1→実装2→品質3→push4」。
  - `build_fixing_prompt`: 既存手順3 (commit/push) を 4 に繰り下げ、新手順3 に品質チェック挿入。
- 新規ユニットテスト: 各プロンプト関数が `cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` 文字列を含むことを検証。

## 主要インターフェース
関数シグネチャの変更なし。プロンプト本文に品質チェックブロックを挿入。

## 学び / トレードオフ
- データモデル・ステートマシン・外部 API・依存関係に一切影響しない最小変更で問題解決。
- テキスト重複が生じるが、3 プロンプトの独立性を優先することで将来の個別調整に柔軟性を確保。
- 既存テストがプロンプト内の文字列存在を確認する方式だったため、手順番号の繰り下げは影響せず、追加テストで新規文言の存在を検証するだけで品質担保できた。
