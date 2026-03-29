# Requirements Document

## Introduction
Cupola 用の GitHub Issue テンプレート（`.github/ISSUE_TEMPLATE/cupola-task.yml`）を作成する。Cupola は Issue の本文を cc-sdd の requirements フェーズの入力として使用するため、テンプレートを提供することでユーザーが必要な情報（概要・背景・要求事項・スコープ・技術コンテキスト・受け入れ条件）を漏れなく記述できるようにする。

## Requirements

### Requirement 1: テンプレートファイルの配置
**Objective:** As a Cupola ユーザー, I want GitHub Issue 作成画面でテンプレートを選択できる, so that 必要な情報を構造化された形式で入力できる

#### Acceptance Criteria
1. The Issue テンプレート shall `.github/ISSUE_TEMPLATE/cupola-task.yml` に配置される
2. When GitHub リポジトリの Issue 作成画面を開いた時, the テンプレート shall 選択肢として表示される
3. The テンプレート shall GitHub Issue Forms 形式（YAML）で定義される

### Requirement 2: 必須フィールドの定義
**Objective:** As a Cupola エージェント, I want Issue に概要・背景・要求事項が必ず含まれる, so that cc-sdd の requirements フェーズに十分な入力情報を得られる

#### Acceptance Criteria
1. The テンプレート shall 「概要」フィールドを必須として含む
2. The テンプレート shall 「背景・動機」フィールドを必須として含む
3. The テンプレート shall 「要求事項」フィールドを必須として含む
4. If 必須フィールドが未入力の場合, the GitHub shall Issue の作成を許可しない

### Requirement 3: 任意フィールドの定義
**Objective:** As a Cupola ユーザー, I want 追加のコンテキスト情報を任意で入力できる, so that より精度の高い設計ドキュメントを生成できる

#### Acceptance Criteria
1. The テンプレート shall 「スコープ」フィールドを任意として含む
2. The テンプレート shall 「技術的コンテキスト」フィールドを任意として含む
3. The テンプレート shall 「受け入れ条件」フィールドを任意として含む
4. The テンプレート shall 「補足事項」フィールドを任意として含む

### Requirement 4: テンプレートメタデータ
**Objective:** As a Cupola ユーザー, I want Issue タイトルに自動的にプレフィックスが付与される, so that Cupola 対象の Issue を一目で識別できる

#### Acceptance Criteria
1. The テンプレート shall タイトルプレフィックスとして `[cupola] ` を設定する
2. The テンプレート shall `agent:ready` ラベルを自動付与しない
3. The テンプレート shall テンプレートの名前と説明を含む

### Requirement 5: スコープの制約
**Objective:** As a 開発チーム, I want このタスクのスコープを明確にする, so that 不要な成果物を作成しない

#### Acceptance Criteria
1. このタスクにおけるプロダクト成果物（`.github/ISSUE_TEMPLATE/` 配下に追加するファイル）は Issue テンプレート YAML ファイルのみとし、`.cupola/specs/` 配下にコミットされる設計成果物（requirements/design/tasks/research/spec.json など）はこの制約の対象外とする
2. The 成果物 shall PR テンプレートを含まない
3. The 成果物 shall ラベルの作成を含まない
