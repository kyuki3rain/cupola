# Cupola 要件定義書

## Introduction

GitHub Issue/PR を起点に Claude Code + cc-sdd を駆動して設計・実装を自動化するローカル常駐エージェント「Cupola」の初期実装。Rust で Clean Architecture に基づき、ステートマシン駆動の polling ループで Issue 検知→設計→レビュー→実装→完了の全工程を自動化する。SQLite による状態管理、GitHub REST/GraphQL API 連携、git worktree による Issue 単位の独立作業環境を提供する。

本ドキュメントは [ccforge/docs/v2/requirements.md](../../../../ccforge/docs/v2/requirements.md) v0.4 を原本として取り込み、cc-sdd の EARS 形式受け入れ基準を付加したものである。

-----

## 原本: Cupola 要件定義書 v0.4

### 改訂履歴

|バージョン|日付        |概要         |
|-----|----------|-----------|
|v0.1 |—         |初期構想       |
|v0.2 |2026-03-27|要件定義として正式整理|
|v0.3 |2026-03-28|レビュー反映（コメント管理簡素化、agent:cancel 廃止、同時実行数制限削除等）|
|v0.4 |2026-03-29|GitHub 操作の責務分離（2.7）、入出力分離、output-schema 導入、stall 検知、設定項目追加（owner/repo/default_branch/stall_timeout_secs）、execution_log に structured_output 追加|

-----

### 1. 目的

GitHub を唯一の操作面として使い、Issue と PR を起点に Claude Code を外部起動し、cc-sdd ベースの設計・実装・修正対応を自動で進めるローカル常駐エージェントを構築する。

本システムは Issue ごとに独立した worktree と branch を持ち、SQLite によるステートマシンで状態を管理しながら、以下の工程を自動化する。

- Issue の着手判定
- 作業環境初期化
- 設計フェーズ実行（cc-sdd による requirements / design / task 生成）
- 設計 PR 作成
- 設計レビュー監視と修正対応
- 実装フェーズ実行（cc-sdd spec-impl）
- 実装 PR 作成
- 実装レビュー監視と修正対応
- 完了時 cleanup
- 異常終了時の再実行

-----

### 2. 基本方針

#### 2.1 GitHub 完全依存

ユーザー操作は GitHub 上で完結する。cupola は専用 UI を持たない。

#### 2.2 ローカルエージェント常駐

ローカルに常駐するエージェントが GitHub を polling し、Issue / PR の状態を観測してステートマシンを前進させる。

#### 2.3 Issue 単位の独立実行

各 Issue は独立した worktree・branch・SQLite 状態を持つ。複数 Issue の同時処理を許可する。

#### 2.4 フェーズ分離

開発フローは以下の 2 段階に分離する。

1. **設計フェーズ** — cc-sdd による requirements / design / task 生成
1. **実装フェーズ** — cc-sdd spec-impl による実装

各フェーズはそれぞれ独立した PR を持つ。

#### 2.5 冪等性重視

途中失敗、エージェント再起動、polling 重複があっても安全に再開できることを最優先とする。

#### 2.6 cleanup 必須

完了・キャンセル時には branch / worktree / ローカル状態を必ず cleanup する。cleanup 自体も冪等であること。

#### 2.7 GitHub 操作の責務分離

GitHub API に対する操作は、その性質に応じて cupola と Claude Code で責務を分離する。

**cupola が行う操作（GitHub API 操作の全て）:**

- Issue の取得・コメント投稿・close
- PR の作成・merge 状態確認・review thread 監視
- review thread へのコメント返信・resolve

**Claude Code が行う操作（git 操作のみ）:**

- commit / push

この分離により、Claude Code は **git さえ使えれば正常に動作する** 状態を保つ。GitHub API の認証・バージョン互換性・レート制限等の問題は cupola 側に閉じ込められ、Claude Code の実行安定性に影響しない。

Claude Code への入力は cupola がファイルとして worktree に書き出し（`.cupola/inputs/`）、Claude Code からの出力は output-schema で構造化して受け取る。cupola はその結果に基づき、PR 作成・コメント返信・thread resolve 等の GitHub 操作を自ら実行する。状態遷移の判定は、polling による外部観測（PR の merge 状態、review thread の resolve 状態）で行う。

#### 2.8 言語設定

cupola の全体設定として出力言語を指定する。この設定は以下に適用される。

- cc-sdd の spec.json.language（設計成果物の記述言語）
- Issue コメントの言語
- PR body の言語

デフォルトは `ja`（日本語）。設定値は cc-sdd が対応する言語コード（`ja`, `en` 等）に準ずる。

-----

### 3. Git 運用方針

#### 3.1 ブランチ構成

Issue ごとに以下のブランチを作成する。

|ブランチ          |命名規則                     |用途         |
|--------------|-------------------------|-----------|
|default branch|（リポジトリ既定）                |本流。変更しない   |
|issue-main    |`cupola/<issue番号>/main`  |Issue の作業主線|
|design branch |`cupola/<issue番号>/design`|設計成果物の作業線  |

実装専用ブランチは作成しない。実装は issue-main に直接コミットする。

#### 3.2 Worktree 運用

Issue ごとに **1 つの worktree** を作成する。worktree 内でのブランチ遷移は以下の通り。

1. **初期化:** default branch の HEAD を起点に worktree を作成し、`cupola/<issue番号>/main` ブランチを作成してチェックアウト。さらに `cupola/<issue番号>/design` ブランチを作成してチェックアウトする
2. **設計フェーズ:** design ブランチ上で作業
3. **設計 PR merge 後:** issue-main にチェックアウトし、`git pull` で merge 結果を取得
4. **実装フェーズ:** issue-main 上で作業

worktree の再作成は行わない。

#### 3.3 PR 構成

|フェーズ|PR 方向                                              |意味         |
|----|---------------------------------------------------|-----------|
|設計  |`cupola/<issue番号>/design` → `cupola/<issue番号>/main`|設計レビュー     |
|実装  |`cupola/<issue番号>/main` → default branch           |実装レビュー兼本流統合|

#### 3.4 設計根拠

この構成を採用する理由は以下の通り。

- 実装ブランチを追加しないため、ブランチ分岐が最小になる
- 設計承認後、issue-main をそのまま実装の作業線として使える
- 最終的な成果物は issue-main に集約される
- 実装 PR がそのまま本流への統合 PR になる

実装レビュー中に修正コミットが issue-main に積まれるが、これは「作業中ブランチ」として自然な挙動であり、問題にならない。

-----

### 4. cc-sdd 成果物の配置

#### 4.1 ディレクトリ

cc-sdd のディレクトリ構成をそのまま採用し、出力先トップフォルダを `.cupola/` に設定する。

```
.cupola/
├── steering/                          # プロジェクト全体のコンテキスト（product.md, tech.md, structure.md 等）
├── specs/
│   └── <feature-name>/               # Issue（機能）ごとのスペック
│       ├── spec.json                  # メタデータ（フェーズ、承認状態、言語設定）
│       ├── requirements.md            # EARS 形式の要件定義
│       ├── design.md                  # 技術設計書
│       ├── tasks.md                   # 実装タスク一覧
│       └── research.md               # 調査・設計判断の記録
└── settings/                          # cc-sdd のルール・テンプレート
```

cc-sdd の詳細なディレクトリ構成・テンプレート・ルール定義については cc-sdd のドキュメントに準ずる。

#### 4.2 選定理由

- cc-sdd の既存構造をそのまま活用でき、独自の配置変換が不要
- spec.json によるフェーズ管理・承認ゲート制御がそのまま使える
- プロダクト名と一致しており、何のためのディレクトリか自明
- ドットプレフィックスにより、アプリケーションコードと明確に分離される

-----

### 5. ステートマシン

#### 5.1 状態一覧

|状態                             |意味                        |
|-------------------------------|--------------------------|
|`idle`                         |未着手。agent:ready が付与されて着手待ち|
|`initialized`                  |Issue 検知済み、作業環境初期化中       |
|`design_running`               |Claude Code による設計フェーズ実行中  |
|`design_review_waiting`        |設計 PR 作成済み、レビュー待ち         |
|`design_fixing`                |設計 PR の review thread 対応中  |
|`implementation_running`       |Claude Code による実装フェーズ実行中  |
|`implementation_review_waiting`|実装 PR 作成済み、レビュー待ち         |
|`implementation_fixing`        |実装 PR の review thread 対応中  |
|`completed`                    |全工程完了、cleanup 済み          |
|`cancelled`                    |キャンセルまたはリトライ上限到達、cleanup 済み|

#### 5.2 状態遷移表

```
idle
  └─[agent:ready 検知]→ initialized
        └─[worktree/branch 作成完了]→ design_running
              ├─[Claude Code 正常終了 → cupola が設計PR作成]→ design_review_waiting
              │     ├─[unresolved review thread 検知]→ design_fixing
              │     │     ├─[Claude Code 正常終了 → cupola がコメント返信+resolve]→ design_review_waiting
              │     │     ├─[失敗 + リトライ上限未到達]→ design_fixing（再実行）
              │     │     └─[失敗 + リトライ上限到達]→ cancelled
              │     └─[PR merge 検知（※unresolved thread の有無にかかわらず優先）]→ implementation_running
              │           ├─[Claude Code 正常終了 → cupola が実装PR作成]→ implementation_review_waiting
              │           │     ├─[unresolved review thread 検知]→ implementation_fixing
              │           │     │     ├─[Claude Code 正常終了 → cupola がコメント返信+resolve]→ implementation_review_waiting
              │           │     │     ├─[失敗 + リトライ上限未到達]→ implementation_fixing（再実行）
              │           │     │     └─[失敗 + リトライ上限到達]→ cancelled
              │           │     └─[PR merge 検知（※同上）]→ completed
              │           └─[失敗 + リトライ上限未到達]→ implementation_running（再実行）
              │           └─[失敗 + リトライ上限到達]→ cancelled
              └─[失敗 + リトライ上限未到達]→ design_running（再実行）
              └─[失敗 + リトライ上限到達]→ cancelled

（terminal 以外の全状態共通。completed / cancelled は対象外）
  └─[Issue close 検知]→ cancelled
```

#### 5.3 状態遷移の補足ルール

- `design_review_waiting` に遷移した直後に unresolved review thread が存在する場合、即座に `design_fixing` に遷移してよい
- `implementation_review_waiting` も同様
- `cancelled` 状態からの復帰は、Issue を reopen し `agent:ready` を再付与する手順とする（リトライ上限到達による cancelled の場合、cupola が Issue を close 済みなので reopen が必要）

-----

### 6. 各状態の詳細仕様

#### 6.1 idle → initialized

**トリガー:** polling により `agent:ready` ラベルが付いた open な Issue を検知し、かつ SQLite にレコードが存在しないか、レコードが terminal 状態（completed / cancelled）である場合

**実行内容:**

1. SQLite に Issue レコードを作成（既存レコードがある場合は上書き。14.2 参照）
1. default branch の HEAD を起点に worktree を作成
1. `cupola/<issue番号>/main` ブランチを作成してチェックアウト、リモートに push
1. `cupola/<issue番号>/design` ブランチを作成してチェックアウト、リモートに push（設計フェーズの作業ブランチ）
1. Issue にコメント投稿: 「設計を開始します」

**遷移条件:** 上記すべて完了 → `design_running`

**失敗時の復旧:** 初期化処理（手順 2〜5）が途中で失敗した場合、`initialized` 状態で停留する。次回の polling サイクルで `initialized` 状態の Issue を検出し、初期化処理を再試行する（冪等性により、既に完了したステップはスキップされる）。初期化の再試行にも `retry_count` / `max_retries` を適用し、上限到達時は `cancelled` に遷移する。

#### 6.2 design_running

**入力準備（cupola、プロセス起動のたびに毎回実行）:**

1. GitHub API で Issue 本文を取得
1. worktree 内に `.cupola/inputs/issue.md` として書き出し

**Claude Code の実行内容:**

1. `.cupola/inputs/issue.md` から Issue 本文を読み取り
1. `cc-sdd init`
1. `cc-sdd requirements`
1. `cc-sdd design`
1. `cc-sdd task`
1. 設計成果物を `.cupola/` に配置
1. commit / push
1. output-schema で PR の title / body を出力

**後処理（cupola）:**

Claude Code 正常終了後、output-schema から PR 情報を取得し、cupola が `cupola/<issue番号>/design` → `cupola/<issue番号>/main` の設計 PR を作成する。

**遷移条件:**

- Claude Code 正常終了 → cupola が設計 PR を作成 → `design_review_waiting`
- Claude Code 失敗 + リトライ上限未到達 → `design_running`（再実行）
- Claude Code 失敗 + リトライ上限到達 → `cancelled`

#### 6.3 design_review_waiting

**実行内容:**

polling により設計 PR を監視する。

**監視対象（最小限）:**

- PR に unresolved review thread があるか（GitHub GraphQL API で判定）
- PR が merge されたか

**遷移条件（merge を優先判定）:**

- PR merge 検知 → worktree を issue-main にチェックアウト → `git pull` で merge 結果を取得 → Issue にコメント投稿「実装を開始します」→ `implementation_running`（unresolved thread が残っていても次フェーズに進む）
- unresolved review thread 1 件以上（かつ未 merge） → `design_fixing`

#### 6.4 design_fixing

**入力準備（cupola、プロセス起動のたびに毎回実行。リトライ時も最新データで更新）:**

1. GraphQL API で設計 PR の unresolved review thread を取得
1. worktree 内に `.cupola/inputs/review_threads.json` として書き出し

**Claude Code の実行内容:**

1. `.cupola/inputs/review_threads.json` から指摘内容を読み取り
1. 設計成果物の修正
1. commit / push
1. output-schema で各 thread への返信内容と resolve 判定を出力

**後処理（cupola）:**

Claude Code 正常終了後、output-schema から各 thread の返信を取得し、cupola が PR へのコメント返信と review thread の resolve を実行する。

**遷移条件:**

- Claude Code 正常終了 → cupola がコメント返信 + resolve → `design_review_waiting`
- Claude Code 正常終了だが output-schema パース失敗 → 後処理をスキップし `design_review_waiting` に遷移（resolve 漏れは次回 polling で再検知される）
- Claude Code 失敗 + リトライ上限未到達 → `design_fixing`（再実行）
- Claude Code 失敗 + リトライ上限到達 → `cancelled`

#### 6.5 implementation_running

**前提:** 設計 PR が merge 済みで、issue-main に設計成果物が統合されている

**入力準備（cupola）:**

追加の入力準備は不要。設計成果物（`.cupola/specs/`）は worktree 内に存在している。

**Claude Code の実行内容:**

1. `cc-sdd spec-impl` を実行
1. issue-main に直接コミット / push
1. output-schema で PR の title / body を出力

**後処理（cupola）:**

Claude Code 正常終了後、output-schema から PR 情報を取得し、cupola が `cupola/<issue番号>/main` → default branch の実装 PR を作成する。

**遷移条件:**

- Claude Code 正常終了 → cupola が実装 PR を作成 → `implementation_review_waiting`
- Claude Code 失敗 + リトライ上限未到達 → `implementation_running`（再実行）
- Claude Code 失敗 + リトライ上限到達 → `cancelled`

#### 6.6 implementation_review_waiting

**実行内容:**

polling により実装 PR を監視する。監視内容は設計レビュー待ちと同一（unresolved review thread の有無 + merge 状態）。

**遷移条件（merge を優先判定）:**

- PR merge 検知 → `completed`（unresolved thread が残っていても完了とする）
- unresolved review thread 1 件以上（かつ未 merge） → `implementation_fixing`

#### 6.7 implementation_fixing

設計修正（6.4）と同一の手順。入力準備（unresolved thread → `.cupola/inputs/review_threads.json`）、Claude Code の実行（修正 + commit/push + output-schema）、後処理（コメント返信 + resolve）の全てが同一構造。

**遷移条件:**

- Claude Code 正常終了 → cupola がコメント返信 + resolve → `implementation_review_waiting`
- Claude Code 正常終了だが output-schema パース失敗 → 後処理をスキップし `implementation_review_waiting` に遷移（resolve 漏れは次回 polling で再検知される）
- Claude Code 失敗 + リトライ上限未到達 → `implementation_fixing`（再実行）
- Claude Code 失敗 + リトライ上限到達 → `cancelled`

#### 6.8 completed

**実行内容:**

1. SQLite レコードを `completed` に更新（※ Issue close より先に行うこと。7.2 の検知方法参照）
1. Issue にコメント投稿:「全工程が完了しました」
1. Issue が open なら close
1. worktree 削除
1. `cupola/<issue番号>/main` ブランチをローカル・リモートともに削除
1. `cupola/<issue番号>/design` ブランチをローカル・リモートともに削除（残存している場合）

#### 6.9 cancelled

**実行内容:** 11.2 キャンセル時の処理 を参照。

`cancelled` への遷移は 2 つのトリガーがある。Issue コメントの内容は遷移元に応じて分岐する。

|遷移元|Issue コメント内容|
|---|---|
|Issue close 検知|cleanup を実行した旨|
|リトライ上限到達（RetryExhausted）|失敗理由、リトライ回数、cleanup を実行した旨|

**復帰手順:** Issue を reopen し、`agent:ready` を再付与する。再開時は `idle` から最初から実行する。

-----

### 7. Issue 運用

#### 7.1 着手条件

Issue に `agent:ready` ラベルが付与されていること。これが唯一の開始トリガーである。

#### 7.2 停止条件

Issue が close された場合、キャンセルとみなす。現在の状態にかかわらず cleanup を実行し `cancelled` に遷移する。

**検知方法:** SQLite 上で非 terminal 状態（completed / cancelled 以外）のレコードが存在する Issue に対して、個別に GitHub API で open/closed 状態を確認する。Issue が closed であればキャンセル処理を行う。

**注意:** `agent:ready` ラベルの有無では close を判定しない。ラベルが手動で除去されても Issue が open であればキャンセルしない（処理は継続する）。cupola 自身が Issue を close する場合（completed / cancelled 遷移時）は、先に SQLite の state を terminal に更新してから close するため、二重キャンセルは発生しない。

#### 7.3 ラベル運用

cupola が使用するラベルは `agent:ready` の 1 つのみ。**人間が手動で付与**するものであり、システム側は付与も削除もしない。

|ラベル           |付与者|意味                   |
|--------------|---|---------------------|
|`agent:ready` |人間 |この Issue の自動処理を開始してよい|

停止は Issue の close で行う。専用の停止ラベルは設けない。

システム側の状態表現は SQLite に閉じ込め、GitHub ラベルには一切依存しない。進捗は Issue コメントで通知する。

#### 7.4 Issue コメントによる通知

cupola は以下のタイミングで Issue にコメントを投稿する。

|タイミング           |コメント内容                      |
|-----------------|----------------------------|
|設計開始時           |設計フェーズを開始する旨                |
|実装開始時           |実装フェーズを開始する旨                |
|完了時             |全工程が完了した旨                   |
|キャンセル実行時（Issue close）|cleanup を実行した旨              |
|キャンセル実行時（リトライ上限）  |失敗理由、リトライ回数、cleanup を実行した旨|

PR 作成・merge 等の通知は GitHub 自体の通知機構に委ねるため、cupola からは投稿しない。

#### 7.5 Issue テンプレート

以下のテンプレートを `.github/ISSUE_TEMPLATE/cupola-task.yml` として提供する。

```yaml
name: cupola タスク
description: cupola による自動設計・実装を依頼する
title: "[cupola] "
labels: []
body:
  - type: markdown
    attributes:
      value: |
        このテンプレートは cupola に自動設計・実装を依頼するためのものです。
        cc-sdd の requirements フェーズの入力として使用されます。
        できるだけ具体的に記述してください。

  - type: textarea
    id: summary
    attributes:
      label: 概要
      description: |
        何を作るのか、1〜3 文で簡潔に説明してください。
      placeholder: "例: ユーザープロフィール画面を新規作成する"
    validations:
      required: true

  - type: textarea
    id: background
    attributes:
      label: 背景・動機
      description: |
        なぜこの変更が必要なのか。現状の問題点や、この機能が解決する課題を記述してください。
      placeholder: |
        例:
        現在、ユーザー情報を確認する手段がない。
        設定画面から間接的に見ることはできるが、導線が深く使いにくい。
    validations:
      required: true

  - type: textarea
    id: requirements
    attributes:
      label: 要求事項
      description: |
        実現すべきことを箇条書きで列挙してください。
        機能要件・非機能要件の区別は不要です。思いつく限り書いてください。
      placeholder: |
        例:
        - ユーザー名、メールアドレス、アバターを表示する
        - アバターはクリックで変更できる
        - レスポンシブ対応必須
        - 表示速度は 200ms 以内
    validations:
      required: true

  - type: textarea
    id: scope
    attributes:
      label: スコープ（やること / やらないこと）
      description: |
        この Issue の範囲を明確にするために、含めるものと含めないものを記述してください。
      placeholder: |
        例:
        やること:
        - プロフィール画面の表示
        - アバター変更機能

        やらないこと:
        - パスワード変更（別 Issue で対応）
        - メール認証フロー
    validations:
      required: false

  - type: textarea
    id: technical_context
    attributes:
      label: 技術的コンテキスト
      description: |
        関連するファイル、モジュール、API、既存の設計判断など、
        設計・実装の参考になる技術情報があれば記述してください。
      placeholder: |
        例:
        - 関連ファイル: src/pages/settings.tsx
        - 既存の User モデル: src/models/user.ts
        - API: GET /api/users/:id は実装済み
        - デザインシステム: shadcn/ui を使用
    validations:
      required: false

  - type: textarea
    id: acceptance_criteria
    attributes:
      label: 受け入れ条件
      description: |
        この Issue が「完了」と判断できる条件を記述してください。
        テスト観点やレビュー時の確認項目として使われます。
      placeholder: |
        例:
        - /profile にアクセスするとプロフィール画面が表示される
        - アバターをクリックするとファイル選択ダイアログが開く
        - 選択した画像がアバターとして反映される
        - モバイル幅（375px）で崩れない
    validations:
      required: false

  - type: textarea
    id: notes
    attributes:
      label: 補足事項
      description: |
        その他、設計・実装に影響しうる情報があれば自由に記述してください。
        参考リンク、類似実装、懸念事項など。
      placeholder: |
        例:
        - 参考: https://example.com/design-spec
        - 将来的にチーム機能を追加予定なので、拡張性を意識してほしい
    validations:
      required: false
```

-----

### 8. レビューコメント管理

#### 8.1 追跡対象

PR 上の **review thread**（GitHub GraphQL の `PullRequestReviewThread`）のみを追跡する。

PR 下部の一般コメント（issue comment）は追跡対象外とする。レビューは review comment で行う運用とする。

#### 8.2 判定方法

GitHub GraphQL API を使用し、PR に `isResolved == false` の review thread が存在するかを polling で確認する。cupola 側に独自のコメント管理テーブルは持たない。GitHub の resolve 状態を唯一の信頼源とする。

#### 8.3 コメント対応フロー

1. cupola が `review_waiting` 状態で unresolved review thread を検知
1. cupola が GraphQL API で unresolved thread の全内容を取得
1. cupola が worktree 内に `.cupola/inputs/review_threads.json` として書き出し
1. cupola が `fixing` に遷移し、Claude Code を起動
1. Claude Code が `.cupola/inputs/review_threads.json` を読み取り、修正・commit / push
1. Claude Code が output-schema で各 thread への返信内容と resolve 判定を出力して正常終了
1. cupola が output-schema に基づき、PR へのコメント返信と review thread の resolve を実行
1. `review_waiting` に遷移
1. 次回 polling で再び unresolved thread があれば `fixing` に再遷移する

#### 8.4 GitHub API 操作の責務分担

全ての GitHub API 操作は cupola が行う。Claude Code は GitHub API を直接呼び出さない。

|操作|担当|タイミング|
|---|---|---|
|unresolved thread の取得|cupola|fixing 遷移時（入力準備）|
|コメント返信の投稿|cupola|Claude Code 正常終了後（後処理）|
|thread の resolve|cupola|Claude Code 正常終了後（後処理）|
|PR の作成|cupola|running 状態の Claude Code 正常終了後（後処理）|

Claude Code の依存は git（commit / push）のみ。gh CLI は不要。

-----

### 9. Polling

#### 9.1 間隔

polling 間隔は **60 秒** を基本とする。

状態に応じて間隔を変えても良いが、初期実装では固定値で十分。

#### 9.2 監視対象

polling で観測する情報は以下に限定する。

|観測対象                                       |判定に使う状態遷移                     |
|---------------------------------------------|------------------------------|
|Issue の `agent:ready` ラベル有無                  |idle → initialized            |
|Issue が open な Issue の検索結果に含まれるか           |非 terminal 状態 → cancelled（不在時）|
|PR の merge 状態                                |review_waiting → 次フェーズ        |
|PR の unresolved review thread 有無（GraphQL API）|review_waiting → fixing       |

※ PR の存在確認は不要。PR は cupola が作成するため、作成時点で PR 番号を DB に記録している。

#### 9.3 監視しないもの

- PR の close（merge でない close は無視。レビュアーが誤って PR を close した場合は、人間が GitHub 上で reopen するか、Issue を close → reopen + agent:ready で最初からやり直す）
- 細かいイベント履歴の完全再現
- コメントスレッドの高度な整合性
- GitHub webhook（polling のみで運用）

-----

### 10. 再実行（リトライ）

#### 10.1 対象状態

以下の状態がリトライ対象。

- `initialized`（初期化処理の再試行）
- `design_running`
- `design_fixing`
- `implementation_running`
- `implementation_fixing`

#### 10.2 再実行条件

以下の **いずれか** を満たす場合に再実行する。

- 該当状態なのに実行中プロセスがいない
- 前回実行が失敗で終了した

成果物ファイルの存在検査は行わない。PR 作成は cupola が行うため、Claude Code の正常終了をもって完了と判断する。

#### 10.3 判定方式

以下の 2 軸で判定する。

1. **起動中かどうか** — プロセスが生きているなら何もしない
1. **完了条件を満たしているか** — 満たしているなら次状態へ遷移

どちらでもなければ再実行する。

#### 10.4 リトライ上限

|項目      |値                              |
|--------|-------------------------------|
|最大リトライ回数|**3 回**（初回実行を含めず）              |
|上限到達時の動作|`cancelled` 状態に遷移（Issue close + cleanup）|
|通知      |Issue にコメント投稿（失敗理由 + リトライ回数を記載）|

#### 10.5 リトライカウントの管理

- retry_count は SQLite の **1 カラム** で管理する
- 状態遷移のたびに **リセット（0 に戻す）** される（例: `design_running` → `design_review_waiting` に進んだらリセット）
- これにより各状態は独立して最大 max_retries 回のリトライ機会を持つ（`design_running` で 3 回リトライ後に `design_review_waiting` に進めば、次に `design_fixing` に入った時点で retry_count は 0 から再スタート）
- キャンセル後の再開時もリセットされる

#### 10.6 stall 検知（タイムアウト）

Claude Code プロセスが `stall_timeout_secs`（デフォルト: 1800 秒 = 30 分）を超えて実行中の場合、stall と判定する。

**stall 時の処理:**

1. プロセスを kill する
1. ProcessFailed として扱い、retry_count をインクリメントする
1. 状態は遷移しない（同状態にとどまる）
1. retry_count が max_retries 未満であれば、次の polling サイクルで再起動する
1. retry_count が max_retries に到達した場合は `cancelled` に遷移する（Issue close + cleanup）

-----

### 11. キャンセル

#### 11.1 トリガー

以下のいずれかで `cancelled` に遷移する。

- Issue が close されたことを検知した場合
- リトライ上限に到達した場合（RetryExhausted）

#### 11.2 キャンセル時の処理

cancelled への遷移は、completed と同様に「状態遷移（SQLite 更新）→ 副作用実行」のパターンに従う。SQLite を先に更新することで、副作用実行中に次の polling サイクルが走っても二重処理が発生しない（7.2 の検知方法参照）。

1. SQLite レコードを `cancelled` に更新（**最初に行うこと**）
1. 実行中の Claude Code プロセスを停止
1. Issue が open なら close（リトライ上限到達の場合。人間が close した場合は既に close 済み）
1. worktree 削除
1. `cupola/<issue番号>/main` ブランチをローカル・リモートともに削除
1. `cupola/<issue番号>/design` ブランチをローカル・リモートともに削除
1. 未 merge の関連 PR を close（任意）
1. Issue にコメント投稿（遷移元に応じて内容を分岐。6.9 参照）

#### 11.3 キャンセル後の再開

キャンセル後に再開する場合は、**必ず最初から**実行する。途中復帰は行わない。

再開手順: Issue を reopen → `agent:ready` を付与 → `idle` から再スタート

-----

### 12. Cleanup

#### 12.1 対象

|対象                      |説明                       |
|------------------------|-------------------------|
|worktree                |Issue 対応の git worktree   |
|design branch（ローカル+リモート）|`cupola/<issue番号>/design`|
|issue-main branch（ローカル+リモート）|`cupola/<issue番号>/main`  |
|実行プロセス管理情報              |PID 等のプロセストラッキング情報       |
|一時ファイル                  |作業ディレクトリ内の中間ファイル         |

#### 12.2 方針

- cleanup は **冪等** であること。途中まで削除済みの状態でも何度でも安全に再実行できること
- 対象リソースが存在しない場合はスキップし、エラーにしない
- cleanup の失敗はログに記録するが、状態遷移をブロックしない

-----

### 13. ローカルファイル配置と .gitignore

#### 13.1 ローカルファイル配置

cupola が使用するローカルファイルは、リポジトリルート直下の `.cupola/` 内に集約する。

```
.cupola/
├── steering/              # プロジェクトコンテキスト（git 追跡対象）
├── specs/                 # cc-sdd スペック（git 追跡対象）
│   └── <feature-name>/
│       ├── spec.json
│       ├── requirements.md
│       ├── design.md
│       ├── tasks.md
│       └── research.md
├── settings/              # cc-sdd ルール・テンプレート（git 追跡対象）
├── cupola.toml            # cupola 全体設定（git 追跡対象）
├── inputs/                # Claude Code への入力ファイル（ローカルのみ、worktree 内に書き出し）
├── worktrees/             # Issue ごとの git worktree（ローカルのみ）
├── cupola.db              # SQLite データベース（ローカルのみ）
├── cupola.db-wal          # SQLite WAL ファイル（ローカルのみ）
├── cupola.db-shm          # SQLite 共有メモリ（ローカルのみ）
└── logs/                  # ログファイル（ローカルのみ、日付別: cupola-YYYY-MM-DD.log）
```

#### 13.2 git 追跡の方針

|対象                                            |追跡 |理由             |
|----------------------------------------------|---|---------------|
|cc-sdd 成果物（steering / specs / settings）       |する |設計ドキュメントはレビュー対象|
|cupola.toml                                   |する |リポジトリ共有設定      |
|inputs/                                       |しない|Claude Code への一時入力|
|worktrees/                                    |しない|ローカル作業領域       |
|cupola.db / WAL / SHM                         |しない|ローカル状態管理       |
|logs/                                         |しない|ローカルログ         |

#### 13.3 .gitignore

以下のエントリをリポジトリの `.gitignore` に追加する。

```gitignore
# Cupola - local runtime files
.cupola/worktrees/
.cupola/cupola.db
.cupola/cupola.db-wal
.cupola/cupola.db-shm
.cupola/logs/
.cupola/inputs/
```

-----

### 14. SQLite 最小テーブル構成

#### 14.1 issues テーブル

Issue ごとの状態管理。

|カラム                |型                  |説明                             |
|-------------------|-------------------|-------------------------------|
|id                 |INTEGER PRIMARY KEY|内部 ID                          |
|github_issue_number|INTEGER UNIQUE     |GitHub Issue 番号                |
|state              |TEXT NOT NULL      |現在の状態（5.1 の状態名）                |
|design_pr_number   |INTEGER            |設計 PR 番号（nullable）             |
|impl_pr_number     |INTEGER            |実装 PR 番号（nullable）             |
|worktree_path      |TEXT               |worktree のパス                   |
|retry_count        |INTEGER DEFAULT 0  |現フェーズのリトライ回数                   |
|current_pid        |INTEGER            |実行中 Claude Code の PID（nullable）|
|created_at         |TEXT               |レコード作成日時                       |
|updated_at         |TEXT               |最終更新日時                         |
|error_message      |TEXT               |直近のエラーメッセージ（nullable）          |

#### 14.2 再開時のレコード処理

cancelled 状態の Issue が reopen + agent:ready で再開された場合、既存の issues レコードを **上書き（UPDATE）** する。

|カラム|再開時の処理|
|---|---|
|id|維持|
|github_issue_number|維持（同一 Issue）|
|state|`idle` にリセット|
|design_pr_number|NULL にリセット|
|impl_pr_number|NULL にリセット|
|worktree_path|NULL にリセット|
|retry_count|0 にリセット|
|current_pid|NULL にリセット|
|created_at|維持|
|updated_at|現在日時に更新|
|error_message|NULL にリセット|

execution_log は削除しない。過去の実行履歴はデバッグ用に蓄積し続ける。

#### 14.3 execution_log テーブル

Claude Code 実行の履歴。リトライ判定とデバッグに使用。

|カラム              |型                            |説明                             |
|-----------------|-----------------------------|---------------------------------|
|id               |INTEGER PRIMARY KEY          |内部 ID                          |
|issue_id         |INTEGER REFERENCES issues(id)|親 Issue                        |
|state            |TEXT                         |実行時の状態                       |
|started_at       |TEXT                         |実行開始日時                       |
|finished_at      |TEXT                         |実行終了日時（nullable）             |
|exit_code        |INTEGER                      |終了コード（nullable）              |
|structured_output|TEXT                         |output-schema の結果 JSON（nullable）|
|error_message    |TEXT                         |エラーメッセージ（nullable）           |

-----

### 15. 設定ファイル

#### 15.1 配置

`.cupola/cupola.toml` に cupola の全体設定を記述する。git 追跡対象とし、リポジトリごとの設定を共有する。

#### 15.2 設定項目

|項目|型|デフォルト値|説明|
|---|---|---|---|
|`owner`|string|（必須）|GitHub リポジトリオーナー|
|`repo`|string|（必須）|GitHub リポジトリ名|
|`default_branch`|string|（必須）|リポジトリのデフォルトブランチ名（`main`, `master` 等）|
|`language`|string|`"ja"`|出力言語（Issue コメント、PR body、cc-sdd 成果物）|
|`polling_interval_secs`|integer|`60`|polling 間隔（秒）|
|`max_retries`|integer|`3`|状態ごとのリトライ上限|
|`stall_timeout_secs`|integer|`1800`|Claude Code プロセスの stall 判定タイムアウト（秒）。超過時は kill して ProcessFailed 扱い|
|`log.level`|string|`"info"`|ログレベル（`trace`, `debug`, `info`, `warn`, `error`）|
|`log.dir`|string|（なし）|ログ出力ディレクトリ。未指定時は stderr のみに出力。日付ごとにファイルを分割する（`cupola-YYYY-MM-DD.log` 形式）|

#### 15.3 CLI フラグによる上書き

設定ファイルの値は CLI フラグで上書きできる。優先順位: CLI フラグ > 設定ファイル > デフォルト値。

```
cupola run --polling-interval-secs 30 --log-level debug
```

#### 15.4 設定ファイル例

```toml
owner = "kyuki3rain"
repo = "my-project"
default_branch = "main"
language = "ja"
polling_interval_secs = 60
max_retries = 3
stall_timeout_secs = 1800

[log]
level = "info"
dir = ".cupola/logs"
```

-----

### 16. 推奨運用フロー（まとめ）

```
1. 人間が Issue を作成（テンプレート使用）
2. 人間が agent:ready ラベルを付与
3. cupola が検知 → 初期化 → Issue にコメント「設計を開始します」
4. Claude Code が cc-sdd 設計一式を実行
5. cupola が設計 PR 作成（design → issue-main）
6. 人間がレビュー → コメントがあれば Claude Code が修正、cupola がコメント返信+resolve
7. 人間が設計 PR を merge
8. cupola が検知 → Issue にコメント「実装を開始します」
9. Claude Code が cc-sdd spec-impl を実行
10. cupola が実装 PR 作成（issue-main → default branch）
11. 人間がレビュー → コメントがあれば Claude Code が修正、cupola がコメント返信+resolve
12. 人間が実装 PR を merge
13. cupola が検知 → Issue close + cleanup
```

停止したい場合:

- Issue close → cleanup → cancelled
- 再開: reopen + agent:ready → 最初から

-----

### 付録 A: 設計判断の記録

|判断項目      |採用案                          |理由                        |
|----------|-----------------------------|--------------------------|
|実装ブランチ    |作らない（issue-main 直接）          |ブランチ分岐最小化                 |
|成果物配置     |`.cupola/`                   |プロダクト名一致、ドットプレフィックスでコードと分離|
|着手トリガー    |ラベル `agent:ready`            |polling しやすく意図が明確         |
|停止トリガー    |Issue close のみ               |シンプルに統一                    |
|状態管理      |SQLite（GitHub ラベル不使用）        |システム内部に閉じ込めて単純化           |
|進捗通知      |Issue コメント（最小限）              |PR 通知は GitHub に委任         |
|キャンセル後の再開 |最初から                         |途中復帰の複雑性を排除               |
|polling 間隔|60 秒固定                       |初期はシンプルに                  |
|リトライ上限    |3 回                          |無限ループ回避                   |
|コメント管理    |GitHub review thread の resolve 状態を信頼源|cupola 独自管理テーブル不要、GraphQL で統一|
|追跡対象コメント  |review comment のみ（issue comment は対象外）|運用ルールで review comment を使ってもらう|
|merge vs コメント|merge 検知を優先（unresolved thread が残っていても次へ進む）|レビュアーが merge した = 承認の意思表示|
|再開時のレコード |既存レコードを上書き（execution_log は保持）|UNIQUE 制約との整合性、ログは蓄積|
|fixing リトライ|running と同じパターン（失敗時は同状態にとどまりリトライ）|状態遷移を統一しシンプルに|
|リトライカウント  |状態ごとに独立管理、次状態への遷移でリセット|各状態で十分なリトライ機会を確保|
|Issue close 検知|open Issue 検索で不在なら close とみなす|cupola 自身の close と人間の close を区別不要|
|成果物ディレクトリ構成|cc-sdd の構造をそのまま採用（`.cupola/specs/{feature}/`）|cc-sdd の spec.json フェーズ管理をそのまま活用|
|設定ファイル     |`.cupola/cupola.toml`（git 追跡）  |リポジトリごとの設定を共有、CLI フラグで上書き可|
|言語設定       |全体設定として指定（デフォルト `ja`）    |Issue コメント、PR body、cc-sdd 成果物に統一適用|
|GitHub API 操作|全て cupola が実行（Claude Code は git のみ）|Claude Code の依存を最小化。gh CLI バージョンリスクを排除。テスタビリティ向上|
|入出力分離     |入力はファイル書き出し、出力は output-schema|プロンプトサイズを抑制。構造化された結果でリトライ・エラーハンドリングが容易|
|Issue テンプレート|`agent:ready` ラベルを自動付与しない|着手の意思決定は人間が明示的にラベル付与で行う。誤って自動処理が始まることを防止|

-----

## Requirements（EARS 形式受け入れ基準）

### Requirement 1: Issue 検知と初期化

**Objective:** 開発者として、`agent:ready` ラベル付き Issue を自動検知し作業環境を初期化したい。手動の環境構築なしに設計フェーズを開始できるようにするため。

#### Acceptance Criteria

1. When `agent:ready` ラベル付きの open な Issue が polling で検知される, Cupola shall SQLite に Issue レコードを作成し `initialized` 状態に遷移する
2. When Issue の初期化が開始される, Cupola shall default branch の HEAD を起点に worktree を作成し、`cupola/<issue番号>/main` および `cupola/<issue番号>/design` ブランチを作成・push する
3. When 初期化が正常に完了する, Cupola shall Issue にコメント「設計を開始します」を投稿し `design_running` に遷移する
4. If 初期化処理の途中で失敗する, then Cupola shall `initialized` 状態で停留し、次回の polling サイクルで初期化処理を再試行する
5. If SQLite に terminal 状態（completed / cancelled）のレコードが存在する Issue が再検知される, then Cupola shall 既存レコードを上書きリセットし `idle` から再スタートする

### Requirement 2: 設計フェーズ実行

**Objective:** 開発者として、Issue の内容に基づき cc-sdd による設計一式（requirements / design / tasks）を自動生成したい。設計ドキュメントの手動作成を不要にするため。

#### Acceptance Criteria

1. When `design_running` 状態で Claude Code プロセスを起動する, Cupola shall GitHub API から Issue 本文を取得し `.cupola/inputs/issue.md` として worktree 内に書き出す
2. When Claude Code が正常終了する, Cupola shall output-schema から PR 情報を取得し `cupola/<issue番号>/design` → `cupola/<issue番号>/main` の設計 PR を作成する
3. When 設計 PR が正常に作成される, Cupola shall `design_review_waiting` に遷移する
4. If Claude Code が異常終了し retry_count < max_retries, then Cupola shall retry_count をインクリメントし `design_running` 状態にとどまり次サイクルで再起動する
5. If Claude Code が異常終了し retry_count >= max_retries, then Cupola shall `cancelled` に遷移する

### Requirement 3: 設計レビュー監視

**Objective:** 開発者として、設計 PR のレビュー状態を自動監視し、merge または未解決コメント検知で次のアクションに進みたい。レビュー完了の手動確認を不要にするため。

#### Acceptance Criteria

1. While `design_review_waiting` 状態, Cupola shall polling により設計 PR の merge 状態と unresolved review thread の有無を監視する
2. When 設計 PR の merge が検知される, Cupola shall worktree を issue-main にチェックアウトし `git pull` を実行して `implementation_running` に遷移する（unresolved thread の有無にかかわらず merge を優先）
3. When 設計 PR に unresolved review thread が検知される（かつ未 merge）, Cupola shall `design_fixing` に遷移する

### Requirement 4: レビュー指摘対応

**Objective:** 開発者として、PR のレビュー指摘を自動で修正し、返信・resolve まで一貫して処理したい。修正→返信→resolve の手動サイクルを自動化するため。

#### Acceptance Criteria

1. When `design_fixing` または `implementation_fixing` 状態で Claude Code を起動する, Cupola shall GraphQL API で unresolved review thread を取得し `.cupola/inputs/review_threads.json` として worktree 内に書き出す
2. When Claude Code が正常終了する, Cupola shall output-schema から各 thread への返信内容と resolve 判定を取得し、PR へのコメント返信と resolve を実行する
3. When 後処理が完了する, Cupola shall `review_waiting` 状態に遷移する
4. If output-schema のパースに失敗する, then Cupola shall 後処理をスキップし `review_waiting` に遷移する（resolve 漏れは次回 polling で再検知される）
5. If Claude Code が異常終了し retry_count < max_retries, then Cupola shall retry_count をインクリメントし同状態にとどまり次サイクルで再起動する

### Requirement 5: 実装フェーズ実行

**Objective:** 開発者として、設計承認後に cc-sdd spec-impl による実装を自動実行したい。設計から実装への手動引き継ぎを不要にするため。

#### Acceptance Criteria

1. When `implementation_running` 状態で Claude Code プロセスを起動する, Cupola shall worktree 内の既存設計成果物（`.cupola/specs/`）を入力として cc-sdd spec-impl を実行する
2. When Claude Code が正常終了する, Cupola shall output-schema から PR 情報を取得し `cupola/<issue番号>/main` → default branch の実装 PR を作成する
3. When 実装 PR が正常に作成される, Cupola shall `implementation_review_waiting` に遷移する
4. If Claude Code が異常終了し retry_count < max_retries, then Cupola shall retry_count をインクリメントし `implementation_running` 状態にとどまり次サイクルで再起動する
5. If Claude Code が異常終了し retry_count >= max_retries, then Cupola shall `cancelled` に遷移する

### Requirement 6: 実装レビュー監視と完了

**Objective:** 開発者として、実装 PR の merge を自動検知し、完了処理（cleanup 含む）まで一貫して行いたい。全工程の自動完了を実現するため。

#### Acceptance Criteria

1. While `implementation_review_waiting` 状態, Cupola shall polling により実装 PR の merge 状態と unresolved review thread の有無を監視する
2. When 実装 PR の merge が検知される, Cupola shall `completed` に遷移する（unresolved thread が残っていても完了とする）
3. When 実装 PR に unresolved review thread が検知される（かつ未 merge）, Cupola shall `implementation_fixing` に遷移する
4. When `completed` に遷移する, Cupola shall SQLite を先に更新し、Issue にコメント投稿、Issue close、worktree 削除、ブランチ削除を実行する

### Requirement 7: キャンセルと Cleanup

**Objective:** 開発者として、Issue close またはリトライ上限到達時に安全にリソースを解放したい。不要な worktree やブランチが残存しないようにするため。

#### Acceptance Criteria

1. When 非 terminal 状態の Issue が close されたことを polling で検知する, Cupola shall `cancelled` に遷移し cleanup を実行する
2. When リトライ上限に到達する, Cupola shall `cancelled` に遷移し cleanup を実行する
3. When `cancelled` に遷移する, Cupola shall SQLite を先に更新し、実行中プロセスの停止、worktree 削除、ブランチ削除（ローカル + リモート）を実行する
4. The Cupola shall cleanup を冪等に実行する（対象リソースが存在しない場合はスキップし、途中の失敗は残りの操作をブロックしない）
5. When キャンセルが Issue close 由来, Cupola shall cleanup 実行した旨を Issue にコメント投稿する
6. When キャンセルがリトライ上限到達由来, Cupola shall 失敗理由、リトライ回数、cleanup 実行した旨を Issue にコメント投稿し、Issue を close する

### Requirement 8: Stall 検知

**Objective:** 開発者として、Claude Code プロセスがハングした場合に自動検知・停止したい。無限に待ち続けることを防止するため。

#### Acceptance Criteria

1. While Claude Code プロセスが実行中, Cupola shall プロセスの起動時刻から `stall_timeout_secs` を超過していないかを各 polling サイクルで確認する
2. When プロセスが `stall_timeout_secs` を超過する, Cupola shall プロセスを kill し ProcessFailed として扱う
3. When stall によるプロセス kill 後, Cupola shall retry_count をインクリメントし、上限未到達なら次サイクルで再起動、上限到達なら `cancelled` に遷移する

### Requirement 9: 設定管理と CLI

**Objective:** 開発者として、TOML 設定ファイルと CLI フラグで Cupola の動作を制御したい。リポジトリごとの設定共有と実行時の柔軟な上書きを両立するため。

#### Acceptance Criteria

1. The Cupola shall `.cupola/cupola.toml` から `owner`, `repo`, `default_branch`（必須）およびその他オプション設定を読み込む
2. When CLI フラグが指定される, Cupola shall 設定ファイルの値を上書きする（優先順位: CLI > cupola.toml > デフォルト値）
3. The Cupola shall `cupola run` コマンドで polling ループを開始する
4. The Cupola shall `cupola init` コマンドで SQLite スキーマを冪等に初期化する
5. The Cupola shall `cupola status` コマンドで全 Issue の状態を一覧表示する
6. If 設定ファイルの読み込みに失敗する, then Cupola shall 起動を中断しエラーメッセージを表示する

### Requirement 10: 冪等性と再起動復旧

**Objective:** 開発者として、Cupola の再起動やクラッシュ後に安全に処理を再開したい。途中状態からの復旧を確実にするため。

#### Acceptance Criteria

1. When Cupola が再起動される, Cupola shall SQLite から全 Issue レコードを読み込み、非 terminal 状態の Issue の `current_pid` を NULL にクリアする
2. When 再起動後に `needs_process()` な状態の Issue が検出される, Cupola shall 次の polling サイクルでプロセスを再起動する
3. When PR 作成直後にクラッシュした場合, Cupola shall 再起動後にブランチ名から既存 PR を検索し、重複作成を防止する
4. The Cupola shall Ctrl+C（SIGINT）受信時に全プロセスに SIGTERM を送信し、10 秒待機後に SIGKILL を送信する graceful shutdown を実行する

### Requirement 11: GitHub 操作の責務分離

**Objective:** 開発者として、Claude Code は git 操作のみ、GitHub API 操作は全て Cupola が行う責務分離を維持したい。Claude Code の依存を最小化し安定動作を確保するため。

#### Acceptance Criteria

1. The Cupola shall 全ての GitHub API 操作（Issue 取得・コメント投稿・close、PR 作成・merge 確認・review thread 監視・コメント返信・resolve）を自ら実行する
2. The Cupola shall Claude Code への入力をファイル（`.cupola/inputs/`）として worktree に書き出す
3. The Cupola shall Claude Code からの出力を output-schema（`--json-schema` フラグ）で構造化して受け取る
4. The Cupola shall Claude Code を `-p` フラグ（非対話モード）、`--output-format json`、`--dangerously-skip-permissions` フラグ付きで起動する

### Requirement 12: ログ管理

**Objective:** 運用者として、Cupola の動作状況を構造化ログで追跡したい。障害調査とデバッグを効率化するため。

#### Acceptance Criteria

1. The Cupola shall `tracing` + `tracing-subscriber` による構造化ログを出力する
2. Where `log.dir` が設定される, Cupola shall stderr に加えて日付別ファイル（`cupola-YYYY-MM-DD.log`）にもログを出力する
3. The Cupola shall polling サイクル開始/終了、イベント検知、状態遷移、プロセス起動/終了、GitHub API 呼び出し、エラー発生をログに記録する
4. The Cupola shall `log.level` 設定に基づきログ出力レベルをフィルタする
