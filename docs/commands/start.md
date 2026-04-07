# `cupola start`

Cupola 本体を起動し、GitHub Issue を監視する polling loop を開始するコマンド。

## 関連アーキテクチャ

`start` が起動する本体処理は `docs/architecture` に整理されている。

- 全体像: [../architecture/README.md](../architecture/README.md)
- polling loop: [../architecture/polling-loop.md](../architecture/polling-loop.md)
- 状態遷移: [../architecture/state-machine.md](../architecture/state-machine.md)
- 副作用: [../architecture/effects.md](../architecture/effects.md)
- メタデータ更新: [../architecture/metadata.md](../architecture/metadata.md)

foreground / background、PID ファイル、ログ初期化などのプロセス管理まわりはこのコマンド固有の仕様であり、上記アーキテクチャ文書の外側にある。

## 役割

- 設定ファイルを読み込んで `Config` を構築する
- SQLite / GitHub client / Claude Code runner / worktree manager を初期化する
- polling loop を起動する
- foreground 実行、daemon 親プロセス、daemon 子プロセスの 3 モードを切り替える

## オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `--polling-interval-secs <u64>` | `cupola.toml` の `polling_interval_secs` を一時的に上書きする | 設定ファイルの値 |
| `--log-level <trace\|debug\|info\|warn\|error>` | `cupola.toml` のログレベルを一時的に上書きする | 設定ファイルの値 |
| `--config <path>` | 設定ファイルのパス | `.cupola/cupola.toml` |
| `-d`, `--daemon` | バックグラウンド daemon として起動する | `false` |
| `--daemon-child` | daemon 子プロセスとして起動する内部フラグ。通常は直接使わない | hidden |

## 起動モード

### foreground

`cupola start`

- 呼び出したプロセス自身が polling loop を実行する
- PID ファイルを書いたうえでログを初期化する
- 標準エラーと日次ローテーションのログファイルへ tracing を出力する
- 終了時には best-effort で PID ファイルを削除する

### background daemon

`cupola start --daemon`

- まず親プロセスが設定検証と PID ファイル事前確認だけを行う
- その後、自分自身を `start --daemon-child` 付きで再 exec する
- 親は子 PID を表示して即終了する
- 子プロセスは `setsid()` で親端末から分離し、`stdin` / `stdout` / `stderr` は `/dev/null` に向けられる

### daemon child

内部用モード。通常の利用者は直接呼ばない。

- daemon 本体として polling loop を実行する
- foreground と同様に PID ファイルと logging を初期化する
- DB パスは `--config` の親ディレクトリ配下の `cupola.db` を使う

## PID ファイル

- PID ファイルの場所は常に `--config` の親ディレクトリ配下の `cupola.pid`
- 例: `--config .cupola/cupola.toml` なら PID ファイルは `.cupola/cupola.pid`
- 起動時に既存 PID を確認し、対象プロセスが生きていれば起動を拒否する
- 対象プロセスが死んでいれば stale PID とみなして削除してから起動する
- foreground と daemon は同じ PID ファイル規約を共有する

## 設定読み込み

`--config` の TOML を読み込み、CLI override を反映してから `Config::validate()` を通す。

`start` で実際に影響する主な設定:

- `owner`
- `repo`
- `default_branch`
- `language`
- `polling_interval_secs`
- `max_retries`
- `max_ci_fix_cycles`
- `stall_timeout_secs`
- `max_concurrent_sessions`
- `model` / `models`
- `trusted_associations`
- `[log].level`
- `[log].dir`

## 起動時の初期化

起動後は次を順に行う。

1. `.cupola/cupola.db` を開く
2. DB schema を初期化する
3. `gh auth token` ベースで GitHub token を解決する
4. REST / GraphQL client、Issue repository、Execution log repository を組み立てる
5. Claude Code runner と Git worktree manager を組み立てる
6. `PollingUseCase` を起動する

## ログ

- logging は `init_logging()` で初期化される
- 出力先は標準エラーと、`[log].dir` 配下の `cupola.YYYY-MM-DD` 形式のファイル
- `cupola logs` はこのファイル群のうち辞書順で最新のものを読む

## エラー時の挙動

- 設定ファイルの読込・検証失敗で即終了する
- PID ファイルが有効なら `cupola is already running` 系のエラーになる
- polling loop 内部で終了しても、外側で PID ファイル削除を best-effort 実行する

## 実装上の注意

- foreground 起動時の DB パスは現在固定で `.cupola/cupola.db`
- daemon child 起動時の DB パスは `--config` の親ディレクトリ配下の `cupola.db`
- そのため `--config` をカスタムした場合、foreground と daemon で DB 解決先が一致しない可能性がある。これは現行実装の挙動である
