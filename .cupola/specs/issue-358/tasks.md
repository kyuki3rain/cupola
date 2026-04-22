# Implementation Plan

- [ ] 1. handle_status の Issue 行出力に CI修復回数を追加する
- [ ] 1.1 Issue 行フォーマットに `ci-fix: N[/MAX]` を追記する
  - `max_ci_fix_cycles: Option<u32>` の有無に応じてラベル文字列を生成する（`ci-fix: N/MAX` または `ci-fix: N`）
  - 通知保留あり・なしの両 `writeln!` にラベルを埋め込む
  - worktree パスの後にラベルを配置し、既存の ⚠ 表示は末尾に維持する
  - `ci_fix_count = 0` でも常に表示されることを確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [ ] 2. テストを整合させる
- [ ] 2.1 既存テストのアサーションを新フォーマットに対応させる
  - `handle_status` を呼ぶ既存テストのうち、出力文字列を検証するものを確認し必要なら更新する
  - フォーマット変更で破壊されるアサーションを修正する
  - _Requirements: 2.1_

- [ ] 2.2 (P) `max_ci_fix_cycles` 設定ありのケースを検証する新規テストを追加する
  - `ci_fix_count = 2`、`max_ci_fix_cycles = Some(5)` → 出力に `ci-fix: 2/5` を含む
  - `ci_fix_count = 0`、`max_ci_fix_cycles = Some(5)` → 出力に `ci-fix: 0/5` を含む
  - _Requirements: 2.2_

- [ ] 2.3 (P) `max_ci_fix_cycles` 未設定のケースを検証する新規テストを追加する
  - `ci_fix_count = 3`、`max_ci_fix_cycles = None` → 出力に `ci-fix: 3` を含み `/` を含まない
  - _Requirements: 2.3_

- [ ] 2.4 (P) 通知保留フラグが立つケースのフォーマットを検証する新規テストを追加する
  - `ci_fix_count = 6 > max = 5`、`ci_fix_limit_notified = false` → 出力に `ci-fix: 6/5 ⚠ ci-fix-limit notification pending` を含む
  - _Requirements: 2.4_
