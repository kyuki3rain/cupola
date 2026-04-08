# Cupola E2E テスト計画

`cargo test` で回しているユニット／統合テスト（`docs/tests/test-spec.md`）は
mock GitHub / mock Claude / in-memory SQLite で完結している。本ドキュメントは
**実 GitHub・実 Claude Code CLI・実 git** を使った E2E テストの準備と運用を定義する。

E2E の目的は、個々の層が正しく動くことではなく、cupola バイナリが 1 サイクル分
の polling loop を現実の環境で期待どおりに駆動できることを確認すること。
したがって E2E は**少数の代表的なシナリオ**に絞り、ユニット／統合層で詰める
べき分岐はここでは扱わない。

## 1. スコープ

### テストする
- `cupola start / stop / status` の常駐管理
- GitHub Issue の検出 → `InitializeRunning` → `DesignRunning` → `DesignReviewWaiting` のリアルな遷移
- 実 GitHub API を使った PR 作成・コメント投稿・ラベル操作
- 実 Claude Code CLI を使った spec 生成・commit・push
- 実 git worktree 操作と `cupola/{feature}/{main,design}` ブランチ生成
- DB と `process_runs` テーブルの実状態

### テストしない（ユニット／統合層で担保）
- `decide` の全遷移行（`domain::decide::tests`）
- Effect 優先度や best-effort 分類（`domain::effect::tests`）
- WorldSnapshot 構築ロジック（`application::polling::collect::tests`）
- Stale Guard / orphan recovery の DB 書き込み詳細

### 明示的に非対象
- Claude Code の応答品質
- GitHub 側の UI 表示
- 他ユーザーの trusted 判定（単一テストユーザーのみ扱う）
- 同時並行 issue の競合

## 2. 前提環境

### 2.1 ホスト要件
| 項目 | バージョン / 条件 |
|---|---|
| OS | macOS 14+ または Linux x86_64 / arm64 |
| git | 2.40+ |
| Rust toolchain | `rustc 1.94+`（`devbox run -- cargo build --release` が通ること） |
| `claude` CLI | `claude --version` が通る状態。`--dangerously-skip-permissions` が使える |
| `gh` CLI | 認証済み（`gh auth status` が Ok） |
| sqlite3 | 3.40+（DB 状態確認用、必須ではない） |
| 空きディスク | ≥ 2 GB（worktree × 複数シナリオ分） |

### 2.2 GitHub テスト対象リポジトリ（ephemeral 方式）

E2E は **run ごとに新しい private リポジトリを作成し、成功したら削除する**
ephemeral 方式で運用する。長寿命 sandbox の再利用はしない。

**方針**

| 項目 | 値 |
|---|---|
| 可視性 | private（Claude が書き込むため） |
| 命名規約 | `cupola-e2e-<YYYYMMDD-HHMMSS>-<rand3>` |
| 作成時 | `run.sh` の先頭で `gh repo create` |
| 成功時 | `gh repo delete` で自動削除 |
| 失敗時 | 削除せずに残し、名前と `.cupola/` コピー先を stdout に出す |
| デフォルトブランチ | `main`（作成直後に `README.md` だけ commit） |
| ブランチ保護 | 設定しない |
| Actions | optional（CI 失敗シナリオ E-15/E-17 を回すときだけ enable） |
| 初期 commit | 2.6 の seed リポジトリテンプレ（TS TODO CLI）を zip から展開して push |

**なぜ ephemeral**

- **決定的**: 前回 run の残骸（Issue・PR・ブランチ・DB）が一切混入しない
- **並列実行可**: 別 run は別 repo、衝突しない
- **失敗時の forensics が楽**: `--keep-repo` or 失敗検知で自動的に残すので、
  後から `gh repo view` で全状態を観察できる
- **シンプル**: 複雑な nuke スクリプトで「どこまで削るか」を考えなくていい

**マイグレーションは別レイヤ**: スキーマ移行の回帰は E2E ではなく
`cargo test` の単体テストで担保する。詳細は
[migration-test.md](./migration-test.md) を参照。E2E 側は「現在のスキーマで
動くか」だけを見る。

**ランダムサフィックス**: 同一秒に 2 本 kick しても衝突しないように 3 文字の
英数字（`[a-z0-9]{3}`）を付ける。`gh repo view` で既存チェックしてから作る。

#### ラベル
setup 時に以下のラベルを作成する:

| ラベル | 目的 |
|---|---|
| `agent:ready` | cupola がピックアップする入口 |
| `weight:light` | モデル選択テスト用 |
| `weight:heavy` | モデル選択テスト用 |

### 2.3 認証

| 項目 | 取得方法 | 必要スコープ |
|---|---|---|
| `gh auth login` の token | `gh auth login --scopes "repo,workflow,delete_repo"` | `repo`, `workflow`, **`delete_repo`** |
| cupola の GitHub token | `gh auth token` で取得できれば別途設定不要 | 上記と同じ |
| Claude Code credential | `~/.claude/` 配下に事前ログイン済み | ― |

**`delete_repo` スコープが必須**。ephemeral 方式で repo を自動削除するために
必要。既に `gh auth login` 済みでもスコープが足りない場合は以下で追加:

```bash
gh auth refresh -h github.com -s delete_repo
```

`run.sh` の冒頭で `gh auth status` をパースし、`delete_repo` が無い場合は
即座に exit 1 + 上記コマンドを案内する（`--keep-repo` 指定時のみ例外的に
スコープ不足でも続行可）。

**trusted actor**: テストを実行するユーザーが作成直後の repo の Owner に
なるため、`trusted_associations = [Owner, Member, Collaborator]` デフォルト
で動作する。

### 2.4 作業ディレクトリ

run ごとに timestamped ディレクトリを作る。成功時はアーカイブ扱いで残して
よいが、`--no-keep-dir` で削除もできる。

```
$HOME/work/cupola-e2e-run/
  20260408-153012-a3f/              ← repo 名と同じ suffix
    repo-name.txt                   ← 作成した repo のフルネーム
    target/                         ← git clone した対象リポジトリ
      .cupola/
        cupola.toml
        cupola.db
        worktrees/
        logs/
      .claude/
    cupola-logs.txt                 ← E2E ランナー自身のログ
    result.json                     ← 各シナリオの pass/fail + 所要時間
```

run ディレクトリの timestamp は repo 名の timestamp と**一致させる**。
失敗時の追跡（ディレクトリ ↔ repo）を容易にするため。

### 2.5 Cupola バイナリ
```bash
devbox run -- cargo build --release
export CUPOLA_BIN="$PWD/target/release/cupola"
```

全 E2E シナリオは `$CUPOLA_BIN` 経由で起動する。

### 2.6 設定ファイルテンプレート
`tests/e2e/fixtures/cupola.toml.tmpl` に置く想定:

```toml
owner = "${OWNER}"
repo = "${REPO}"
default_branch = "main"
language = "ja"

polling_interval_secs = 30
max_retries = 2
max_ci_fix_cycles = 2
stall_timeout_secs = 1200
max_concurrent_sessions = 1

trusted_associations = ["Owner", "Member", "Collaborator"]

[log]
level = "debug"

[model]
default = "claude-sonnet-4-6"

[models]
# weight × state-key のマッピングは空で OK（default にフォールバック）
```

`polling_interval_secs=30` は E2E 待ち時間を短縮するための値。
`max_concurrent_sessions=1` にすると検証がシンプルになる。

## 3. 事前準備スクリプト

`scripts/e2e/` 配下に以下を置く想定（未実装）。

### 3.1 `setup.sh`
単一エントリポイントは `scripts/e2e/run.sh`。内部で以下の関数を持つ。

### 3.1 `ensure_prereqs`
1. `gh auth status` を読んで `delete_repo` スコープの有無を確認
2. `claude --version`、`git --version`、`devbox run -- cargo --version` を確認
3. `$CUPOLA_BIN` が存在しなければ `devbox run -- cargo build --release`
4. 空きディスク 2GB 以上を確認

### 3.2 `create_ephemeral_repo`
1. `NAME = "cupola-e2e-$(date +%Y%m%d-%H%M%S)-$(random3)"` を生成
2. `gh repo view "$OWNER/$NAME"` で衝突チェック、衝突したら random3 を再生成（最大 3 回）
3. `gh repo create "$OWNER/$NAME" --private --add-readme`
4. `gh repo clone "$OWNER/$NAME" "$RUN_DIR/target"`
5. 2.6 の seed（TypeScript TODO CLI テンプレート）を `target/` に展開し `git commit -m "init: seed"` して `git push`
6. ラベル `agent:ready`, `weight:light`, `weight:heavy` を `gh label create`
7. `$RUN_DIR/repo-name.txt` に `OWNER/NAME` を書き込む

### 3.3 `init_cupola`
1. `cd "$RUN_DIR/target"` で `cupola init` を実行
2. `cupola.toml` テンプレートを `owner/repo` で展開
3. `cupola doctor` が Start Readiness 全項目 OK を返すことを確認

### 3.4 `run_scenarios`
1. 引数で指定されたシナリオ（なければ全件）を順次実行
2. 各シナリオの start/end 時刻と結果を `$RUN_DIR/result.json` に追記
3. いずれかが FAIL したら `FAILED=1` をセットして後続も続行（`--fail-fast` 指定時は即 return）

### 3.5 `teardown`
通常終了（全 PASS + `--keep-repo` なし）:
1. `$CUPOLA_BIN stop`（動いていれば）
2. `.cupola/logs/` と `.cupola/cupola.db` を `$RUN_DIR/` にコピーして保全
3. `gh repo delete "$OWNER/$NAME" --yes`
4. `--no-keep-dir` 指定時のみ `$RUN_DIR` を削除（デフォルトは残す）

異常終了 or `--keep-repo` 指定時:
1. `$CUPOLA_BIN stop`
2. `.cupola/logs/` と `cupola.db` を保全
3. 削除せずに `$RUN_DIR/repo-name.txt` の内容を stdout に出し、
   `gh repo view`・`gh repo delete` のコマンドを案内

### 3.6 ヘルパー関数
- `wait_for_state <issue_number> <expected_state> <timeout_secs>`: polling 間隔を加味しつつ `cupola status --all` を読んで状態到達を待つ
- `wait_for_pr <issue_number> <type=design|impl> <timeout_secs>`: GitHub API で PR が open になるのを待つ
- `sqlite_query <sql>`: `.cupola/cupola.db` に直接クエリして状態を検証
- `random3`: `tr -dc 'a-z0-9' < /dev/urandom | head -c3`

### 3.7 `run.sh` の CLI

```
usage: run.sh [OPTIONS] [SCENARIO_ID...]

Options:
  --keep-repo           success でも repo を削除しない
  --no-keep-dir         run ディレクトリを成功時に削除する
  --fail-fast           最初の失敗で以降をスキップ
  --reuse-repo NAME     既存 repo を使う（作成・削除をスキップ）
  --delete-repo NAME    指定 repo だけ削除して終了
  --owner OWNER         repo のオーナー（デフォルト: gh auth の current user）
  -h, --help
```

`--reuse-repo` は失敗後のデバッグ用。`--delete-repo` は残骸の手動掃除用。

## 4. テスト項目一覧

各項目の ID は `E-<番号>`。全シナリオとも ephemeral な新規 repo に対して
1 ランに詰め込む構成（`run.sh` が引数なしで呼ばれたとき、順に E-01 から
実行する）。

| ID | シナリオ | 担保する振る舞い |
|---|---|---|
| **E-01** | 正常系 happy path | Idle → Completed まで 1 本の Issue で通す |
| **E-02** | daemon 起動と停止 | `start --daemon` → `status` → `stop` が PID ファイル経由で機能 |
| **E-03** | 非 trusted actor によるラベル付与 | Untrusted 判定で label 削除 + reject コメントが投稿される |
| **E-04** | Design PR close（merge なし） | `DesignReviewWaiting` → `DesignRunning` に戻り新規 PR が作られる |
| **E-05** | Issue 途中クローズ | 任意の Running 状態で issue を close → `Cancelled` + cancel コメント |
| **E-06** | retry 枯渇 | `claude` を fail 固定にして `max_retries` 到達 → `Cancelled` + retry exhausted コメント |
| **E-07** | Cancelled からの reopen | `E-05` / `E-06` の後に reopen → `Idle → InitializeRunning` |
| **E-08** | `cupola cleanup` | Cancelled 状態の issue に対して cleanup を実行し worktree / branch / PR / pr_number が消える |
| **E-09** | 起動時 orphan recovery | `start --daemon` 中に `kill -9` → 再 start で `process_runs.state=running` が failed 化される |
| **E-10** | `cupola doctor` グリーン | Start Readiness / Operational Readiness がすべて OK を返す |
| **E-11** | `cupola status --all` | close_finished / active セッション数 / 行フォーマットが docs と一致 |
| **E-12** | `cupola logs -f` フォロー | ログローテーション越しに継続 tail できる |
| **E-13** | `cupola init --upgrade` | 管理ファイルのみが上書きされ、`cupola.toml` と `steering/*` は保持される |
| **E-14** | `cupola compress` | 完了済み spec を summarize できる（Claude 呼び出しを含む） |

**Smoke 最小セット**: E-01, E-02, E-09, E-10, E-11。これらだけなら ephemeral
repo 1 本を 15〜20 分で回せる。日次の smoke や PR 前確認はこのセットで十分。

**フルラン**: E-01〜E-14 を順次実行。破壊的シナリオ（E-03〜E-08）を含むため
ephemeral repo だからこそ気軽に回せる。所要 60〜90 分。

以下は **optional**（CI で回すかは判断）:

| ID | シナリオ | 条件 |
|---|---|---|
| **E-15** | CI 失敗での fixing | seed に Actions を追加し failure を返すワークフローを置く |
| **E-16** | reviewer thread での fixing | `gh pr review` で trusted unresolved thread を作る |
| **E-17** | CI 予算上限到達 | `max_ci_fix_cycles` 回失敗させ PostCiFixLimitComment を確認 |

### 4.1 シナリオ間の依存

ephemeral な 1 repo に複数シナリオを載せるため、**シナリオは独立して実行
できるよう**に設計する。依存がある場合は明示する:

- **E-07 は E-05 or E-06 に依存**: Cancelled 状態が前提なので、E-07 単独で
  走らせる場合は E-05 の手順を前段として実行する
- **E-08 は E-05 or E-06 に依存**: 同上
- それ以外は相互独立

`run.sh` は暗黙に前提シナリオを自動実行しない。ユーザーが `run.sh E-07` と
指定したら、前段が必要な旨だけ案内してエラー終了する（誤操作防止）。

## 5. 各シナリオの詳細手順

以下、代表的なシナリオを詳細化する。残りは同じテンプレートで肉付けする。

---

### E-01: 正常系 happy path

**目的**: Idle → Completed まで Issue 1 本を通す。

**前提**
- `setup.sh` 実行済み、`cupola doctor` グリーン、ラベル完備
- cupola 停止状態

**手順**
1. `gh issue create --title "E-01: add hello" --body "README に hello と書く" --label "weight:light"` でイシュー作成、`issue_number` を取得
2. `$CUPOLA_BIN start --daemon`
3. `gh issue edit $issue_number --add-label agent:ready`
4. `wait_for_state $issue_number InitializeRunning 60`
5. `wait_for_state $issue_number DesignRunning 300`
6. `wait_for_pr $issue_number design 900`
7. `gh pr merge --squash` で design PR を merge
8. `wait_for_state $issue_number ImplementationRunning 120`
9. `wait_for_pr $issue_number impl 900`
10. `gh pr merge --squash` で impl PR を merge
11. `wait_for_state $issue_number Completed 120`

**期待結果**
- Issue は GitHub 上で closed になっている
- `.cupola/worktrees/issue-<n>/` がすでに削除されている
- `cupola status --all` に当該 Issue が現れる（Completed）
- `sqlite_query "SELECT close_finished FROM issues WHERE github_issue_number=$n"` が `1`
- Design / Impl それぞれのコメントが 1 つずつ投稿されている（重複なし）

**後処理**
- `$CUPOLA_BIN stop`
- `gh pr list --state all` で残留 PR がないこと
- `cleanup.sh`

---

### E-02: daemon 起動と停止

**目的**: PID ファイル経由の排他と graceful shutdown が正しいこと。

**手順**
1. `$CUPOLA_BIN start --daemon` → 標準出力に `started cupola (pid=XXX)` が出る
2. `$CUPOLA_BIN start --daemon` 再実行 → `cupola is already running (pid=XXX)` で非 0 終了
3. `$CUPOLA_BIN status` → `Process: running (daemon, pid=XXX)` を含む
4. `$CUPOLA_BIN stop` → `stopped cupola (pid=XXX)`
5. `$CUPOLA_BIN status` → `Process: not running`
6. `.cupola/cupola.pid` が存在しない

---

### E-03: 非 trusted actor によるラベル付与

**目的**: 外部ユーザーが `agent:ready` を付けても弾かれることを保証する。

**要件**
- 第 2 の GitHub アカウント（sandbox に対して Contributor 以下）が必要
- このアカウントには sandbox の push 権限がない状態にしておく

**手順**
1. trusted ユーザーが issue を作成
2. 非 trusted ユーザーで `gh issue edit $n --add-label agent:ready`
3. `cupola start --daemon`
4. 1 サイクル待機（`sleep 45`）
5. `gh issue view $n --json labels` で `agent:ready` が削除されていること
6. `gh issue view $n --json comments` に reject コメントが 1 件あること
7. `cupola status --all` で state が `Idle` のまま
8. `stop`

---

### E-04: Design PR close（merge なし）

**目的**: ReviewWaiting 中に PR を close した場合の再作成フロー。

**手順**
1. E-01 の手順 1–6 を実施
2. Design PR が open になった直後に `gh pr close <pr>`（merge せず）
3. `wait_for_state $issue_number DesignRunning 120`
4. `wait_for_pr $issue_number design 900` で新しい PR 番号を取得
5. 取得した PR 番号が手順 2 の PR 番号と異なる
6. `sqlite_query "SELECT pr_number FROM process_runs WHERE type='design' ORDER BY id DESC LIMIT 1"` が新 PR 番号に一致

---

### E-05: Issue 途中クローズ

**手順**
1. E-01 の手順 1–5 を実施
2. `gh issue close $issue_number --reason "not planned"`
3. `wait_for_state $issue_number Cancelled 60`
4. `gh issue view $issue_number --json comments` に cancel コメントが 1 件あること
5. `.cupola/worktrees/issue-<n>/` は**残っている**（Cancelled は worktree を保持）
6. `sqlite_query "SELECT close_finished FROM issues WHERE github_issue_number=$n"` が `1`

---

### E-06: retry 枯渇

**目的**: `max_retries` 到達時に `Cancelled` + PostRetryExhaustedComment。

**工夫**
- Claude CLI そのものを壊す代わりに、**`.cupola/inputs/issue.md` がないと必ず失敗するような stub Claude スクリプト**を `claude` の PATH 上位に置いて使う
- `scripts/e2e/fake-claude-fail.sh` を PATH の先頭にする

**手順**
1. fake Claude を有効化
2. E-01 の手順 1–4 を実施
3. `wait_for_state $issue_number Cancelled 600`
4. `gh issue view` に retry exhausted コメントが存在する
5. `sqlite_query "SELECT state FROM process_runs WHERE issue_id=? AND type='design' AND state='failed'"` が 2 件（max_retries=2 の場合）

---

### E-07: Cancelled からの reopen

**手順**
1. E-05 または E-06 の後、cupola を動かしたまま `gh issue reopen $issue_number`
2. `wait_for_state $issue_number Idle 60`
3. `gh issue edit $issue_number --add-label agent:ready` がまだ付いたままなら次サイクルで `InitializeRunning` になる
4. `wait_for_state $issue_number InitializeRunning 120`

**注意**
- Cancelled → Idle は `close_finished && issue.state == open` を満たすサイクルで起こる
- label が残っていれば次サイクルで自動的に InitializeRunning に入る

---

### E-08: `cupola cleanup`

**手順**
1. E-05 状態（Cancelled）を用意
2. `$CUPOLA_BIN stop`
3. `$CUPOLA_BIN cleanup` を実行
4. 出力に対象 issue 番号と `worktree: removed / branches: removed / prs: none / metadata: cleared` 相当が含まれる
5. `git branch -r | grep cupola/` が空
6. `.cupola/worktrees/issue-<n>/` が存在しない
7. `sqlite_query "SELECT pr_number FROM process_runs WHERE issue_id=?"` の結果がすべて NULL

---

### E-09: 起動時 orphan recovery

**手順**
1. E-01 の手順 1–5 を実施（`DesignRunning` に入ったタイミング）
2. `kill -9 $(cat .cupola/cupola.pid | head -1)` で強制終了
3. `.cupola/cupola.pid` を手動削除
4. `sqlite_query "SELECT state FROM process_runs WHERE type='design' ORDER BY id DESC LIMIT 1"` が `running`
5. `$CUPOLA_BIN start --daemon`
6. 数秒待機後、同じクエリが `failed` に変わっている
7. `error_message` カラムに `orphaned` を含む文字列が入っている
8. 次サイクルで新しい design ProcessRun が再発行される

---

### E-10: `cupola doctor` グリーン

**手順**
1. `$CUPOLA_BIN doctor` を実行
2. 終了コード 0
3. 出力に `Start Readiness` セクションが存在し、5 項目すべて `✅`
4. `Operational Readiness` セクションがすべて `✅` または `⚠️`（`❌` はなし）

---

### E-11: `cupola status --all`

**手順**
1. 1 つの Issue を Completed まで進める
2. 別の Issue を Cancelled にする
3. `$CUPOLA_BIN status --all` を実行
4. Completed の行が表示される
5. Cancelled の行に `close_finished=true` が含まれる
6. `Claude sessions: 0/1`（max_concurrent_sessions=1 の場合）が表示される

---

### E-12: `cupola logs -f`

**手順**
1. `$CUPOLA_BIN start --daemon`
2. `timeout 10 $CUPOLA_BIN logs -f > /tmp/logs.out`
3. `/tmp/logs.out` に polling サイクルのログが 1 件以上ある
4. `stop`

---

### E-13: `cupola init --upgrade`

**手順**
1. `setup.sh` 済み状態で `cupola.toml` と `.cupola/steering/dummy.md` をカスタマイズ
2. `$CUPOLA_BIN init --upgrade` を実行
3. 出力に `settings/rules/*: updated`、`cupola.toml: skipped (user-owned)`、`steering/*: skipped (user-owned)` が含まれる
4. `.cupola/cupola.toml` と `.cupola/steering/dummy.md` が手順 1 のまま
5. `.cupola/settings/.version` が最新バージョンに更新されている

---

### E-14: `cupola compress`

**手順**
1. `.cupola/specs/issue-1/spec.json` に `"phase": "completed"` を持つダミーファイルを置く
2. `$CUPOLA_BIN compress` を実行
3. 出力に対象件数と spec 名が表示される
4. Claude が呼び出された形跡がある（`.cupola/logs/` に Claude invocation 行）
5. 実行後 `spec.json` の phase が `archived` に更新される（Claude 側の挙動依存）

---

## 6. 実行方法

### 6.1 単発実行
```bash
# 全 E2E（シーケンシャル）
scripts/e2e/run.sh

# 個別
scripts/e2e/run.sh E-01 E-02

# ドライラン（setup だけ）
scripts/e2e/run.sh --dry-run
```

### 6.2 実行時間の目安
| シナリオ | 所要時間 |
|---|---|
| E-01 | 15–30 分（Claude 実行時間依存） |
| E-02, E-10, E-12 | 1 分以下 |
| E-03, E-05, E-07 | 3–5 分 |
| E-04, E-08, E-09, E-11 | 5–10 分 |
| E-06 | 3–5 分 |
| E-13, E-14 | 1–2 分 |
| フルラン | 60–90 分 |

### 6.3 並列化
ephemeral 方式なので複数 run の並列実行は**名前衝突しない限り可**。
ただし Claude 同時起動数・GitHub API secondary rate limit・課金で実質的に
1〜2 並列が上限。並列させる場合は別シェルセッションで `run.sh` を叩くだけで
よい（互いに干渉しない）。

## 7. クリーンアップ

### 7.1 通常終了時（全 PASS）
`run.sh` の `teardown` が以下を実施:
1. `$CUPOLA_BIN stop`（動いていれば）
2. `.cupola/logs/` と `.cupola/cupola.db` を `$RUN_DIR/` にコピー
3. `gh repo delete "$OWNER/$NAME" --yes` で **ephemeral repo を丸ごと削除**
4. `--no-keep-dir` 指定時のみ `$RUN_DIR` を削除

ephemeral なので Issue / PR / branch / worktree の個別削除は**不要**。
repo 削除で全てが一度に消える。

### 7.2 異常終了時 / `--keep-repo` 指定時
`teardown` は削除せずに以下を実施:
1. `$CUPOLA_BIN stop`
2. `.cupola/logs/` と `.cupola/cupola.db` を保全
3. stdout に以下を出す:
   ```
   Repo is preserved for inspection:
     https://github.com/<owner>/<name>
   Run directory:
     $HOME/work/cupola-e2e-run/<timestamp>/
   To clean up later:
     scripts/e2e/run.sh --delete-repo <owner>/<name>
   ```

CI ランナーの場合は upload-artifact で `$RUN_DIR/` を吸い出してから
`gh repo delete` を走らせる（失敗時でも課金・容量を膨らませないため、
CI では `--keep-repo` を使わずに artifact 経由で forensics する）。

### 7.3 残骸の一括掃除（緊急用）
ephemeral 方式でも、異常終了が重なると `cupola-e2e-*` が複数残ることがある。
以下のワンライナーで一括削除できる:

```bash
gh repo list <owner> --limit 1000 --json name --jq '.[].name' \
  | grep '^cupola-e2e-' \
  | xargs -I{} gh repo delete "<owner>/{}" --yes
```

**警告**: 名前 prefix が `cupola-e2e-` 以外のリポジトリに対して実行しないよう、
必ず `grep '^cupola-e2e-'` のフィルタを通す。`run.sh --delete-repo` を
ラップする `scripts/e2e/sweep.sh` を用意して誤操作を防ぐ。

## 8. CI への組み込み（案）

- **デフォルトでは CI で回さない**。Claude 呼び出しが入るため認証・コスト・
  安定性の観点で適さない
- nightly の opt-in ワークフローとして用意する
  - GitHub Actions の `workflow_dispatch` のみトリガ
  - secrets: `E2E_GH_TOKEN`、`E2E_CLAUDE_CREDENTIALS`
  - 専用 runner に限定（self-hosted 推奨）
  - 失敗時は `.cupola/logs/` と `.cupola/cupola.db` を artifact に退避
- 日常の PR チェックは `cargo test` + `clippy` + `fmt` のみで十分

## 9. 既知の制約とリスク

- **Claude の出力が毎回同じとは限らない**。spec 内容の文字列 match は避ける。
  「ファイルが存在すること」「コミットされていること」「PR が作られていること」
  程度の粒度で assert する。
- **GitHub API レート制限**。E-01 を 10 回連続回すと secondary rate limit に
  当たることがある。リトライではなく sleep で回避する。
- **課金**。Claude Code のトークン消費が発生する。E2E を走らせる前に予算を
  確認する。
- **GitHub webhook 等の非同期挙動**。PR の merge 直後にすぐ状態を観測すると
  `mergeable` が `null` のまま返ることがある。wait_for_pr のリトライ間隔を
  5 秒以上にしておく。
- **タイムゾーン**。DB は UTC を使う前提。`sqlite_query` での datetime 比較
  時は必ず UTC で扱う。
- **Cancelled → Idle は 1 サイクルに 1 遷移**。`reopen` 直後に label が付いて
  いても `Idle → InitializeRunning` は次サイクルまで発火しない（docs/state-machine.md
  の該当シナリオ記載どおり）。

## 10. TODO（このドキュメントの次にやること）

- [ ] `scripts/e2e/run.sh` を実装する（`create_ephemeral_repo` / `teardown` 含む）
- [ ] `scripts/e2e/seed/`（TS TODO CLI テンプレート一式）を配置する
- [ ] `scripts/e2e/fixtures/cupola.toml.tmpl` を配置する
- [ ] `scripts/e2e/fake-claude-fail.sh`（E-06 用の stub claude）を配置する
- [ ] `scripts/e2e/sweep.sh`（`cupola-e2e-*` の一括削除）を実装する
- [ ] 最初は E-01, E-02, E-10 の smoke セットだけで動作確認し、徐々に拡大
- [ ] マイグレーションテストは別枠として [migration-test.md](./migration-test.md) に従って `tests/migrations/` を実装する
- [ ] 月次 or リリース前のみ手動で回す運用から始め、安定してから nightly 化を検討
