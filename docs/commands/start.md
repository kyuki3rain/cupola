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
| `-d`, `--daemon` | バックグラウンド daemon として起動する | `false` |
| `--daemon-child` | daemon 子プロセスとして起動する内部フラグ。通常は直接使わない | hidden |

## パス

すべてのパスは `.cupola/` を基準に導出する。

| ファイル | パス |
|---------|------|
| 設定ファイル | `.cupola/cupola.toml` |
| DB | `.cupola/cupola.db` |
| PID ファイル | `.cupola/cupola.pid` |
| ログディレクトリ | `.cupola/logs/` |
| worktree | `.cupola/worktrees/` |

## 起動モード

### foreground

```bash
cupola start
```

- 呼び出したプロセス自身が polling loop を実行する
- PID ファイルを書いたうえでログを初期化する
- 標準エラーと日次ローテーションのログファイルへ tracing を出力する
- 終了時には best-effort で PID ファイルを削除する

### background daemon

```bash
cupola start --daemon
```

- まず親プロセスが設定検証を行い、PID ファイルを atomic create（O_CREAT|O_EXCL 相当）して排他を確保する
- 排他確保後、自分自身を `start --daemon-child` 付きで再 exec する
- 親は子 PID を表示して即終了する
- 子プロセスは `setsid()` で親端末から分離し、`stdin` / `stdout` / `stderr` は `/dev/null` に向けられる
- PID ファイルは親が確保したものを子が最終的な内容（pid + mode）で上書きする

### daemon child

内部用モード。通常の利用者は直接呼ばない。

- daemon 本体として polling loop を実行する
- 親がすでに atomic create 済みの PID ファイルに pid と mode を上書きする（衝突チェックは行わない）
- logging を初期化する

## PID ファイル

PID ファイルは 2 行フォーマット。

```
{pid}
{mode}
```

`{mode}` は `foreground` または `daemon`。`status` がこの値を読んで起動モードを表示する。

- foreground 起動時は PID ファイルを atomic create して排他を確保してから polling loop を開始する
- daemon 起動時は親が atomic create で排他を確保してから子を再 exec する（二重起動レース防止）
- 既存 PID ファイルがあり対象プロセスが生きていれば起動を拒否する（foreground および daemon 親プロセスの動作。daemon 子プロセスは衝突チェックを行わず内容上書きのみ）
- 対象プロセスが死んでいれば stale PID とみなして削除してから起動する

## 設定読み込み

`.cupola/cupola.toml` を読み込み、CLI override を反映してから `Config::validate()` を通す。

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

## 起動時の初期化

起動後は次を順に行う。

1. `.cupola/cupola.db` を開く（ファイルが存在しなければ `cupola init` 未実施としてエラー終了）
2. DB に idempotent な schema migration を適用する
3. `gh auth token` ベースで GitHub token を解決する
4. REST / GraphQL client、Issue repository、Execution log repository を組み立てる
5. Claude Code runner と Git worktree manager を組み立てる
6. `PollingUseCase` を起動する

## ログ

- logging は `init_logging()` で初期化される
- 出力先は標準エラーと、`.cupola/logs/` 配下の `cupola.YYYY-MM-DD` 形式のファイル
- `cupola logs` はこのファイル群のうち辞書順で最新のものを読む

## エラー時の挙動

- 設定ファイルの読込・検証失敗で即終了する
- PID ファイルが有効なら `cupola is already running` 系のエラーになる
- polling loop 内部で終了しても、外側で PID ファイル削除を best-effort 実行する

## Claude Code へのプロンプト

SpawnProcess が Claude Code を起動する際、状態に応じたプロンプトと output schema を渡す。

**InitializeRunning はここに含まれない。** SpawnInit は Claude Code を起動せず、`tokio::spawn` で直接 worktree 作成・ブランチ作成・spec 雛形生成を行う（`InitTaskManager` / `JoinHandle` で管理）。InitializeRunning が生成するファイルについては後述の「SpawnInit が生成するファイル」を参照。

### DesignRunning

- **役割**: 自動 design agent
- **指示内容**:
  1. `/cupola:spec-design {feature_name}` を実行して spec 成果物を生成
  2. AGENTS.md / CLAUDE.md の品質チェックを実行
  3. `git add .cupola/specs/ .cupola/steering/ && git commit && git push`
- **出力**: `pr_title` / `pr_body`（PR は system が作成）。PR body に `Related: #{issue_number}` を含める。auto-close キーワード（`Closes` 等）は使わない
- **インプットファイル**: `.cupola/inputs/issue.md`（Issue 本文）

### ImplementationRunning

- **役割**: 自動 implementation agent
- **指示内容**:
  1. `/cupola:spec-impl {feature_name}` を実行して TDD でタスクを実装
  2. AGENTS.md / CLAUDE.md の品質チェックを実行
  3. `git push`
- **出力**: `pr_title` / `pr_body`（PR は system が作成）。PR body に `Closes #{issue_number}` を含める
- **前提**: design 成果物（requirements.md / design.md / tasks.md）が現在のブランチにマージ済み

### DesignFixing / ImplementationFixing

- **役割**: 自動 review agent
- **指示内容**:
  1. `/cupola:fix design`（DesignFixing）または `/cupola:fix impl`（ImplementationFixing）を実行
  2. スキル内で input ファイルの存在を確認し、必要な対応を自律的に行う
     - `.cupola/inputs/review_threads.json` があればレビューコメントに対応
     - `.cupola/inputs/ci_errors.txt` があれば CI 失敗を修正
     - マージコンフリクトがあれば先に解消
- **出力**（ReviewComments あり）: スレッドごとに `thread_id` / `response` / `resolved` を返す。スレッドへの返信・resolve は system が行う
- **出力**（ReviewComments なし）: `{"threads": []}` を返す
- **インプットファイル**: `.cupola/inputs/review_threads.json`、`.cupola/inputs/ci_errors.txt`（causes に応じて生成）

### 共通制約

すべてのプロンプトに共通して以下を指示する。

- GitHub API（`gh` コマンド含む）は使わない
- PR の作成は行わない（system が担当）
- 成果物はすべて `language`（`cupola.toml` の設定値）で記述する

## input ファイル

SpawnProcess は Claude Code 起動前に `{worktree_path}/.cupola/inputs/` を再作成し、状態に応じたファイルを書き込む。

### `issue.md`（全状態共通）

```
# Issue #{number}: {title}

## Labels
{labels}

## Body
{body}
```

### `review_threads.json`（ReviewComments あり時）

trusted actor のレビュースレッドを JSON 形式で書き込む。非 trusted のコメントは除外される。

### `ci_errors.txt`（CiFailure あり時）

CI ログのエラー内容を書き込む。

## SpawnInit が生成するファイル

InitializeRunning では Claude Code を起動せず、以下のファイルを直接生成する。

| ファイル | 内容 |
|---------|------|
| `.cupola/specs/{feature_name}/spec.json` | spec メタデータ（feature_name、language、phase、approvals 等） |
| `.cupola/specs/{feature_name}/requirements.md` | Issue 本文を `PROJECT_DESCRIPTION` として埋め込んだ雛形 |

既存ファイルがある場合は上書きする。これらは `/cupola:spec-design` が読み込む入力となる。
