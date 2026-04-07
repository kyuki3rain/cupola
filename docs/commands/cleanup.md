# `cupola cleanup`

`Cancelled` 状態の Issue に紐づく worktree / branch / 一部メタデータを整理するコマンド。

## 役割

- DB から `State::Cancelled` の Issue を取得する
- 各 Issue ごとに worktree を削除する
- `cupola/{feature_name}/main` と `cupola/{feature_name}/design` を削除する
- 安全に削除できた場合だけ DB 上の関連メタデータをクリアする

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

## DB メタデータ更新

worktree と branch の削除が完了した Issue について、DB の `worktree_path` を `None` にクリアする。

ただし、worktree が残ったまま参照だけ失うと復旧不能になるため、worktree 削除が成功または「既に存在しない」と確認できた場合だけ更新する。

## 出力

- 対象がなければ `対象の Cancelled Issue が見つかりませんでした`
- 対象があれば処理件数と Issue ごとの結果を表示する

## 補足

- `Completed` 遷移時に自動で行われる cleanup は polling loop 側の effect であり、このコマンドとは別物
- このコマンドは「残置された `Cancelled` Issue の後片付け」を手動で行うためのもの

## 制約

- **open PR が残る場合がある**: Issue close 由来の Cancelled でも retry 枯渇由来でも区別せず削除する。レビュー待ち PR が open のままでも branch は削除される。PR はリモートに残るが base branch が消えるためマージ不能になる。cleanup は意図的な後片付け操作なので、この挙動は許容される
- **reopen 後の再始動**: `Cancelled → Idle` 遷移時に `consecutive_failures_epoch` がリセットされるため、retry 枯渇で Cancelled になった Issue でも cleanup なしで reopen すれば再始動できる。cleanup した場合は worktree・branch を削除しているため、再始動後はスマートルーティングで拾えるものがなく、InitializeRunning から再実行される
