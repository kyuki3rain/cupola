# improve-build-fixing-prompt サマリー

## Feature
`src/application/prompt.rs` の `build_fixing_prompt` の品質問題 3 点を修正し、修正プロンプトの正確性・安全性・一貫性を向上。

## 要件サマリ
1. `_issue_number` / `_pr_number` のアンダースコアを除去し、プロンプトヘッダーに `PR #<pr_number> (Issue #<issue_number>)` を埋め込む。
2. `causes` に `ReviewComments` が含まれる場合のみスレッド output 指示 (`thread_id` / `response` / `resolved`) を含め、含まれない場合は `{"threads": []}` を明示するセクションに切り替える。
3. `git add -A` を廃止し、`git diff --name-only` で変更ファイルを確認して個別にステージングするよう誘導。
4. 既存テストを更新し、上記 3 点 + `git add -A` の非存在を検証するテスト (`fixing_prompt_no_git_add_all`) を追加。

## アーキテクチャ決定
- applicationレイヤーの純粋関数内でのテンプレート条件分岐（既存 `instructions` 構築と同じパターン）を採用。別関数分割案は重複コード増加と `build_session_config` シグネチャ変更が必要なため不採用。
- `FIXING_SCHEMA` は変更不要。`threads` 必須制約は `{"threads": []}` を返すよう明示することで維持。
- `build_fixing_prompt` / `build_session_config` のシグネチャ変更なし。ドメイン層・アダプタ層への影響なし。

## コンポーネント
- `src/application/prompt.rs`: `build_fixing_prompt` 関数とテストモジュール

## 主要インターフェース
- `fn build_fixing_prompt(issue_number: u64, pr_number: u64, language: &str, causes: &[FixingProblemKind]) -> String`

## 学び/トレードオフ
- design prompt は特定ディレクトリ指定、fixing は任意ファイル修正のため `git add <files>` + `git diff --name-only` 確認案内を採用し一時ファイル混入を防止。
- プロンプト文字列生成のみで外部依存・DB・アダプタへの影響なし。
- `FIXING_SCHEMA` との整合性維持のため空 threads を明示することが重要。
