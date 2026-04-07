# `cupola cleanup`

`Cancelled` 状態の Issue に紐づく worktree / branch / PR / メタデータを完全にリセットするコマンド。

## 役割

cleanup は、`close_finished = true`（CloseIssue 成功済み）の `Cancelled` Issue について「reopen 後に必ず InitializeRunning から始まる」状態を保証する強制リセット操作である。`close_finished = false` の Issue は cleanup 後も `Cancelled → Idle` 遷移条件が成立しないため、再始動には別途の回復操作が必要（後述の補足参照）。

- DB から `State::Cancelled` の Issue を取得する
- 各 Issue ごとに worktree を削除する
- `cupola/{feature_name}/main` と `cupola/{feature_name}/design` を削除する
- open な design PR / impl PR を GitHub 上で close する
- 完了した操作に応じて DB 上の関連メタデータをクリアする

## 実行前提

- DB は `.cupola/cupola.db` を使う
- DB が存在しない場合は `No database found. Run \`cupola init\` first.` を出して終了する
- `.cupola/cupola.pid` が有効な場合（daemon / foreground プロセスが動作中）は **エラーで終了する**。先に `cupola stop` を実行してから cleanup を実行すること

```
Error: cupola is running (pid=12345). Run `cupola stop` first.
```

## 削除対象

### worktree

- `issue.worktree_path` があり、かつ実在する場合だけ削除を試みる
- パスが既に存在しない場合は情報ログだけ出して続行する

### branch

各 Issue について次の 2 本を削除対象にする。

- `cupola/{feature_name}/main`
- `cupola/{feature_name}/design`

`{feature_name}` は SpawnInit が branch 作成時に確定する識別子（デフォルト `issue-{N}`、例: `issue-42`）。この値は DB に記録されており、cleanup はその記録値を参照して削除する。

branch 削除に失敗しても次の Issue の処理は継続する。

### open PR の close

ProcessRun に記録された `design` / `impl` の `pr_number` を参照し、GitHub 上で open になっている PR を close する。close の後、`pr_number` を DB からクリアする（後述の DB メタデータ更新）。

- close に失敗しても次の Issue の処理は継続する
- すでに closed / merged の PR はスキップする（`pr_number` のクリアは行う）

## DB メタデータ更新

各操作の完了状況に応じて次のフィールドをクリアする。

| フィールド | クリア条件 | 理由 |
|-----------|-----------|------|
| `worktree_path` | worktree 削除が成功または「既に存在しない」と確認できた場合 | worktree が残ったまま参照だけ失うと復旧不能になるため |
| `ci_fix_count` | 常にリセット（`0`） | 新しい PR 世代として CI 修正予算を初期化するため |
| `ProcessRun(design).pr_number` | 常にクリア（`None`） | smart routing が merged PR を検出しないようにするため |
| `ProcessRun(impl).pr_number` | 常にクリア（`None`） | 同上 |

smart routing（Collect フェーズ）は DB の `ProcessRun.pr_number` だけを参照して PR を観測する。GitHub 上にマージ済み PR が残っていても、DB 側の `pr_number` が `None` であれば Collect はその PR を検出できず、ルーティングは DesignRunning 側に倒れる。これにより **cleanup 後に reopen した Issue は必ず InitializeRunning → DesignRunning から再実行される**。

その他のフィールド（`close_finished`・`consecutive_failures_epoch`・`feature_name`）は cleanup では変更しない。これらは `Cancelled → Idle` 遷移時に状態機械が適切にリセットする。

## 出力

- 対象がなければ `対象の Cancelled Issue が見つかりませんでした`
- 対象があれば処理件数と Issue ごとの結果を表示する

## 補足

- `Completed` 遷移時に自動で行われる cleanup は polling loop 側の effect であり、このコマンドとは別物
- このコマンドは「残置された `Cancelled` Issue の後片付け」を手動で行うためのもの
- `.cupola/specs/{feature_name}/` は削除しない。spec 資産は設計ログとして committed ファイルに残す。reopen 後の SpawnInit は `spec.json`（フェーズを初期状態にリセット）と `requirements.md` を上書き再生成する。旧 `design.md` / `tasks.md` が残留しても `/cupola:spec-design` がフェーズ初期状態を読んで上書き再生成するため問題ない
- cleanup 後に Issue を reopen すると `Cancelled → Idle` 遷移が発生し、`ProcessRun.pr_number` がクリアされているためスマートルーティングは PR を検出できず、必ず InitializeRunning → DesignRunning から再実行される
- `Cancelled → Idle` 遷移時に `consecutive_failures_epoch` に現在時刻がセットされるため、retry 枯渇で Cancelled になった Issue でも cleanup なしで reopen すれば再始動できる
- `close_finished = false` のまま Cancelled になった Issue（GitHub API 権限不備等）は cleanup しても `close_finished` はリセットされないため、`Cancelled → Idle` 遷移条件が成立せず再始動できない。回復手順は `docs/architecture/metadata.md` の `close_finished` セクションを参照
