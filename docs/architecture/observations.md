# 観測値定義（WorldSnapshot）

## 概要

観測値はイベントではなく「今サイクルの世界の状態スナップショット」である。Collect フェーズが毎サイクル構築し、Decide フェーズに渡す。

- 観測値は「〜が起きた」ではなく「〜の状態である」という意味を持つ
- 同じ観測値は条件が成立している限り毎サイクル流れ続ける
- Decide はどの状態でどの観測値を参照するかを自ら決定する（観測値側に「チェック対象状態」の概念はない）

## WorldSnapshot

```
WorldSnapshot {
  github_issue: GithubIssueSnapshot
  design_pr:    Option<PrSnapshot>      // type=design で pr_number IS NOT NULL の最新レコードがある場合のみ
  impl_pr:      Option<PrSnapshot>      // type=impl で pr_number IS NOT NULL の最新レコードがある場合のみ
  processes:    ProcessesSnapshot
  ci_fix_exhausted: bool                // issue.ci_fix_count >= config.max_ci_fix_cycles
}
```

### GithubIssueSnapshot

```
GithubIssueSnapshot {
  state:               open | closed
  has_ready_label:     bool
  ready_label_trusted: bool             // has_ready_label=false のとき意味を持たない
  weight:              TaskWeight
}
```

- **観測元**: GitHub Issues API
- `ready_label_trusted`: 原則として、ラベル付与者の author_association が `trusted_associations` に含まれるかを確認する。`TrustedAssociations::All` 設定時は open issue に限り `true` とみなす。なお、closed issue の場合はこの一般則よりも closed issue 用ルールを優先し、設定値に関わらず常に `false` を返す

### PrSnapshot

```
PrSnapshot {
  state:               open | merged | closed
  has_review_comments: bool             // trusted actor のコメントを含む unresolved review thread が存在する
  ci_status:           ok | failure | unknown
  has_conflict:        bool             // PR の mergeable が false（null の場合は unknown 扱い → false）
}
```

- **観測元**: GitHub Pull Requests API、Review Threads API、Check Runs API
- `state`: `closed` はマージされずにクローズされた状態。`merged` とは区別する
- `has_review_comments`: スレッド内のコメントが trust 判定（`trusted_associations` によるロール判定 **OR** `trusted_reviewers` によるユーザー名判定）を1件も通過しないスレッドは、存在自体を無視する（遷移トリガーにもならず、Claude Code にも渡さない）。GitHub のレビュー決定（REQUEST_CHANGES 等）はスレッドを伴わない場合は観測されない（レビュースレッドのみ対象）
- `ci_status`: check-run の conclusion が `failure` または `timed_out` の場合に `failure`。`cancelled` は `unknown` 扱い（新しいコミット push による自動キャンセルが多く、次のランを待つ）。計算中（null）も `unknown`

**`closed` PR の扱い**: `*ReviewWaiting` または `*Fixing` 状態で PR が `closed`（マージなし）になった場合、対応する `*Running` 状態に戻して PR を再作成する。遷移ルールは [state-machine.md](./state-machine.md) の遷移テーブルを参照。

### ProcessesSnapshot

```
ProcessesSnapshot {
  init:       Option<ProcessSnapshot>
  design:     Option<ProcessSnapshot>
  design_fix: Option<ProcessSnapshot>   // 最新レコード（index が最大のもの）
  impl:       Option<ProcessSnapshot>
  impl_fix:   Option<ProcessSnapshot>   // 最新レコード（index が最大のもの）
}
```

- **観測元**: ProcessRun テーブル（DB）
- 各 type ごとに `(issue_id, type)` で最新レコードを1件取得
- 該当レコードが存在しない場合は `None`

### ProcessSnapshot

```
ProcessSnapshot {
  state:                pending | running | succeeded | failed | stale
  index:                u32
  run_id:               i64    // 最新レコードの DB primary key（SpawnProcess エフェクトで pending_run_id として使用）
  consecutive_failures: u32    // 同 type の直近連続失敗数（retry_count 相当）
}
```

**consecutive_failures の計算**:
同 `(issue_id, type)` の ProcessRun を `index` 降順で走査し、`state=failed` の連続件数を数える。`state=succeeded`、`state=stale`、`state=pending`、または `state=running` に当たった時点で停止（stale・pending は失敗ではないためカウントしない。running はまだ完了していないためカウントしない）。

**init type の注意**: `init` ProcessRun は Stale Guard の対象外（SessionManager ではなく InitTaskManager/JoinHandle で管理）。そのため `init.state` は `running | succeeded | failed` のみ取り得る（`stale` には遷移しない）。

## Collect のビルドロジック

```
for each issue in DB:
  1. GitHub Issues API → GithubIssueSnapshot を構築
  2. ProcessRun(type=design) のうち `pr_number IS NOT NULL` の最新レコードから pr_number を取得
     → pr_number が Some の場合: GitHub PR API → PrSnapshot(design_pr) を構築
       （404 → None 扱い。5xx・タイムアウト等はエラーとしてこの Issue のサイクルをスキップ）
  3. ProcessRun(type=impl) のうち `pr_number IS NOT NULL` の最新レコードから pr_number を取得
     → pr_number が Some の場合: GitHub PR API → PrSnapshot(impl_pr) を構築
       （404 → None 扱い。5xx・タイムアウト等はエラーとしてこの Issue のサイクルをスキップ）
  4. ProcessRun テーブルから各 type の最新レコードを取得 → ProcessesSnapshot を構築
     （consecutive_failures も同時に集計）
  5. issue.ci_fix_count >= config.max_ci_fix_cycles → ci_fix_exhausted を設定
  6. WorldSnapshot を組み立てて Decide に渡す
```

Collect は DB・GitHub API の読み取りのみを行う。DB への書き込みは行わない。

**既知の制約**: PR 観測は DB に `pr_number` が記録されていることが前提。DB 破損・移行ミス等で `pr_number` が失われた場合、GitHub 上に PR が存在しても観測できず、スマートルーティングが機能しない（`InitializeRunning → DesignRunning` 側に倒れ、PR 作成が試みられる）。ただし PR 作成は冪等性要件（[polling-loop.md](./polling-loop.md) 参照）によりブランチ上の既存 PR を発見すれば再作成せず pr_number のみ取得する。

**PR 削除時の挙動**: GitHub 上で PR が削除された場合、DB に `pr_number` が残っていても Collect は `design_pr / impl_pr = None` を返す。`DesignRunning / ImplementationRunning` の遷移テーブルには `process.succeeded + PR == None` の行がないため遷移なしとなり、SpawnProcess が再発火して同ブランチ上に新たなプロセスを起動する。PR 作成は冪等性要件によりブランチに対して新規 PR を作成して再び pr_number を取得する。PR 削除は通常運用では発生しないが、発生した場合は自動的に PR が再作成される。
