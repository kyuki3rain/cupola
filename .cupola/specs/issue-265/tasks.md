# Implementation Plan

- [x] 1. ドメイン層の削除確認と現状検証
- [x] 1.1 ドメイン層にチェック結果型の定義や参照が残存していないことを検証する
  - ドメイン層のモジュール一覧を確認し、チェック結果に関するモジュールが存在しないことを検証する
  - ドメイン層内にチェック結果型への参照が残っていないことを確認する
  - チェックステータス型がユースケース層とエントリポイント層のみで参照されていることを確認する
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 2. Doctor型定義の専用モジュール分離
- [x] 2.1 Doctor結果型をユースケース実装とは独立した専用モジュールに分離する
  - Doctor診断に関する結果型（セクション、ステータス、チェック結果）を専用モジュールとして定義する
  - 分離後も各型の公開インターフェースが維持されていることを確認する
  - ユースケース実装から型定義が除かれ、専用モジュールへの依存に置き換えられていることを確認する
  - _Requirements: 2.1, 2.2_

- [x] 2.2 `src/application/mod.rs` に `pub mod doctor_result;` を追加する
  - 既存の `pub mod doctor_use_case;` の隣に宣言を追加する
  - _Requirements: 2.3_

- [x] 2.3 `src/bootstrap/app.rs` のインポートパスを更新する
  - `use crate::application::doctor_use_case::{CheckStatus, DoctorUseCase};` を `CheckStatus` と `DoctorUseCase` を別々にインポートするよう更新する（`CheckStatus` は `doctor_result` から、`DoctorUseCase` は `doctor_use_case` から）
  - `use crate::application::doctor_use_case::DoctorSection;` を `use crate::application::doctor_result::DoctorSection;` に更新する
  - _Requirements: 2.4_

- [x] 3. 品質確認
- [x] 3.1 テストと静的解析を実行してリグレッションがないことを確認する
  - `devbox run test` を実行してすべてのテストがパスすることを確認する
  - `devbox run clippy` を実行して警告がゼロであることを確認する
  - `devbox run fmt-check` を実行してフォーマット差分がないことを確認する
  - _Requirements: 2.5, 2.6_
