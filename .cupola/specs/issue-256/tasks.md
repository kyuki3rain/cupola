# Implementation Plan

- [ ] 1. observe_github_issue に issue_state 引数を追加し、Idle 状態のみ check_label_actor を呼ぶよう変更する
- [ ] 1.1 observe_github_issue のシグネチャを変更し、Idle 条件分岐を追加する
  - `observe_github_issue` の引数リストに `issue_state: State` を追加する
  - `has_ready_label == true` の分岐内で `issue_state == State::Idle` かどうかを判定する
  - `issue_state != State::Idle` の場合は `check_label_actor` を呼ばず、即座に `ready_label_trusted = false` を設定する
  - `observe_issue` 内の `observe_github_issue` 呼び出し箇所に `issue.state` を渡す
  - _Requirements: 1.1, 1.2, 1.4_

- [ ] 2. テストを更新・追加する
- [ ] 2.1 (P) 既存の observe_github_issue テスト 3 件を新シグネチャに合わせて更新する
  - `closed_issue_returns_closed_snapshot`: `issue_state` 引数として任意の State（例: `State::Idle`）を渡すよう更新
  - `open_issue_with_ready_label_calls_permission_api`: `issue_state: State::Idle` を渡すよう更新
  - `open_issue_without_ready_label_is_not_trusted`: `issue_state: State::Idle` を渡すよう更新
  - _Requirements: 2.1, 2.2, 2.3, 3.2, 3.3_

- [ ] 2.2 (P) 非 Idle 状態で check_label_actor が呼ばれないことを検証するテストを追加する
  - `State::all()` から `State::Idle` を除いた全 variant に対してループまたはパラメータ化テストを実装する
  - 各ケースで `is_open=true`、`has_ready_label=true`、`issue_state=<non-Idle>` を設定し、`fetch_label_actor_login` が呼ばれないことを `AtomicBool` トラッカーで検証する
  - テスト名は明確にする（例: `non_idle_states_skip_check_label_actor`）
  - _Requirements: 1.2, 3.1_
