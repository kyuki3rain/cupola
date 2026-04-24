# design-pr-closes-removal

## Feature
`build_design_prompt` が生成する設計 PR のプロンプトに「PR body で `Related: #N` を使用し、`Closes` は使わない」という指示を追加する。設計 PR は中間成果物であり、マージ時に元の Issue が GitHub の自動 close キーワードで閉じられてしまうのを防ぐ。実装 PR の `Closes #N` は維持する。

## 要件サマリ
- `build_design_prompt` の PR body 出力指示に `Related: #{issue_number}` を含めることを追加。
- 設計 PR body には `Closes` / `Fixes` / `Resolves` 等の自動 close キーワードを含めない制約を追加。
- 設計 PR マージ時に Issue が自動 close されないことを保証。
- `build_implementation_prompt` の `Closes #{issue_number} を含めること` は変更せず維持。
- `fallback_pr_body` は既に設計 PR で `Related: #N`、実装 PR で `Closes #N` と正しく分岐済みのため変更不要。

## アーキテクチャ決定
- **修正箇所の選定**: (1) プロンプトに指示追加、(2) PR body のポストプロセスで `Closes` → `Related` 置換、の 2 案を検討。(1) を採用。理由: 最もシンプルで、既存の `build_implementation_prompt` と同じ指示追加パターンを踏襲できる。ポストプロセス置換は言語処理ロジックを増やし、ユーザー記述の意図しない `Closes` まで書き換えるリスクがあり不採用。
- **Claude の指示遵守への依存**: Claude が指示を無視するリスクは存在するが、`fallback_pr_body` が既に `Related: #N` を使う設計となっているためフォールバック経路は安全。出力パース成功時の指示無視時のみリスクあり、この経路はテストで最低限検出する方針。
- **GitHub のキーワード仕様理解**: `Closes` / `Fixes` / `Resolves` + `#N` が自動 close をトリガーする。`Related` はこれらに含まれないため自動 close を発生させない。この仕様を根拠に `Related:` 形式の利用を決定。
- **Non-Goals**: `build_fixing_prompt` 変更、`fallback_pr_body` 変更、PR body ポストプロセス導入はスコープ外として明示。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `build_design_prompt`（更新） | application/prompt | 設計エージェント向けプロンプトの PR body 指示に `Related` 使用と `Closes` 禁止を追加 |
| `build_implementation_prompt`（変更なし） | application/prompt | `Closes #{issue_number}` を維持（回帰テスト対象） |
| `fallback_pr_body`（変更なし） | application/prompt | 設計/実装で `Related` / `Closes` 分岐済み |

## 主要インターフェース
- `build_design_prompt` 内のフォーマット文字列に以下を追加:
  - `output-schema への出力` セクションの `pr_body` 指示: `Related: #{issue_number}` を含めること
  - `制約事項` セクション: PR body に `Closes` を含めない旨の制約
- `build_implementation_prompt` の L151 相当の指示パターンを踏襲。

## 学び / トレードオフ
- プロンプトテンプレート修正のみで機能的な要件を満たせる典型例。ソースコード変更量は最小（1 関数内のテンプレート文字列）。
- `fallback_pr_body` が既に正しい分岐を持っていたため、Claude の出力ぶれに対するセーフティネットが自然に機能する構成だった。これは偶然ではなく、設計時点での分離が後のバグ修正コストを下げた好例。
- Claude の指示遵守に依存する領域は、テストで「指示が含まれていること」を確認するのが限界で、「Claude が実際にその指示に従うか」の end-to-end 検証は難しい。運用中の PR レビューで継続モニタリングが必要。
- PR 本文のキーワードによる Issue 自動 close は GitHub の仕様であり、プロジェクト側でのオプトアウトは存在しない（リポジトリ設定で無効化できない）。文字列レベルで確実に避けるしかない。
- 将来的に同様の「プロンプト出力制約」が増えた場合、プロンプト生成を template-based に再設計する余地はあるが、現時点では format 文字列ベースで十分。
