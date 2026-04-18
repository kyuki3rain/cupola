# Requirements Document

## Project Description (Input)
## 背景

Design/Impl フェーズのプロンプト（`prompt.rs`）はスキルに委譲している：

```
1. Run /cupola:spec-design {feature_name}
```

しかし Fixing フェーズのプロンプトはスキルを呼ばず、手順をインラインで記述している（`build_design_fixing_prompt` / `build_implementation_fixing_prompt`）。

一方で `/cupola:fix` スキル（`.claude/commands/cupola/fix.md`）が存在するが、cupola daemon からは使われていない。結果として同様の内容が二重管理になっている。

## 修正方針

Design/Impl と同じパターンに統一する：

1. `prompt.rs` の fixing プロンプトを `Run /cupola:fix design` / `Run /cupola:fix impl` に委譲
2. インラインの手順（ステップ、コミットメッセージ等）を削除
3. `/cupola:fix` スキルを正本とする
4. コミットメッセージの動的生成（#297）もスキル側で対応

## 影響範囲

- `src/application/prompt.rs`: `build_design_fixing_prompt` / `build_implementation_fixing_prompt` を簡略化
- `.claude/commands/cupola/fix.md`: 必要に応じて調整（コミットメッセージ動的化等）

## 関連

- #297 (fixing コミットメッセージ改善) — この Issue で吸収
- #298 (スキルの自立化と cupola のオーケストレーター化)

## Requirements

### Requirement 1: fixing プロンプトの `/cupola:fix` スキル委譲

**Objective:** Cupola デーモンの開発者として、fixing フェーズのプロンプトロジックを `/cupola:fix` スキルに委譲したい。それにより、prompt.rs とスキルの二重管理を解消し、保守性を向上させたい。

#### Acceptance Criteria

1. When `build_design_fixing_prompt` が呼び出された場合、prompt system shall `Run /cupola:fix design` 指示を含む簡略化されたプロンプトを返す
2. When `build_implementation_fixing_prompt` が呼び出された場合、prompt system shall `Run /cupola:fix impl` 指示を含む簡略化されたプロンプトを返す
3. The prompt system shall インラインのステップ記述（コミットメッセージ指示・品質チェック手順・`git add`/`git push` 手順）を fixing プロンプトから除外する
4. The prompt system shall PR番号・Issue番号・言語等のコンテキスト情報を引き続きプロンプトに含める
5. The prompt system shall output-schema セクション（レビュースレッドへの応答フォーマット指示）を引き続きプロンプトに含める
6. The prompt system shall `has_merge_conflict` フラグに基づく明示的な "Merge Conflict Resolution Required" セクションを fixing プロンプトから除外する（スキルが自律的に検出・対応するため）
7. The prompt system shall `has_merge_conflict` および `default_branch` パラメータを fixing プロンプト生成関数のシグネチャから除外する

### Requirement 2: `/cupola:fix` スキルの design/impl 対応

**Objective:** Cupola スキルの利用者として、`/cupola:fix` が設計／実装の種別に応じた適切なコミットメッセージを生成してほしい。それにより、#297 の課題（コミットメッセージの動的生成）を解消し、スキルを唯一の正本とした管理を実現したい。

#### Acceptance Criteria

1. When `/cupola:fix design` が実行された場合、fix skill shall `docs:` プレフィックスのコミットメッセージを生成する
2. When `/cupola:fix impl` が実行された場合、fix skill shall `fix:` プレフィックスのコミットメッセージを生成する
3. The fix skill shall 変更内容に基づいた動的なコミットメッセージ本文を生成する（固定文字列 "address requested changes" への依存を排除する）

### Requirement 3: テストの整合性維持

**Objective:** Cupola の品質管理担当者として、リファクタリング後もテストスイートが新しいプロンプト仕様を正しく検証することを確認したい。それにより、動作変更に伴うリグレッションを防止したい。

#### Acceptance Criteria

1. The test suite shall `Run /cupola:fix design` および `Run /cupola:fix impl` がそれぞれのフェーズのプロンプトに含まれることを検証する
2. The test suite shall 削除されたインライン手順（コミットメッセージ指示・`git add` 手順等）が fixing プロンプトに含まれないことを検証する
3. The test suite shall 削除された "Merge Conflict Resolution Required" セクションに依存する既存テストを削除または更新する
4. The test suite shall output-schema セクション（thread_id・response・resolved 等）が引き続き正しく生成されることを検証する
