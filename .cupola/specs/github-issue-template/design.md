# Design Document

## Overview
**Purpose**: Cupola 用の GitHub Issue テンプレートを提供し、cc-sdd の requirements フェーズに必要な構造化された入力情報をユーザーから収集する。
**Users**: Cupola を利用する開発者が、Issue 作成時にテンプレートを選択して必要な情報を記述する。

### Goals
- cc-sdd の requirements フェーズに必要な情報を漏れなく収集するためのテンプレートを提供する
- 必須フィールドのバリデーションにより情報の欠落を防止する
- Cupola 対象の Issue をタイトルプレフィックスで識別可能にする

### Non-Goals
- PR テンプレートの作成
- ラベルの作成・管理
- `agent:ready` ラベルの自動付与

## Architecture

### Architecture Pattern & Boundary Map
本機能は単一の YAML 設定ファイルの追加であり、アーキテクチャ上の変更は発生しない。GitHub Issue Forms の仕様に準拠した静的ファイルとして配置する。

### Technology Stack

| Layer | Choice / Version | Role in Feature | Notes |
|-------|------------------|-----------------|-------|
| Infrastructure | GitHub Issue Forms (YAML) | Issue テンプレートの定義 | `.github/ISSUE_TEMPLATE/` に配置 |

## Requirements Traceability

| Requirement | Summary | Components | Interfaces | Flows |
|-------------|---------|------------|------------|-------|
| 1.1 | テンプレートファイルの配置パス | cupola-task.yml | — | — |
| 1.2 | テンプレートの選択可能表示 | cupola-task.yml（name, description） | — | — |
| 1.3 | Issue Forms 形式での定義 | cupola-task.yml（YAML 構造） | — | — |
| 2.1 | 概要フィールド（必須） | cupola-task.yml body[0] | — | — |
| 2.2 | 背景・動機フィールド（必須） | cupola-task.yml body[1] | — | — |
| 2.3 | 要求事項フィールド（必須） | cupola-task.yml body[2] | — | — |
| 2.4 | 必須フィールド未入力時の制御 | validations.required: true | — | — |
| 3.1 | スコープフィールド（任意） | cupola-task.yml body[3] | — | — |
| 3.2 | 技術的コンテキストフィールド（任意） | cupola-task.yml body[4] | — | — |
| 3.3 | 受け入れ条件フィールド（任意） | cupola-task.yml body[5] | — | — |
| 3.4 | 補足事項フィールド（任意） | cupola-task.yml body[6] | — | — |
| 4.1 | タイトルプレフィックス | cupola-task.yml title | — | — |
| 4.2 | ラベル自動付与の抑止 | cupola-task.yml（labels 省略） | — | — |
| 4.3 | テンプレート名・説明 | cupola-task.yml（name, description） | — | — |
| 5.1 | 成果物は YAML ファイルのみ | — | — | — |
| 5.2 | PR テンプレート対象外 | — | — | — |
| 5.3 | ラベル作成対象外 | — | — | — |

## Components and Interfaces

| Component | Domain/Layer | Intent | Req Coverage | Key Dependencies | Contracts |
|-----------|-------------|--------|--------------|-----------------|-----------|
| cupola-task.yml | Infrastructure | Cupola 用 Issue テンプレート定義 | 1.1–4.3 | GitHub Issue Forms (P0) | — |

### Infrastructure

#### cupola-task.yml

| Field | Detail |
|-------|--------|
| Intent | GitHub Issue 作成画面で Cupola タスク用の構造化フォームを提供する |
| Requirements | 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 4.3 |

**Responsibilities & Constraints**
- GitHub Issue Forms 仕様に準拠した YAML ファイルとして定義する
- 必須フィールド（「概要」「背景・動機」「要求事項」）は `validations.required: true` を設定する
- 任意フィールド（スコープ・技術的コンテキスト・受け入れ条件・補足事項）は `validations.required: false` を設定する
- `labels` プロパティは設定しない（`agent:ready` ラベルの自動付与を抑止）

**Dependencies**
- External: GitHub Issue Forms — YAML テンプレートエンジン (P0)

**テンプレート構造定義**

```yaml
name: Cupola Task
description: Cupola エージェントに処理させるタスクを記述する Issue テンプレート
title: "[cupola] "
body:
  # --- 必須フィールド ---
  - type: textarea
    id: summary
    attributes:
      label: 概要
      description: タスクの概要を簡潔に記述してください
    validations:
      required: true

  - type: textarea
    id: background
    attributes:
      label: 背景・動機
      description: なぜこのタスクが必要なのか、背景や動機を記述してください
    validations:
      required: true

  - type: textarea
    id: requirements
    attributes:
      label: 要求事項
      description: 具体的な要求事項を箇条書きで記述してください
    validations:
      required: true

  # --- 任意フィールド ---
  - type: textarea
    id: scope
    attributes:
      label: スコープ
      description: "やること・やらないことを記述してください（任意）"
    validations:
      required: false

  - type: textarea
    id: technical-context
    attributes:
      label: 技術的コンテキスト
      description: "関連する技術的な制約や前提条件を記述してください（任意）"
    validations:
      required: false

  - type: textarea
    id: acceptance-criteria
    attributes:
      label: 受け入れ条件
      description: "完了条件を箇条書きで記述してください（任意）"
    validations:
      required: false

  - type: textarea
    id: notes
    attributes:
      label: 補足事項
      description: "その他の補足情報があれば記述してください（任意）"
    validations:
      required: false
```

**Implementation Notes**
- ファイル配置パス: `.github/ISSUE_TEMPLATE/cupola-task.yml`
- `labels` プロパティを意図的に省略することで、ラベルの自動付与を抑止する
- `title` に `[cupola] ` プレフィックスを設定し、ユーザーが続きを入力する形式にする

## Testing Strategy

### 手動検証
- GitHub リポジトリの Issue 作成画面で「Cupola Task」テンプレートが選択肢に表示されることを確認する
- 必須フィールド（概要・背景・動機・要求事項）を未入力のまま Issue 作成を試み、バリデーションエラーが発生することを確認する
- 全フィールドを入力して Issue を作成し、本文が正しく構造化されていることを確認する
- Issue タイトルに `[cupola] ` プレフィックスが付与されていることを確認する
- `agent:ready` ラベルが自動付与されていないことを確認する

### YAML 構文検証
- YAML リンターでテンプレートファイルの構文が正しいことを確認する
- GitHub Issue Forms 仕様に準拠していることを確認する
