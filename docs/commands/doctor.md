# `cupola doctor`

Cupola を起動・運用する前提条件を診断するコマンド。

## 役割

診断結果を 2 セクションに分けて表示する。

- `Start Readiness`
- `Operational Readiness`

## チェック項目

### Start Readiness

起動に必須な項目。ここで `Fail` が 1 件でもあるとコマンドはエラー終了する。

| 項目 | 内容 |
|------|------|
| `config` | `.cupola/cupola.toml` をロードできるか |
| `git` | `git --version` が成功するか |
| `github token` | `gh auth token` が成功するか |
| `claude CLI` | `claude --version` が成功するか |
| `database` | `.cupola/cupola.db` が存在するか |

### Operational Readiness

運用時の補助資産や GitHub 側準備。ここは `Warn` が出ても `doctor` 自体は成功終了しうる。

| 項目 | 内容 |
|------|------|
| `assets version` | バイナリが期待する資産バージョンと `.cupola/settings/.version` が一致するか |
| `assets` | `.claude/commands/cupola` と `.cupola/settings` が揃っているか |
| `steering` | `.cupola/steering` に可視ファイルがあるか |
| `agent:ready label` | GitHub 上に `agent:ready` ラベルがあるか |
| `weight labels` | `weight:light` / `weight:medium` / `weight:heavy` があるか |

`assets version` が古い場合は次のように表示する。

```
⚠️ assets version  v1.0.0 → v1.2.0 available
   fix: cupola init --upgrade
```

## 既知の制約

`github token` チェックはトークンの存在のみを確認し、スコープは検証しない。Fine-grained PAT で `issues:write` を付与していない場合、doctor は通るが `CloseIssue` が失敗し続けて `Cancelled` 状態が永久停止になる。`gh auth login` の標準 OAuth フローでは通常 `issues:write` が付与されるため、一般的な運用では問題にならない。

## 表示形式

- `OK`: `✅`
- `Warn`: `⚠️`
- `Fail`: `❌`
- remediation がある場合は追加で `fix: ...` を表示する
