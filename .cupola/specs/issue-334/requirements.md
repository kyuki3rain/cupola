# Requirements Document

## Introduction

設計ドキュメントと実装の差異を精査した結果、ドキュメント修正が必要と判明した 2 件を修正する。対象は `docs/architecture/metadata.md` の `feature_name` 初期化主体の記述誤りと、`docs/architecture/effects.md` の `SpawnInit` 処理内容における `state=running` の記載漏れである。いずれも軽微な変更であり、実装コードの変更は一切含まない。

## Requirements

### Requirement 1: metadata.md の feature_name 初期化主体を実装に合わせて修正する

**Objective:** 開発者として、`metadata.md` の `feature_name` セクションが実際のコード挙動を正確に反映していることを確認したい。そのため、初期化タイミングと主体を実装（`collect.rs`）に合わせた記述が必要である。

#### Acceptance Criteria

1. `docs/architecture/metadata.md` の `feature_name` テーブルにおいて、タイミング列が `Collect の Discovery で新規 issue を DB 登録する時（デフォルト: \`issue-{N}\`）` となっている。
2. `docs/architecture/metadata.md` の `feature_name` テーブルにおいて、主体列が `Collect` となっている。
3. When `docs/architecture/metadata.md` を参照したとき、`feature_name` の初期化主体について誤解が生じない記述になっている。
4. The docs shall 変更後も `observations.md:107` に記載された「Collect が例外的に DB 書き込みを行う Discovery 箇所」との整合性を保つ。

### Requirement 2: effects.md の SpawnInit 処理内容に state=running を明記する

**Objective:** 開発者として、`effects.md` の `SpawnInit` 詳細が `polling-loop.md` の記述と一致していることを確認したい。そのため、INSERT 時の `state=running` が明記されている必要がある。

#### Acceptance Criteria

1. `docs/architecture/effects.md` の `SpawnInit` 処理内容セルが `ProcessRun(type=init, state=running) INSERT → fetch / worktree 作成 / ...` となっている。
2. When `docs/architecture/effects.md` を参照したとき、`SpawnInit` が `state=running` で `ProcessRun` を INSERT することが明確に読み取れる。
3. The docs shall `docs/architecture/polling-loop.md:164-179` の記述（`let run = ProcessRun { state: running, ... }`）と一貫した説明を提供する。
