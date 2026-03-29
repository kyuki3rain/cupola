# Requirements Document

## Project Description (Input)
polling ループのバグ修正: (1) PR merge 時の Closes #N による Issue 自動 close で cancelled になる問題を、review_waiting 状態での Issue close 検知時に PR merge を先に確認することで修正する。(2) 設計フェーズの output-schema に feature_name を追加し、実装プロンプトに明示的に埋め込むことで、複数 specs 存在時の feature 特定問題を解決する。

## Introduction

Cupola の E2E テストで発見された 2 つのバグを修正する。いずれも polling ループのイベント処理と Claude Code 連携に関する問題であり、正常な全工程完了（completed）と正確な実装フェーズ実行を阻害している。

- GitHub Issue #6: PR merge 時の Closes による cancelled バグ
- GitHub Issue #7: feature_name の output-schema 追加

## Requirements

### Requirement 1: review_waiting 状態での Issue close 検知時に PR merge を優先確認する

**Objective:** 開発者として、PR merge 時に GitHub の `Closes #N` で Issue が自動 close されても、cupola が正しく completed に遷移してほしい。cancelled ではなく completed として処理されるべき。

#### Acceptance Criteria

1. While `implementation_review_waiting` 状態で Issue close が検知される, Cupola shall PR の merge 状態を先に確認し、merge 済みであれば ImplementationPrMerged イベントを優先して発行する
2. While `design_review_waiting` 状態で Issue close が検知される, Cupola shall PR の merge 状態を先に確認し、merge 済みであれば DesignPrMerged イベントを優先して発行する
3. When review_waiting 状態で Issue close が検知され、かつ PR が merge されていない場合, Cupola shall 従来通り IssueClosed イベントを発行し cancelled に遷移する
4. When 実装 PR の merge と Issue close が同一 polling サイクルで検知される, Cupola shall completed に遷移し「全工程が完了しました」コメントを投稿する
5. The Cupola shall review_waiting 以外の状態での Issue close 検知は従来通り cancelled に遷移する

### Requirement 2: 設計フェーズの output-schema に feature_name を追加する

**Objective:** 開発者として、設計フェーズで Claude Code が生成した feature name を cupola が記録し、実装フェーズで正確にその feature を指定して実行できるようにしたい。

#### Acceptance Criteria

1. The Cupola shall 設計フェーズ（design_running）の output-schema に `feature_name` フィールドを追加する（`{ pr_title, pr_body, feature_name }`）
2. The Cupola shall 設計プロンプトに feature_name の出力指示を含める
3. When 設計フェーズの Claude Code が正常終了する, Cupola shall output-schema から feature_name を抽出し Issue レコードに記録する
4. The Cupola shall 実装プロンプトに feature_name を埋め込み、`/kiro:spec-impl {feature_name}` を明示的に指示する
5. If feature_name の抽出に失敗する, then Cupola shall フォールバックとして `ls .cupola/specs/` + phase フィルタの従来指示を使用する
6. The Cupola shall issues テーブルに feature_name カラムを追加する（nullable TEXT）
