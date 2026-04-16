# Requirements Document

## Introduction

`CheckResult` / `CheckStatus` が2箇所で二重定義されており、混乱を招いていた問題の解消仕様書である。

**現状分析**:
- `src/domain/check_result.rs`（`CheckResult` / `CheckStatus` Pass/Fail/Skipped）は、commit 88066b1 にて "doctor 以外で未使用" として削除済み
- `src/application/doctor_use_case.rs` には `CheckStatus`（Ok/Warn/Fail）、`DoctorCheckResult`、`DoctorSection` がインライン定義されており、992行の大きなファイルになっている
- `src/bootstrap/app.rs` は `doctor_use_case::CheckStatus` および `doctor_use_case::DoctorSection` をインポートして使用している

本リファクタリングの目的は2点ある。
1. ドメイン層の重複型定義が解消済みであることを確認・確定する
2. アプリケーション層内の Doctor 型定義を専用モジュールへ分離し、単一責務の原則を満たす

## Requirements

### Requirement 1: ドメイン層の重複型定義の解消確認

**Objective:** As a developer, I want to confirm that the duplicate `CheckStatus` definitions across domain and application layers have been eliminated, so that there is a single source of truth for doctor-related types.

#### Acceptance Criteria

1.1. The system shall not define `CheckStatus`, `CheckResult`, or any doctor check types in `src/domain/`.

1.2. When the codebase is inspected, `src/domain/check_result.rs` shall not exist.

1.3. The system shall have exactly one definition of `CheckStatus` (Ok / Warn / Fail variants) in the application layer.

### Requirement 2: Doctor型の専用モジュールへの分離

**Objective:** As a developer, I want doctor-related types (`CheckStatus`, `DoctorCheckResult`, `DoctorSection`) to be defined in a dedicated module separate from the use case logic, so that responsibility boundaries are clear and files follow the single-responsibility principle.

#### Acceptance Criteria

2.1. The system shall define `CheckStatus`、`DoctorCheckResult`、`DoctorSection` を `src/application/doctor_result.rs` という専用ファイルに移動する。

2.2. When `doctor_use_case.rs` is inspected, it shall not contain inline definitions of `CheckStatus`, `DoctorCheckResult`, or `DoctorSection`.

2.3. The system shall declare `pub mod doctor_result` in `src/application/mod.rs`.

2.4. When `bootstrap/app.rs` uses doctor types, it shall import them from `crate::application::doctor_result` instead of `crate::application::doctor_use_case`.

2.5. When `devbox run test` is executed after the refactoring, all existing tests shall pass without any modifications to test logic.

2.6. If `devbox run clippy` is run after the refactoring, the system shall produce no warnings.
