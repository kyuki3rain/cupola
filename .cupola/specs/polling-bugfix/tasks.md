# Implementation Plan

## Requirements Coverage

| Requirement | Tasks |
|-------------|-------|
| 1.1, 1.2, 1.3, 1.4, 1.5 | 1.1 |
| 2.1, 2.2, 2.3, 2.4, 2.5, 2.6 | 2.1, 2.2, 2.3 |

## Tasks

- [x] 1. review_waiting 状態での Issue close 検知時に PR merge を優先確認する
- [x] 1.1 Step 1 の Issue close 検知ロジックを修正する
  - polling サイクル Step 1 で Issue close を検知した際、対象 Issue が review_waiting 状態（DesignReviewWaiting / ImplementationReviewWaiting）かどうかを判定する
  - review_waiting 状態の場合、対応する PR 番号（design_pr_number / impl_pr_number）を使って is_pr_merged を呼び出す
  - PR が merge 済みであれば IssueClosed ではなく PrMerged イベント（DesignPrMerged / ImplementationPrMerged）を発行する
  - PR が未 merge、または PR 番号が未設定の場合は従来通り IssueClosed イベントを発行する
  - is_pr_merged の API 呼び出しが失敗した場合はログ記録し、安全側に倒して IssueClosed イベントを発行する
  - review_waiting 以外の状態での Issue close 検知は従来動作を維持する
  - 統合テストを追加する: review_waiting + Issue close + PR merge 済み → completed 遷移を検証する
  - 統合テストを追加する: review_waiting + Issue close + PR 未 merge → cancelled 遷移を検証する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [x] 2. 設計フェーズの output-schema に feature_name を追加し実装プロンプトに埋め込む
- [x] 2.1 Issue エンティティと DB スキーマに feature_name を追加する
  - Issue エンティティに feature_name（Option<String>）フィールドを追加する
  - issues テーブルの CREATE TABLE に feature_name TEXT カラムを追加する
  - 既存 DB 向けに起動時の ALTER TABLE ADD COLUMN を安全に実行する（既にカラムが存在する場合はエラーを無視）
  - SQLite リポジトリの SELECT / INSERT / UPDATE クエリに feature_name を追加する
  - reset_for_restart で feature_name を NULL にリセットする
  - 既存のユニットテストを feature_name の追加に合わせて更新する
  - _Requirements: 2.6_

- [x] 2.2 (P) output-schema とプロンプトを修正する
  - PR 作成用 output-schema（PR_CREATION_SCHEMA）に feature_name フィールドを追加する（required には含めない）
  - PrCreationOutput 構造体に feature_name（Option<String>）を追加する
  - 設計プロンプトに feature_name の出力指示を追加する（「cc-sdd の feature name を出力してください」）
  - 実装プロンプトのシグネチャに feature_name を追加し、Some の場合は `/kiro:spec-impl {name}` を明示的に指示、None の場合は従来の ls + phase フィルタ指示をフォールバックとして使用する
  - build_session_config のシグネチャに feature_name を追加し、ImplementationRunning 時に渡す
  - output-schema パーステスト（feature_name あり・なし）を追加する
  - プロンプト生成テスト（feature_name あり・なし）を追加する
  - _Requirements: 2.1, 2.2, 2.4, 2.5_

- [x] 2.3 設計フェーズ正常終了時に feature_name を DB に保存する
  - create_pr_from_output 内で、DesignRunning 状態の正常終了後に output の feature_name を Issue レコードに保存する
  - feature_name が None の場合はスキップする（フォールバック動作）
  - ImplementationRunning 時の build_session_config 呼び出しで、DB から取得した feature_name を渡す
  - _Requirements: 2.3, 2.4, 2.5_

- [x] 3. 全体の検証
- [x] 3.1 既存テストの回帰確認とリファクタリング
  - 全既存ユニットテスト（110 件）と統合テスト（9 件）がパスすることを確認する
  - clippy 警告ゼロ、cargo fmt 一致を確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_
