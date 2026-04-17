# Requirements Document

## Project Description (Input)
## 背景

#215 の代替案。TTL キャッシュよりシンプルで効果的な最適化。

## 現状

`observe_github_issue` で `check_label_actor` が呼ばれる条件:
- `is_open == true`
- `has_ready_label == true`

**`issue.state` を参照していない**ため、`agent:ready` ラベルが残ったまま active 状態（`InitializeRunning`, `DesignRunning` 等）の issue では毎サイクル 2 API コールが走る:
- `fetch_label_actor_login` (Timeline API)
- `fetch_user_permission` (Permission API)

## 問題

`ready_label_trusted` を消費するのは `decide_idle` のみ。non-Idle 状態では**結果が使われない API コール**が積み重なっている。

## 提案

`issue.state == State::Idle` の時だけ `check_label_actor` を呼ぶ:
- Idle issue: 従来通り（decide_idle で消費される）
- active issue: スキップ（結果が使われないため）
- terminal issue: is_open=true の場合も、`decide_cancelled` では `ready_label_trusted` を参照しないためスキップ可
- Cancelled → Idle 遷移: その同サイクルでは必要ないが、次サイクルで state=Idle になってから呼ばれるので正しく動作する

## 影響範囲

- `src/application/polling/collect.rs`
  - `observe_github_issue` のシグネチャに `issue_state: State` を追加
  - Idle のみ `check_label_actor` を呼ぶ
- テスト更新

## 期待される削減効果

active 状態が長期化する issue（例: 30 サイクル）で、1 issue あたり 60 コール削減。

## 優先度

低〜中。実用上の影響は限定的だが、実装が単純なので労力対効果は良い。

Related: #203, closed #215, closed #214, PR #255

## Requirements

## はじめに

本ドキュメントは、ポーリングループにおける `check_label_actor` 呼び出しの最適化に関する要件を定義する。
`ready_label_trusted` の結果を利用するのは `decide_idle` のみであるため、`issue.state == State::Idle` の場合のみ `check_label_actor` を呼び出すよう変更する。
これにより、active および terminal 状態の issue で発生していた不要な GitHub API 呼び出し（Timeline API + Permission API）を削減する。

## Requirements

### Requirement 1: Idle 状態限定のラベルアクター検証

**Objective:** Cupola ポーリングシステムの開発者として、Idle 状態の issue に対してのみ `check_label_actor` を呼び出したい。それにより、active・terminal 状態で発生していた無駄な API コールを排除できる。

#### Acceptance Criteria

- **1.1**: While `issue.state == State::Idle` and `has_ready_label == true`, the Polling System shall call `check_label_actor` to verify the label actor's trust
- **1.2**: While `issue.state != State::Idle` and `has_ready_label == true`, the Polling System shall skip `check_label_actor` and set `ready_label_trusted` to `false`
- **1.3**: When `issue.state` transitions to `State::Idle` (e.g., after Cancelled → Idle), the Polling System shall call `check_label_actor` in the next polling cycle where the state is read as `Idle`
- **1.4**: The Polling System shall pass `issue.state` to the label actor verification step so that the function can determine whether to call `check_label_actor`

### Requirement 2: 既存動作の維持（Idle 状態）

**Objective:** Cupola ポーリングシステムの開発者として、Idle 状態における `check_label_actor` の既存の動作を維持したい。それにより、Idle issue の信頼性検証フローが壊れないことを保証できる。

#### Acceptance Criteria

- **2.1**: While `issue.state == State::Idle`, `has_ready_label == true`, and `TrustedAssociations::All`, the Polling System shall return `ready_label_trusted: true` without calling Timeline API or Permission API
- **2.2**: While `issue.state == State::Idle`, `has_ready_label == true`, and `TrustedAssociations::Specific(...)`, the Polling System shall call Timeline API and Permission API and return trust based on the result
- **2.3**: While `issue.state == State::Idle` and `has_ready_label == false`, the Polling System shall return `ready_label_trusted: false` without API calls
- **2.4**: If label actor verification fails due to a GitHub API error while `issue.state == State::Idle`, the Polling System shall log a warning and treat the issue as untrusted

### Requirement 3: テストカバレッジ

**Objective:** Cupola ポーリングシステムの開発者として、非 Idle 状態での API 非呼び出しと Idle 状態での API 呼び出しを網羅的にテストしたい。それにより、最適化が回帰なく機能することを保証できる。

#### Acceptance Criteria

- **3.1**: The Polling System shall have unit tests verifying that the external label actor API is NOT called for each non-Idle state when `has_ready_label == true`
- **3.2**: The Polling System shall have unit tests verifying that the external label actor API IS called for `State::Idle` with `has_ready_label == true` and specific trust associations configured
- **3.3**: The Polling System shall have unit tests verifying that the external label actor API is NOT called for `State::Idle` with `has_ready_label == false`
