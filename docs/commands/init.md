# `cupola init`

現在のリポジトリを Cupola 用に bootstrap するコマンド。

## 役割

- `.cupola/` ディレクトリを作成する
- SQLite schema を初期化する
- `.cupola/cupola.toml` の雛形を生成する
- Claude Code 用の commands / rules / templates を導入する
- `.gitignore` に Cupola 用エントリを追記する
- Claude Code で `/cupola:steering` を試行する

## オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--agent <claude-code>` | 導入対象の agent runtime | `claude-code` |

現行実装では `claude-code` だけをサポートする。

## 作成・更新対象

### `.cupola/`

- `.cupola/cupola.toml`
- `.cupola/cupola.db`
- `.cupola/settings/rules/*`
- `.cupola/settings/templates/specs/*`
- `.cupola/settings/templates/steering/*`
- `.cupola/settings/templates/steering-custom/*`

### `.claude/`

- `.claude/commands/cupola/spec-compress.md`
- `.claude/commands/cupola/spec-design.md`
- `.claude/commands/cupola/spec-impl.md`
- `.claude/commands/cupola/steering.md`

### `.gitignore`

次の Cupola 管理ファイルを追記対象にする。

- `.cupola/cupola.db`
- `.cupola/cupola.db-wal`
- `.cupola/cupola.db-shm`
- `.cupola/cupola.pid`
- `.cupola/logs/`
- `.cupola/worktrees/`
- `.cupola/inputs/`

## 生成物の性質

- `.cupola/cupola.toml` は雛形であり、`owner` / `repo` / `default_branch` は空のまま生成される
- DB schema 初期化は idempotent
- 既存ファイルがある場合、作成処理の多くは skip される

## steering bootstrap

最後に次のコマンドを現在リポジトリ直下で試行する。

```bash
claude -p /cupola:steering --dangerously-skip-permissions
```

- 成功時は追加メッセージなし
- `claude` 未導入なら `skipped (claude not installed)` 扱い
- その他失敗も `skipped (...)` としてレポートし、`init` 自体は継続する

## 出力

標準出力には次をまとめて表示する。

- database の初期化有無
- `cupola.toml` 作成有無
- agent assets 導入有無
- `.gitignore` 更新有無
- steering bootstrap の結果
- 対象 agent

## `cupola.toml` 雛形の初期値

コメントとして次の主要設定例が含まれる。

- `language`
- `polling_interval_secs`
- `max_retries`
- `stall_timeout_secs`
- `max_concurrent_sessions`
- `[log]`
- `model` / `models`

詳細な意味は `docs/architecture/data-model.md` の Config 節も参照。
