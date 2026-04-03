# Requirements Document

## Introduction

本番コードに残存している `expect()` 呼び出しをエラー伝播パターン `.context()?` に置き換え、デーモン全体がpanicでクラッシュするリスクを排除する。加えて、`clippy::expect_used` lint を追加して今後の再混入を静的に防止する。

対象は `src/application/prompt.rs` の2箇所と `src/application/transition_use_case.rs` の1箇所の合計3箇所。テストコードでの `expect()` 使用は引き続き許可する。

## Requirements

### Requirement 1: prompt.rs の expect() をエラー伝播に変更

**Objective:** 開発者として、`build_session_config` が `expect()` でpanicするのではなく `Result` を返してほしい。そうすることで、DesignFixing・ImplementationFixing 状態でDBに `pr_number` が存在しない場合に対象Issueのspawnのみをスキップし、他のIssueの処理を継続できる。

#### Acceptance Criteria

1. The cupola system shall change the return type of `build_session_config` from `SessionConfig` to `Result<SessionConfig>`.
2. When `build_session_config` is called in DesignFixing state and `pr_number` is `None`, the cupola system shall return `Err` with context message `"fixing state requires pr_number in DB"` instead of calling `expect()`.
3. When `build_session_config` is called in ImplementationFixing state and `pr_number` is `None`, the cupola system shall return `Err` with context message `"fixing state requires pr_number in DB"` instead of calling `expect()`.
4. When `build_session_config` returns `Err` in `step7_spawn_processes`, the cupola system shall skip spawning for that Issue and continue processing other Issues.
5. If `build_session_config` returns `Err` in `step7_spawn_processes`, the cupola system shall log the error at warn or error level before skipping.

### Requirement 2: transition_use_case.rs の expect() をエラー伝播に変更

**Objective:** 開発者として、`reset_for_restart` 直後の `find_by_id` で `expect()` が使われているのを `?` によるエラー伝播に変更してほしい。そうすることで、万が一DBの不整合が発生した場合でもデーモン全体がpanicせずエラーとして処理される。

#### Acceptance Criteria

1. When `find_by_id` returns `None` after `reset_for_restart`, the cupola system shall return `Err` with context message `"issue not found after reset_for_restart"` instead of calling `expect()`.
2. While `step6_apply_events` handles the error from this call via `if let Err(e)`, the cupola system shall propagate the error upward without additional error-handling changes in the call site.
3. The cupola system shall not change the behavior of the surrounding `step6_apply_events` error handling logic.

### Requirement 3: clippy lint による再発防止

**Objective:** 開発者として、本番コードへの `expect()` 混入を静的解析で検出・拒否したい。そうすることで、コードレビューや CI で自動的に再発を防止できる。

#### Acceptance Criteria

1. The cupola system shall add `expect_used = "deny"` under `[lints.clippy]` in `Cargo.toml`.
2. The cupola system shall add `[lints.clippy] all = "warn"` in `Cargo.toml` if not already present.
3. Where test code is present, the cupola system shall allow `clippy::expect_used` via `#![cfg_attr(test, allow(clippy::expect_used))]` in `src/lib.rs`.
4. When `cargo clippy` is run, the cupola system shall report a `deny`-level error for any new `expect()` call in non-test production code.
5. The cupola system shall not flag `expect()` usage inside `#[cfg(test)]` blocks as an error.

### Requirement 4: 対象外 expect() の非変更

**Objective:** 開発者として、変更対象外の `expect()` は現状維持したい。そうすることで、不必要な設計変更を避け変更範囲を最小に保てる。

#### Acceptance Criteria

1. The cupola system shall NOT modify `src/application/session_manager.rs` line 86 (`entry.child.wait().expect("wait after kill")`).
2. While `session_manager.rs` の `expect()` はkill直後のwaitで実質到達不可能であるため、the cupola system shall leave this call site unchanged.
