# `cupola cleanup`

`Cancelled` 状態の Issue に紐づく worktree / branch / 一部メタデータを整理するコマンド。

## 役割

- DB から `State::Cancelled` の Issue を取得する
- 各 Issue ごとに worktree を削除する
- `cupola/{issue_number}/main` と `cupola/{issue_number}/design` を削除する
- 安全に削除できた場合だけ DB 上の関連メタデータをクリアする

## オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--config <path>` | DB 位置の基準になる設定ファイルパス | `.cupola/cupola.toml` |

## 実行前提

- DB は `--config` の親ディレクトリ配下の `cupola.db` を使う
- DB が存在しない場合は `No database found. Run \`cupola init\` first.` を出して終了する
- 実行時には標準出力で「daemon が動作中なら停止してから cleanup を実行してください」と警告する

このコマンド自身は daemon 停止確認を強制しない。利用者側で `cupola stop` を先に実行する前提である。

## 削除対象

### worktree

- `issue.worktree_path` があり、かつ実在する場合だけ削除を試みる
- パスが既に存在しない場合は情報ログだけ出して続行する

### branch

各 Issue について次の 2 本を削除対象にする。

- `cupola/{n}/main`
- `cupola/{n}/design`

branch 削除に失敗しても次の Issue の処理は継続する。

## DB メタデータ更新

次のフィールドを `None` にする可能性がある。

- `worktree_path`
- `design_pr_number`
- `impl_pr_number`

ただし、worktree が残ったまま参照だけ失うと復旧不能になるため、`worktree_path` を安全に消せると判断できた場合だけ更新する。

## 出力

- 対象がなければ `対象の Cancelled Issue が見つかりませんでした`
- 対象があれば処理件数と Issue ごとの結果を表示する

## 補足

- `Completed` 遷移時に自動で行われる cleanup は polling loop 側の effect であり、このコマンドとは別物
- このコマンドは「残置された `Cancelled` Issue の後片付け」を手動で行うためのもの
