# Implementation Plan

- [ ] 1. ドメイン層の削除確認と現状検証
- [ ] 1.1 `domain/check_result.rs` が存在しないことを確認し、`domain/mod.rs` に参照がないことを検証する
  - `src/domain/` 配下のファイル一覧を確認し `check_result.rs` が存在しないことを検証
  - `grep -r "check_result\|CheckResult" src/domain/` で残存参照がないことを確認
  - `grep -r "CheckStatus" src/` で参照箇所が `doctor_use_case.rs` と `bootstrap/app.rs` の2ファイルのみであることを確認
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2. Doctor型定義の専用モジュール分離
- [ ] 2.1 `src/application/doctor_result.rs` を新規作成し、`doctor_use_case.rs` から型定義を移動する
  - `DoctorSection`、`CheckStatus`、`DoctorCheckResult` の3型を `doctor_result.rs` に定義する
  - 各型の pub 可視性を維持する
  - `doctor_use_case.rs` から同3型の定義ブロック（L8–L29相当）を削除する
  - `doctor_use_case.rs` の先頭に `use crate::application::doctor_result::{CheckStatus, DoctorCheckResult, DoctorSection};` を追加する
  - _Requirements: 2.1, 2.2_

- [ ] 2.2 `src/application/mod.rs` に `pub mod doctor_result;` を追加する
  - 既存の `pub mod doctor_use_case;` の隣に宣言を追加する
  - _Requirements: 2.3_

- [ ] 2.3 `src/bootstrap/app.rs` のインポートパスを更新する
  - `use crate::application::doctor_use_case::{CheckStatus, DoctorUseCase};` を `CheckStatus` と `DoctorUseCase` を別々にインポートするよう更新する（`CheckStatus` は `doctor_result` から、`DoctorUseCase` は `doctor_use_case` から）
  - `use crate::application::doctor_use_case::DoctorSection;` を `use crate::application::doctor_result::DoctorSection;` に更新する
  - _Requirements: 2.4_

- [ ] 3. 品質確認
- [ ] 3.1 テストと静的解析を実行してリグレッションがないことを確認する
  - `devbox run test` を実行してすべてのテストがパスすることを確認する
  - `devbox run clippy` を実行して警告がゼロであることを確認する
  - `devbox run fmt-check` を実行してフォーマット差分がないことを確認する
  - _Requirements: 2.5, 2.6_
