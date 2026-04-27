# Implementation Plan

- [ ] 1. (P) metadata.md の feature_name テーブルを実装に合わせて修正する
  - `docs/architecture/metadata.md` の `feature_name` テーブルを開き、タイミング列を `Collect の Discovery で新規 issue を DB 登録する時（デフォルト: \`issue-{N}\`）` に、主体列を `Collect` に変更する
  - 変更後に `docs/architecture/observations.md:107` との整合性を目視確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2. (P) effects.md の SpawnInit 処理内容に state=running を追記する
  - `docs/architecture/effects.md` の `SpawnInit` セクション（処理内容セル）の `ProcessRun(type=init)` を `ProcessRun(type=init, state=running)` に修正する
  - 変更後に Markdown テーブルのフォーマットが崩れていないこと、および `docs/architecture/polling-loop.md:164-179` との整合を目視確認する
  - _Requirements: 2.1, 2.2, 2.3_
