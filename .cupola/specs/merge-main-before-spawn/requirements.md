# Requirements Document

## Project Description (Input)
spawn前にoriginのデフォルトブランチをマージしてagentのworktreeとCIのコードを一致させる機能

## Introduction

Cupola は GitHub Issues/PRs を唯一のインターフェースとして、Claude Code を用いた実装を自動化するシステムである。
Fixing 状態のIssueに対してagentをspawnする際、CIはPRブランチとmainの **merge commit** 上でテストするのに対し、agentのworktreeはPRブランチのままである。
他のPRがmainにマージされると、CIではコンパイルエラーが発生するが、agentはそれを再現できず修正ループに陥る。
本機能はspawn前にデフォルトブランチをフェッチ・マージすることでagentのworktreeとCIのコードベースを一致させ、この乖離を解消する。

## Requirements

### Requirement 1: spawn前のoriginフェッチ

**Objective:** Cupola開発者として、Fixing状態のIssueをspawnする前にoriginのデフォルトブランチをフェッチしたい。そうすることで、agentが最新のリモートコードを把握できる状態でspawnされる。

#### Acceptance Criteria

1. When Fixing状態のIssueに対してspawnを実行するとき, the Cupola shall `origin` リモートから最新の情報をフェッチする
2. The Cupola shall フェッチ処理をspawn処理(`step7_spawn_processes`)内のFixing状態ハンドラで実行する
3. If フェッチが失敗した（ネットワークエラー等）, the Cupola shall 警告ログを出力してフェッチをスキップし、マージなしでspawnを継続する

---

### Requirement 2: マージのクリーン成功時の動作

**Objective:** Cupola開発者として、フェッチ後にデフォルトブランチとのマージがコンフリクトなく成功した場合にspawnを継続したい。そうすることで、agentのworktreeがCIと同じマージコミット状態でspawnされる。

#### Acceptance Criteria

1. When `git merge origin/{default_branch}` がコンフリクトなく完了したとき, the Cupola shall worktreeをマージ後の状態のままspawnを実行する
2. The Cupola shall マージコマンドに `--no-edit` オプションを付与してコミットメッセージ入力を不要にする
3. The Cupola shall クリーンマージが完了した場合に追加のcausesを設定せずspawnを継続する

---

### Requirement 3: マージコンフリクト発生時の動作

**Objective:** Cupola開発者として、デフォルトブランチとのマージでコンフリクトが発生した場合にagentへの適切な指示を提供したい。そうすることで、agentがコンフリクトを解消してから実装作業に取り掛かれる。

#### Acceptance Criteria

1. When `git merge origin/{default_branch}` がコンフリクトにより失敗したとき, the Cupola shall Issue の causes リストに `Conflict` を追加する
2. When マージコンフリクトが発生したとき, the Cupola shall worktreeをマージ中状態のままspawnを継続する（マージを中断しない）
3. When マージコンフリクトが発生したとき, the Cupola shall Fixing用プロンプトの先頭にマージコンフリクト解消指示を挿入する
4. The Cupola shall コンフリクト解消指示を英語で記述する（既存プロンプト慣例に合わせる）
5. The Cupola shall コンフリクト解消指示に以下の内容を含める: コンフリクトマーカーのファイル確認、`git add`によるステージング、`git commit --no-edit`によるマージ完了、その後の修正作業への移行

---

### Requirement 4: マージ失敗（コンフリクト以外）時の動作

**Objective:** Cupola開発者として、フェッチ・マージ処理が予期しない理由で失敗した場合でもspawnが継続されてほしい。そうすることで、ネットワーク障害等の一時的な問題によってspawn全体がブロックされない。

#### Acceptance Criteria

1. If フェッチまたはマージがコンフリクト以外の理由で失敗したとき, the Cupola shall 警告ログを出力する
2. If フェッチまたはマージがコンフリクト以外の理由で失敗したとき, the Cupola shall マージをスキップしてspawnを継続する
3. The Cupola shall フェッチ・マージ失敗によってspawn全体を中断しない

---

### Requirement 5: GitWorktreeトレイトへのmergeメソッド追加

**Objective:** アーキテクチャ担当者として、マージ操作をportトレイトとして定義したい。そうすることで、Clean Architectureの依存関係ルールに従いapplicationレイヤーがadapterの具体実装に依存しない。

#### Acceptance Criteria

1. The Cupola shall `application/port`の`GitWorktree`トレイトに`merge(wt: &Path, branch: &str) -> Result<()>`メソッドを追加する
2. When `git merge --no-edit`がコンフリクトにより失敗したとき, the Cupola shall adapterの実装が`Err`を返す
3. The Cupola shall `adapter/outbound`の`GitWorktreeManager`に`git merge --no-edit`の具体実装を追加する
4. The Cupola shall マージ成功時に`Ok(())`を返し、コンフリクト時に識別可能なエラーを返す

---

### Requirement 6: Fixing用プロンプトのマージコンフリクトフラグ対応

**Objective:** Cupola開発者として、`build_fixing_prompt`関数がマージコンフリクト状態を受け取れるようにしたい。そうすることで、コンフリクト発生時にプロンプト先頭にコンフリクト解消指示が挿入される。

#### Acceptance Criteria

1. The Cupola shall `build_fixing_prompt`関数にマージコンフリクト状態を示すフラグパラメータを追加する
2. When マージコンフリクトフラグが真であるとき, the Cupola shall プロンプトの先頭（他の指示の前）にコンフリクト解消セクションを挿入する
3. While マージコンフリクトフラグが偽であるとき, the Cupola shall 既存プロンプト内容を変更せずそのまま返す
