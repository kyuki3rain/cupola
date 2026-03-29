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
| 2.1 | 概要フィールド（必須） | cupola-task.yml body[1] | — | — |
| 2.2 | 背景・動機フィールド（必須） | cupola-task.yml body[2] | — | — |
| 2.3 | 要求事項フィールド（必須） | cupola-task.yml body[3] | — | — |
| 2.4 | 必須フィールド未入力時の制御 | validations.required: true | — | — |
| 3.1 | スコープフィールド（任意） | cupola-task.yml body[4] | — | — |
| 3.2 | 技術的コンテキストフィールド（任意） | cupola-task.yml body[5] | — | — |
| 3.3 | 受け入れ条件フィールド（任意） | cupola-task.yml body[6] | — | — |
| 3.4 | 補足事項フィールド（任意） | cupola-task.yml body[7] | — | — |
| 4.1 | タイトルプレフィックス | cupola-task.yml title | — | — |
| 4.2 | ラベル自動付与の抑止 | cupola-task.yml（labels: []） | — | — |
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
- `labels: []` を明示的に設定し、ラベルの自動付与を抑止する

**Dependencies**
- External: GitHub Issue Forms — YAML テンプレートエンジン (P0)

**テンプレート構造定義**

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

  # --- 必須フィールド ---
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

  # --- 任意フィールド ---
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

**Implementation Notes**
- ファイル配置パス: `.github/ISSUE_TEMPLATE/cupola-task.yml`
- `labels: []` を明示的に設定し、ラベルの自動付与を抑止する
- `title` に `[cupola] ` プレフィックスを設定し、ユーザーが続きを入力する形式にする
- 先頭に `markdown` タイプのブロックを配置し、テンプレートの説明文を記載する

## Testing Strategy

### 手動検証
- GitHub リポジトリの Issue 作成画面で「cupola タスク」テンプレートが選択肢に表示されることを確認する
- 必須フィールド（概要・背景・動機・要求事項）を未入力のまま Issue 作成を試み、バリデーションエラーが発生することを確認する
- 全フィールドを入力して Issue を作成し、本文が正しく構造化されていることを確認する
- Issue タイトルに `[cupola] ` プレフィックスが付与されていることを確認する
- `agent:ready` ラベルが自動付与されていないことを確認する

### YAML 構文検証
- YAML リンターでテンプレートファイルの構文が正しいことを確認する
- GitHub Issue Forms 仕様に準拠していることを確認する
