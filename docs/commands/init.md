# `cupola init`

現在のリポジトリを Cupola 用に bootstrap するコマンド。

## 役割

- Cupola ディレクトリ（`.cupola/`）を作成する
- SQLite schema を初期化する
- `cupola.toml` の雛形を生成する
- `.cupola/.gitignore` を生成して runtime ファイルを管理する
- Claude Code 用の commands / rules / templates を導入する
- Claude Code で steering を初期化する

## オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--agent <claude-code>` | 導入対象の agent runtime | `claude-code` |
| `--upgrade` | Cupola 管理ファイルを最新版で上書きする | `false` |

現行実装では `claude-code` だけをサポートする。

## 作成・更新対象

### `.cupola/`

- `.cupola/cupola.toml`
- `.cupola/cupola.db`
- `.cupola/.gitignore`
- `.cupola/settings/.version`
- `.cupola/settings/rules/*`
- `.cupola/settings/templates/specs/*`
- `.cupola/settings/templates/steering/*`
- `.cupola/settings/templates/steering-custom/*`

### `.claude/`

- `.claude/commands/cupola/spec-compress.md`
- `.claude/commands/cupola/spec-design.md`
- `.claude/commands/cupola/spec-impl.md`
- `.claude/commands/cupola/fix.md`
- `.claude/commands/cupola/steering.md`

### `.cupola/.gitignore`

ルートの `.gitignore` は変更しない。代わりに `.cupola/.gitignore` を生成し、runtime ファイルのみを無視する。

```gitignore
cupola.db
cupola.db-wal
cupola.db-shm
cupola.pid
logs/
worktrees/
inputs/
```

これにより `.cupola/` 全体をルートの `.gitignore` に追加せずとも、committed 資産（steering / specs / settings / cupola.toml）だけが自動的に追跡される。

## 生成物の性質

- `cupola.toml` は雛形であり、`owner` / `repo` / `default_branch` は空のまま生成される
- DB schema 初期化は idempotent
- 既存ファイルがある場合、作成処理の多くは skip される（`--upgrade` 時を除く）

## Cupola 管理ファイルとユーザー所有ファイル

`init` が扱うファイルは以下の2種類に分類される。

| 種別 | 対象 | `--upgrade` 時の扱い |
|------|------|---------------------|
| Cupola 管理 | `.cupola/settings/rules/*`、`.cupola/settings/templates/*`、`.claude/commands/cupola/*`、`.cupola/.gitignore` | 上書き |
| ユーザー所有 | `.cupola/cupola.toml`、`.cupola/steering/*`、`.cupola/specs/*` | 常にスキップ |

`--upgrade` を実行すると、Cupola 管理ファイルがバイナリに同梱された最新版で上書きされる。カスタマイズしている場合は事前にバックアップを取るか、アップグレード後に差分を確認して手動で補正すること。

## steering bootstrap

最後に次のコマンドを現在リポジトリ直下で試行する。

```bash
claude --dangerously-skip-permissions -p \
  "Please run the /cupola:steering slash command to initialize the steering files for this project."
```

`-p` モードではスラッシュコマンドを直接渡せないため、コマンドを実行するよう指示するプロンプトを渡す。

- 成功時は追加メッセージなし
- `claude` 未導入なら `skipped (claude not installed)` 扱い
- その他失敗も `skipped (...)` としてレポートし、`init` 自体は継続する

## 出力

標準出力には次をまとめて表示する。

- database の初期化有無
- `cupola.toml` 作成有無
- `.cupola/.gitignore` 作成有無
- agent assets 導入有無
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
