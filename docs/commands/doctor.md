# `cupola doctor`

Cupola を起動・運用する前提条件を診断するコマンド。

## 役割

診断結果を 2 セクションに分けて表示する。

- `Start Readiness`
- `Operational Readiness`

## オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--config <path>` | 読み込む設定ファイルのパス | `.cupola/cupola.toml` |

## チェック項目

### Start Readiness

起動に必須な項目。ここで `Fail` が 1 件でもあるとコマンドはエラー終了する。

| 項目 | 内容 |
|------|------|
| `config` | `cupola.toml` をロードできるか |
| `git` | `git --version` が成功するか |
| `github token` | `gh auth token` が成功するか |
| `claude CLI` | `claude --version` が成功するか |
| `database` | `.cupola/cupola.db` が存在するか |

### Operational Readiness

運用時の補助資産や GitHub 側準備。ここは `Warn` が出ても `doctor` 自体は成功終了しうる。

| 項目 | 内容 |
|------|------|
| `assets` | `.claude/commands/cupola` と `.cupola/settings` が揃っているか |
| `steering` | `.cupola/steering` に可視ファイルがあるか |
| `agent:ready label` | GitHub 上に `agent:ready` ラベルがあるか |
| `weight labels` | `weight:light` / `weight:medium` / `weight:heavy` があるか |

## 表示形式

- `OK`: `✅`
- `Warn`: `⚠️`
- `Fail`: `❌`
- remediation がある場合は追加で `fix: ...` を表示する

## 補足

- DB パスや steering パスなど、いくつかのチェック対象は `--config` の親ディレクトリではなく固定の `.cupola/...` を参照する
- そのためカスタム config パスを使う運用では、現行実装の `doctor` 出力と実際の構成がずれる可能性がある
