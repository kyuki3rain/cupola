# リサーチ・設計決定ログ

---
**Purpose**: 技術設計の根拠となる調査結果・アーキテクチャ検討・設計決定の記録。
**Feature**: `issue-323` — execute.rs 分割と ExecuteContext 導入
**Discovery Scope**: Extension（既存 application 層ファイルの機械的リファクタ）

---

## Summary

- **Feature**: execute.rs 分割と ExecuteContext 導入（issue-323）
- **Discovery Scope**: Extension（既存コードの機械的リファクタ。ロジック変更なし）
- **Key Findings**:
  - `execute.rs` は 2869 行に成長しており、12 の関数が 1 ファイルに集約されている
  - `execute_effects` / `execute_one` / `spawn_init_task` / `perform_init_sync` / `spawn_process` / `prepare_process_spawn` の 6 関数に `#[allow(clippy::too_many_arguments)]` が点在
  - 呼び出し元は `polling_use_case.rs` のみ（パブリック API は `execute_effects` 関数 1 本）
  - テストは `#[cfg(test)]` mod 内で非公開関数を `super::` 経由で直接参照しており、モジュール分割後は各 executor ファイルにテストを移動する必要がある

## Research Log

### execute.rs の現状構造分析

- **Context**: 分割対象ファイルの全関数・行数・依存関係を把握するため調査
- **Sources Consulted**: `src/application/polling/execute.rs` 全体（2869 行）を直接読解
- **Findings**:
  - `execute_effects`（L109-170）: エントリポイント、9 引数、`#[allow(clippy::too_many_arguments)]`
  - `execute_one`（L173-391）: 内部ディスパッチ、9 引数、210 行超の match、`#[allow(clippy::too_many_arguments)]`
  - `spawn_init_task`（L394-452）: SpawnInit の claim ライフサイクル管理、`#[allow(clippy::too_many_arguments)]`
  - `prepare_init_handle`（L459-544）: SpawnInit の async I/O ヘルパー
  - `perform_init_sync`（L548-579）: SpawnInit の同期 git 操作（blocking task 内）、`#[allow(clippy::too_many_arguments)]`
  - `spawn_process`（L582-667）: SpawnProcess の reservation ライフサイクル管理、`#[allow(clippy::too_many_arguments)]`
  - `prepare_process_spawn`（L676-813）: SpawnProcess の async I/O ヘルパー、`#[allow(clippy::too_many_arguments)]`
  - `handle_body_tampered`（L822-874）: 本文改ざん検知時の処理（`spawn_process` から呼び出し）
  - `get_pr_number_for_type`（L876-889）: 共通ヘルパー
  - `state_from_phase`（L891-906）: 共通ヘルパー
  - `phase_for_type`（L908-917）: 共通ヘルパー
  - `find_last_error`（L919-932）: 共通ヘルパー
  - `tests` モジュール（L934-2869）: 非公開関数を `super::` 経由でテスト
- **Implications**: テスト移動が必須。各 executor ファイルに独自のモック型が必要

### テストの可視性分析

- **Context**: `#[cfg(test)]` テストがどの非公開関数をどう参照しているか確認
- **Findings**:
  - `super::sha256_hex` → mod.rs に残すため移動不要
  - `super::perform_init_sync` → spawn_init_executor.rs に移動。テストも同モジュールへ
  - `super::prepare_init_handle` → spawn_init_executor.rs に移動。テストも同モジュールへ
  - `super::prepare_process_spawn` → spawn_process_executor.rs に移動。テストも同モジュールへ
  - モック型（`MockGitWorktree`、`MockFileGenerator`）は `perform_init_sync` / `prepare_init_handle` テストで使用 → spawn_init_executor.rs に移動
  - モック型（`MockGitHubForInit`、`MockProcRepo`）は `prepare_init_handle` テストで使用 → spawn_init_executor.rs に移動
  - `effect_priority_order`、`best_effort_classification` テストは Effect の domain ロジックのテスト → mod.rs の `#[cfg(test)]` に残す
- **Implications**: テストモックの重複を避けるため、各 executor 内でモックを独立定義する。将来的にテスト用ユーティリティモジュールに集約することは本 issue のスコープ外

### SpawnableGitWorktree トレイトの配置確認

- **Context**: `pub trait SpawnableGitWorktree` の公開範囲と参照元を確認
- **Findings**: `execute.rs` L101-102 で `pub trait SpawnableGitWorktree` として定義。`execute_effects` の where 句で使用。`polling_use_case.rs` が `execute_effects` を呼び出す際に型推論で解決される
- **Implications**: mod.rs で公開定義を維持することで後方互換性を保てる

### handle_body_tampered の配置検討

- **Context**: `handle_body_tampered` は `spawn_process` から呼ばれる内部関数であり、配置モジュールを決定する必要がある
- **Findings**: `spawn_process` 関数のみから呼ばれる。引数は `github: &G`、`issue_repo: &I`、`config: &Config`、`issue: &mut Issue` の 4 つ
- **Implications**: `spawn_process_executor.rs` 内の非公開関数（`pub(super)` 不要）として自然に配置できる。ExecuteContext が渡されていれば ctx から分解することも可能だが、内部関数なので直接引数で受け取る形のまま移動が最小変更となる

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Flat file 分割のみ | 各 Effect handler を別ファイルに移動。Context なし | シンプル | 引数多数問題が残る。将来の拡張時にも全関数シグネチャ変更が必要 | Issue の要件を満たさない |
| ExecuteContext + モジュール分割 | Context struct で引数を束ね、モジュール分割 | clippy 警告解消、#338 の前提構造。将来の EffectLog 追加がフィールド追加だけで完了 | generic 6 個の where 句が verbose になる | Issue 指定のアプローチ。採用 |
| dyn Trait による Context | Box\<dyn Trait\> で引数を束ねる | where 句が不要になる | ランタイムコスト・vtable、オブジェクトセーフ制約（async fn は対象外）。application 層の静的保証が失われる | 採用しない |

## Design Decisions

### Decision: ExecuteContext の generic パラメータ数

- **Context**: application 層は具体型を知らず、port trait にのみ依存する必要がある（Clean Architecture 制約）
- **Alternatives Considered**:
  1. `dyn Trait` による動的ディスパッチ — async trait はオブジェクトセーフではなく不可
  2. 6 generic パラメータ（G, I, P, C, W, F）— コンパイル時解決、ゼロコスト
- **Selected Approach**: generic param 6 個（G, I, P, C, W, F）
- **Rationale**: Clean Architecture の依存方向制約（application は port のみ依存）を維持しつつゼロコスト抽象化を実現。既存コードの where 句と整合性が取れる
- **Trade-offs**: where 句が verbose になるが、各ファイルの `where G: GitHubClient, I: IssueRepository, ...` は機械的に繰り返すだけで複雑さはない
- **Follow-up**: 実装後に clippy で `#[allow]` が完全に消えているか確認

### Decision: テストの分散配置

- **Context**: 現 `execute.rs` のテストが `super::` で複数の非公開関数を参照している。分割後にどこに配置するかを決定する必要がある
- **Alternatives Considered**:
  1. 全テストを mod.rs に残す — 各 executor の非公開関数を `crate::application::polling::execute::spawn_init_executor::perform_init_sync` のようにパスで参照する必要があり、可視性の変更が必要になる
  2. テストを各 executor モジュールの `#[cfg(test)]` に移動 — 各モジュールが自己完結し、`super::` 参照がそのまま使える
- **Selected Approach**: テストを各 executor モジュールの `#[cfg(test)]` に分散移動
- **Rationale**: 各ファイルが自己完結し、テストと実装が隣接する。可視性変更が最小
- **Trade-offs**: モック型（MockGitWorktree 等）の一部が複数モジュールで重複する可能性があるが、テストコードのため許容範囲

### Decision: mod.rs への残留要素の選定

- **Context**: mod.rs に何を残すかを明確にする必要がある
- **Findings**:
  - `execute_effects`（パブリック API）: mod.rs に必須
  - `retry_db_update`（executor から `super::` 参照）: mod.rs に残す
  - `BodyTamperedError`（spawn_process_executor から `super::` 参照）: mod.rs に残す
  - `sha256_hex`（spawn_process_executor と mod.rs テストから参照）: mod.rs に残す
  - `SpawnableGitWorktree`（パブリックトレイト）: mod.rs に残す
- **Selected Approach**: 上記 5 つを mod.rs に残す
- **Rationale**: 参照元から `super::` で自然にアクセスでき、移動コストがゼロ

## Risks & Mitigations

- テスト参照の可視性エラー — 各 executor ファイル内にテストを移動することで、`super::` 参照をそのまま使い解消
- `handle_body_tampered` の配置が不明確 — `spawn_process_executor.rs` の非公開関数として配置。ctx 分解は行わず現状の引数リストを維持
- generic bounds の重複記述 — 機械的に where 句をコピーするのみ。将来的な改善は #338 スコープ
- ファイル行数 500 行制限 — `spawn_process_executor.rs` が最大のリスク（`prepare_process_spawn` が 130+ 行、`spawn_process` が 85+ 行、テスト含むと超過の可能性）。テストを合わせても 500 行以内に収まることを実装時に確認する

## References

- `src/application/polling/execute.rs` — 分割対象ファイル（2869 行）
- Issue #338 — 将来の EffectLog port 追加の構想（#323 が前提）
- Issue #322 — decide.rs 分割（対をなすリファクタ）
- Issue #316 — PostCiFixLimitComment 通知欠落（comment_executor.rs に集約）
- Issue #311 — stdout/stderr stream-to-file（spawn 系 executor に局所化）
