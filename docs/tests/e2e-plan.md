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

### 3.4 `run_narrative`
1. `scripts/e2e/scenarios/narrative.sh` を source して、`phase_0_preflight` 〜
   `phase_7_teardown` の関数を読み込む。
2. `--only <phase>` が指定されていればそのフェーズだけ、`--from <phase>` が
   指定されていればそのフェーズ以降、どちらもなければ全フェーズを順次実行。
3. 各フェーズは内部で複数のチェックポイント `CP-NN` を実行する
   （section 4 参照）。CP ごとに `result.json` に pass/fail を追記。
4. CP 単位で失敗しても次の CP は続行する（fail-fast 指定時のみ即 return）。
   フェーズ境界でも同様。
5. 全ての CP を通過した場合のみ `EXIT_CODE=0`。一つでも失敗すれば `EXIT_CODE=1`。

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
usage: run.sh [OPTIONS]

Options:
  --keep-repo           success でも repo を削除しない
  --no-keep-dir         run ディレクトリを成功時に削除する
  --fail-fast           最初の CP 失敗で以降をスキップ
  --from PHASE          指定フェーズから開始（例: --from phase_3）
  --only PHASE          指定フェーズだけ実行
  --reuse-repo NAME     既存 repo を使う（作成・削除をスキップ）
  --delete-repo NAME    指定 repo だけ削除して終了
  --owner OWNER         repo のオーナー（デフォルト: gh auth の current user）
  -h, --help
```

- 引数なしで全フェーズを順次実行する（smoke とフルランの区別はない）
- `--reuse-repo` は **失敗後のデバッグ**用。`--from phase_3` と組み合わせて
  途中フェーズから再開できる
- `--delete-repo` は **残骸の手動掃除**用。`gh repo delete` を叩いて即終了

## 4. テスト設計 — ナラティブ + チェックポイント

E2E は **「1 repo に対する 1 本の物語」** として構成する。14 個の独立シナリオを
順次回すのではなく、1 本のストーリーの中に ~40 個の **チェックポイント (CP)**
を配置し、各 CP で特定の振る舞いを検証する。

この設計により:
- **multi-issue concurrency** が自然に踏める（ある issue が Completed した隣で
  別の issue が Running している、等）
- **セットアップが 1 回**で済む（`cupola init` / `doctor` / `start --daemon` は
  1 回だけ）
- **依存関係が仕様**になる: 「E-07 は E-05 に依存」がバグではなく物語の流れ
- **失敗の粒度が細かい**: 「E-05 失敗」より「CP-33 失敗」のほうが原因が近い

### 4.1 フェーズ構成

| Phase | 内容 | Claude | 所要 |
|---|---|---|---|
| 0 | Pre-flight（doctor / init --upgrade / daemon lifecycle） | ×  | ~2 分 |
| 1 | 単独 issue 正常系（#1: Idle → Completed） | ✓  | ~25 分 |
| 2 | PR close 復帰（#2: ReviewWaiting → 再作成 → Completed） | ✓  | ~15 分 |
| 3 | Cancel + reopen（#3: 途中 close → Cancelled → reopen → 完走） | ✓  | ~10 分 |
| 4 | Retry 枯渇 + cleanup（#4: fake-claude → Cancelled → cupola cleanup） | stub  | ~5 分 |
| 5 | Orphan recovery（#5: kill -9 → 再起動で failed 化） | ✓  | ~5 分 |
| 6 | Compress（Phase 1-3 で蓄積した spec を要約） | ✓ (可選)  | ~2 分 |
| 7 | Teardown（cupola stop + repo 削除） | ×  | ~30 秒 |

フルラン所要: **~60〜75 分**、使用する GitHub issue **5〜6 本**、ephemeral repo **1 本**。

### 4.2 チェックポイント一覧

`CP-XX` は連番 ID、`narrative.sh` 内の `check "CP-XX" "description" <command>`
形式で実装する。所属フェーズを `phase_N::` prefix で示す。

#### Phase 0: Pre-flight

| CP | 内容 |
|---|---|
| CP-00 | `cupola doctor` が exit 0 + Start Readiness 全項目 ✅ + Operational Readiness に ❌ なし |
| CP-01 | `cupola init --upgrade` 実行後、`cupola.toml` と `.cupola/steering/*` が変更されていない |
| CP-02 | `cupola init --upgrade` 実行後、`.cupola/settings/.version` が最新値 |
| CP-03 | `cupola start --daemon` の stdout に `started cupola (pid=` が含まれる |
| CP-04 | 2 回目の `cupola start --daemon` が非 0 終了、stderr に `already running` |
| CP-05 | `cupola status` に `Process: running (daemon, pid=` が含まれる |
| CP-06 | `cupola stop` の stdout に `stopped cupola` が含まれる |
| CP-07 | 停止後 `cupola status` に `Process: not running` が含まれる |
| CP-08 | `.cupola/cupola.pid` が存在しない |

（Phase 0 終了後、以降のフェーズのため `cupola start --daemon` をもう一度起動する）

#### Phase 1: 単独 issue 正常系 (Issue #1)

| CP | 内容 |
|---|---|
| CP-10 | Issue #1 作成（`weight:light`, `agent:ready`）、issue 番号を変数に保存 |
| CP-11 | #1 → `InitializeRunning` 観測（timeout 120s） |
| CP-12 | #1 → `DesignRunning` 観測（timeout 300s） |
| CP-13 | `cupola status --all` に #1 の 1 行が規定フォーマット（issue#, state, design_pr, worktree, pid alive）で出る |
| CP-14 | `cupola logs -f` を 10 秒間 tail、polling cycle ログが 1 件以上取れる |
| CP-15 | #1 の design PR が open（timeout 900s） + `process_runs.pr_number` に書き込まれている |
| CP-16 | design PR を `gh pr merge --squash` で merge |
| CP-17 | #1 → `ImplementationRunning` 観測（timeout 300s） |
| CP-18 | #1 の impl PR が open（timeout 900s） |
| CP-19 | impl PR を merge |
| CP-20 | #1 → `Completed` 観測（timeout 180s） |
| CP-21 | `.cupola/worktrees/issue-1/` が削除されている |
| CP-22 | `sqlite_query "SELECT close_finished FROM issues WHERE github_issue_number=1"` が `1` |
| CP-23 | #1 の issue コメントに `PostCompletedComment` 相当の投稿が 1 件ある |

#### Phase 2: PR close 復帰 (Issue #2)

| CP | 内容 |
|---|---|
| CP-30 | Issue #2 作成 |
| CP-31 | #2 → `DesignReviewWaiting` 観測 + design PR が open |
| CP-32 | PR 番号を `OLD_PR` に保存、`gh pr close $OLD_PR`（merge せず） |
| CP-33 | #2 → `DesignRunning` 観測（timeout 180s） |
| CP-34 | 新しい design PR が open、番号が `OLD_PR` と異なる |
| CP-35 | `process_runs` で design type の最新 pr_number が新 PR 番号に更新されている |
| CP-36 | 新 design PR を merge → #2 を最後まで走らせて `Completed` |

#### Phase 3: Cancel + reopen (Issue #3)

| CP | 内容 |
|---|---|
| CP-40 | Issue #3 作成、`DesignRunning` まで進める |
| CP-41 | `gh issue close #3 --reason "not planned"` |
| CP-42 | #3 → `Cancelled` 観測（timeout 120s） |
| CP-43 | #3 の issue コメントに `PostCancelComment` が投稿されている |
| CP-44 | `.cupola/worktrees/issue-3/` は**残っている**（Cancelled は worktree を保持） |
| CP-45 | `close_finished=1` |
| CP-46 | `gh issue reopen #3`（`agent:ready` ラベルは付いたまま） |
| CP-47 | #3 → `Idle` 観測（timeout 120s） |
| CP-48 | 次サイクルで #3 → `InitializeRunning` 観測（timeout 120s） |
| CP-49 | #3 を最後まで走らせて `Completed` まで到達することを確認 |

#### Phase 4: Retry 枯渇 + cleanup (Issue #4)

| CP | 内容 |
|---|---|
| CP-50 | PATH 先頭に `scripts/e2e/fake-claude-fail.sh` を配置、cupola を一度 stop → start |
| CP-51 | Issue #4 作成 |
| CP-52 | #4 → `Cancelled` 観測（retry exhausted、timeout 600s） |
| CP-53 | #4 の issue コメントに `PostRetryExhaustedComment` が投稿されている |
| CP-54 | `sqlite_query "SELECT COUNT(*) FROM process_runs WHERE issue_id=4 AND state='failed'"` が `max_retries` 以上 |
| CP-55 | PATH を元に戻す、`cupola stop` |
| CP-56 | `cupola cleanup` 実行、#4 が対象として出力される |
| CP-57 | `git ls-remote origin 'cupola/issue-4/*'` が空（ブランチ削除確認） |
| CP-58 | `.cupola/worktrees/issue-4/` が存在しない |
| CP-59 | `sqlite_query "SELECT pr_number FROM process_runs WHERE issue_id=4"` が全て NULL |
| CP-60 | `sqlite_query "SELECT ci_fix_count FROM issues WHERE github_issue_number=4"` が `0` |
| CP-61 | `close_finished` は変更されていない（cleanup の対象外） |
| CP-62 | `cupola start --daemon` でフェーズ 5 以降のために再開 |

#### Phase 5: Orphan recovery (Issue #5)

| CP | 内容 |
|---|---|
| CP-70 | Issue #5 作成、`DesignRunning` まで進める |
| CP-71 | `sqlite_query "SELECT state FROM process_runs WHERE issue_id=5 ORDER BY id DESC LIMIT 1"` が `running` |
| CP-72 | `kill -9 $(head -1 .cupola/cupola.pid)` で cupola を強制終了 |
| CP-73 | `rm -f .cupola/cupola.pid` |
| CP-74 | `cupola start --daemon` |
| CP-75 | 10 秒後、上記 CP-71 のクエリが `failed`、`error_message` に `orphaned` を含む |
| CP-76 | 次サイクル後、新しい design ProcessRun が state=running で INSERT されている |

#### Phase 6: Compress

| CP | 内容 |
|---|---|
| CP-80 | `.cupola/specs/issue-1/spec.json` 等、完了済みの spec が存在することを確認 |
| CP-81 | `cupola compress` 実行、stdout に検出件数と spec 名が出る |
| CP-82 | （claude が利用可能なら）`.cupola/logs/` に Claude 呼び出しログが残る。未導入なら `skipped` メッセージが出る |

#### Phase 7: Teardown

| CP | 内容 |
|---|---|
| CP-99 | `cupola stop` が正常終了 |

Phase 7 完了後、`teardown_repo` が発火して `gh repo delete` で ephemeral repo を削除する
（`--keep-repo` 指定時を除く）。

### 4.3 旧シナリオ ID との対応

旧 `E-XX` ID は廃止。参考のため対応表を残す:

| 旧 | 新 |
|---|---|
| E-01 | Phase 1 (CP-10〜23) |
| E-02 | Phase 0 (CP-03〜08) |
| E-03 | **削除** — 2 アカウント要件のため自動化不可能。手動テスト手順を `docs/tests/manual-tests.md` に別出しする |
| E-04 | Phase 2 (CP-30〜36) |
| E-05 | Phase 3 前半 (CP-40〜45) |
| E-06 | Phase 4 前半 (CP-50〜54) |
| E-07 | Phase 3 後半 (CP-46〜49) |
| E-08 | Phase 4 後半 (CP-55〜62) |
| E-09 | Phase 5 (CP-70〜76) |
| E-10 | CP-00 |
| E-11 | CP-13 |
| E-12 | CP-14 |
| E-13 | CP-01, CP-02 |
| E-14 | Phase 6 (CP-80〜82) |

optional シナリオ (E-15 CI failure / E-16 reviewer thread / E-17 ci limit) は
将来 Phase 8 として追加する余地を残す（現時点では未実装）。

### 4.4 実装方針

- `scripts/e2e/scenarios/narrative.sh` の 1 ファイルにまとめる
- 各 phase は bash 関数 `phase_N_xxx()`
- 各 CP は `check "CP-NN" "desc" <command>` または `check_eval "CP-NN" "desc" <inline code>`
- `result.json` に `{ "id": "CP-15", "phase": "phase_1", "status": "pass", "seconds": 123 }` を追記
- `--from phase_3` / `--only phase_5` を `run.sh` でサポート
- `--fail-fast` で初回失敗時に即 teardown
- 旧 `scenarios/e01_*.sh`, `e02_*.sh`, `e10_*.sh` は **削除する**

## 5. 実行方法

### 5.1 基本
```bash
# フルラン（Phase 0 → 7 を順次実行）
scripts/e2e/run.sh

# 特定フェーズから開始（失敗後の再開用）
scripts/e2e/run.sh --from phase_3

# 特定フェーズだけ実行
scripts/e2e/run.sh --only phase_0

# 失敗後に repo を保持したまま実走
scripts/e2e/run.sh --keep-repo

# 保持した repo に相乗りして Phase 5 だけ再実行
scripts/e2e/run.sh --reuse-repo cupola-e2e-20260408-153012-a3f --only phase_5

# 残骸掃除
scripts/e2e/run.sh --delete-repo cupola-e2e-20260408-153012-a3f
```

### 5.2 実行時間の目安
| Phase | 所要 |
|---|---|
| phase_0 | ~2 分 |
| phase_1 | ~25 分（Claude 依存） |
| phase_2 | ~15 分 |
| phase_3 | ~10 分 |
| phase_4 | ~5 分 |
| phase_5 | ~5 分 |
| phase_6 | ~2 分 |
| phase_7 | ~30 秒 |
| フルラン合計 | **60〜75 分** |

### 5.3 並列化
ephemeral 方式なので複数 run の並列実行は**名前衝突しない限り可**。ただし
Claude 同時起動数・GitHub API secondary rate limit・課金で実質的に 1〜2 並列が
上限。並列させる場合は別シェルセッションで `run.sh` を叩くだけでよい
（互いに干渉しない）。

## 6. クリーンアップ

### 6.1 通常終了時（全 CP PASS）
`run.sh` の `teardown` が以下を実施:
1. `$CUPOLA_BIN stop`（動いていれば）
2. `.cupola/logs/` と `.cupola/cupola.db` を `$RUN_DIR/` にコピー
3. `gh repo delete "$OWNER/$NAME" --yes` で **ephemeral repo を丸ごと削除**
4. `--no-keep-dir` 指定時のみ `$RUN_DIR` を削除

ephemeral なので Issue / PR / branch / worktree の個別削除は**不要**。
repo 削除で全てが一度に消える。

### 6.2 異常終了時 / `--keep-repo` 指定時
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

### 6.3 残骸の一括掃除（緊急用）
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

## 7. CI への組み込み（案）

- **デフォルトでは CI で回さない**。Claude 呼び出しが入るため認証・コスト・
  安定性の観点で適さない
- nightly の opt-in ワークフローとして用意する
  - GitHub Actions の `workflow_dispatch` のみトリガ
  - secrets: `E2E_GH_TOKEN`、`E2E_CLAUDE_CREDENTIALS`
  - 専用 runner に限定（self-hosted 推奨）
  - 失敗時は `.cupola/logs/` と `.cupola/cupola.db` を artifact に退避
- 日常の PR チェックは `cargo test` + `clippy` + `fmt` のみで十分

## 8. 既知の制約とリスク

- **Claude の出力が毎回同じとは限らない**。spec 内容の文字列 match は避ける。
  「ファイルが存在すること」「コミットされていること」「PR が作られていること」
  程度の粒度で assert する。
- **GitHub API レート制限**。フルランを 10 回連続回すと secondary rate limit に
  当たることがある。リトライではなく sleep で回避する。
- **課金**。Claude Code のトークン消費が発生する。E2E を走らせる前に予算を
  確認する。フルラン 1 回で目安 $1〜$3。
- **GitHub webhook 等の非同期挙動**。PR の merge 直後にすぐ状態を観測すると
  `mergeable` が `null` のまま返ることがある。wait_for_pr のリトライ間隔を
  5 秒以上にしておく。
- **タイムゾーン**。DB は UTC を使う前提。`sqlite_query` での datetime 比較
  時は必ず UTC で扱う。
- **Cancelled → Idle は 1 サイクルに 1 遷移**。`reopen` 直後に label が付いて
  いても `Idle → InitializeRunning` は次サイクルまで発火しない（docs/state-machine.md
  の該当シナリオ記載どおり）。
- **Phase 間の状態依存**。Phase 3 は Phase 2 と並行している issue が残って
  いても動作するが、Phase 4 は fake-claude を仕込むので並行 issue が汚染
  される。Phase 4 開始時点で他の issue が `*Running` 系にいないことを
  narrative.sh 側で保証する。

## 9. TODO

- [x] `docs/tests/e2e-plan.md` を narrative/CP 構造に再編
- [x] `scripts/e2e/run.sh` / `lib/` / `seed/` の骨組み
- [ ] `scripts/e2e/scenarios/narrative.sh` を実装（phase_0〜phase_7、全 CP）
- [ ] 旧 `scripts/e2e/scenarios/e01_happy_path.sh` / `e02_daemon_lifecycle.sh` / `e10_doctor_green.sh` を削除
- [ ] `scripts/e2e/fake-claude-fail.sh`（Phase 4 用 stub claude）を実装
- [ ] `run.sh` に `--from PHASE` / `--only PHASE` を実装
- [ ] 実走 1 回で初期のタイムアウト値・アサーション文字列を現実に合わせて調整
- [ ] 手動テスト手順（旧 E-03 の untrusted actor シナリオ）を `docs/tests/manual-tests.md` に別出し
- [ ] マイグレーションテストは [migration-test.md](./migration-test.md) の方針に従って別枠で実装
- [ ] 月次 or リリース前のみ手動で回す運用から始め、安定してから nightly 化を検討
