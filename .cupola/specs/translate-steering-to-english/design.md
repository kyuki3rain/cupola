# Design Document: translate-steering-to-english

## Overview

**Purpose**: `.cupola/steering/` 配下の 3 ファイル（product.md, tech.md, structure.md）を英語に翻訳し、OSS プロジェクトとしての言語的一貫性を確保する。

**Users**: プロジェクト貢献者、AI エージェント（Claude Code）、OSS 利用者が英語化された技術コンテキストを参照する。

**Impact**: steering ドキュメントの記述言語が日本語から英語に変更される。spec ドキュメントには影響しない。

### Goals
- product.md, tech.md, structure.md の全内容を英語に翻訳する
- 翻訳前と同等の情報量・正確性を維持する
- Markdown フォーマット（見出し、テーブル、コードブロック）を保持する
- 固有名詞・コード識別子を変更しない

### Non-Goals
- spec ドキュメント（`.cupola/specs/`）の翻訳
- steering ドキュメントの内容の追加・変更・削除
- 新規 steering ファイルの作成
- CLAUDE.md や他のプロジェクトドキュメントの翻訳

## Architecture

### Existing Architecture Analysis

本機能はコード変更を伴わないドキュメント翻訳タスクである。対象ファイルは `.cupola/steering/` 配下の 3 ファイルのみで、他のシステムコンポーネントへの影響はない。

- **現行構造**: `.cupola/steering/` に product.md, tech.md, structure.md が日本語で配置
- **変更後**: 同一ファイルの内容が英語に置換される
- **制約**: ファイルパス・ファイル名・Markdown 構造は変更しない

### Architecture Pattern & Boundary Map

本機能はドキュメント翻訳のみであり、アーキテクチャパターンの選択は不要。既存のファイル配置とディレクトリ構造をそのまま維持する。

### Technology Stack

| Layer | Choice / Version | Role in Feature | Notes |
|-------|------------------|-----------------|-------|
| ドキュメント | Markdown | steering ドキュメントのフォーマット | 構造を維持したまま翻訳 |

## Requirements Traceability

| Requirement | Summary | Components | Interfaces | Flows |
|-------------|---------|------------|------------|-------|
| 1.1 | product.md の全セクション翻訳 | TranslateProduct | - | - |
| 1.2 | product.md の情報量保持 | TranslateProduct | - | - |
| 1.3 | 自然な英語表現 | TranslateProduct | - | - |
| 2.1 | tech.md の全セクション翻訳 | TranslateTech | - | - |
| 2.2 | 技術用語の正確な維持 | TranslateTech | - | - |
| 2.3 | テーブル構造の保持 | TranslateTech | - | - |
| 2.4 | コードブロック内コメントの翻訳 | TranslateTech | - | - |
| 3.1 | structure.md の全セクション翻訳 | TranslateStructure | - | - |
| 3.2 | レイヤー名・パスの維持 | TranslateStructure | - | - |
| 3.3 | コード識別子の維持 | TranslateStructure | - | - |
| 4.1 | Markdown フォーマット保持 | 全コンポーネント共通 | - | - |
| 4.2 | 固有名詞の維持 | 全コンポーネント共通 | - | - |
| 4.3 | ニュアンス優先の意訳 | 全コンポーネント共通 | - | - |
| 4.4 | spec への非影響 | 全コンポーネント共通 | - | - |

## Components and Interfaces

本機能はコード変更を伴わないため、コンポーネントは翻訳対象ファイルごとの作業単位として定義する。

| Component | Domain/Layer | Intent | Req Coverage | Key Dependencies | Contracts |
|-----------|------------|--------|--------------|------------------|-----------|
| TranslateProduct | Steering | product.md を英語に翻訳 | 1.1, 1.2, 1.3, 4.1, 4.2, 4.3, 4.4 | なし | - |
| TranslateTech | Steering | tech.md を英語に翻訳 | 2.1, 2.2, 2.3, 2.4, 4.1, 4.2, 4.3, 4.4 | なし | - |
| TranslateStructure | Steering | structure.md を英語に翻訳 | 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 4.4 | なし | - |

### Steering Layer

#### TranslateProduct

| Field | Detail |
|-------|--------|
| Intent | product.md の全内容を英語に翻訳する |
| Requirements | 1.1, 1.2, 1.3, 4.1, 4.2, 4.3, 4.4 |

**Responsibilities & Constraints**
- Product Overview セクションの概要文を英語に翻訳
- Core Capabilities, Target Use Cases, Value Proposition の各セクションを英語に翻訳
- Cupola, Claude Code, cc-sdd, GitHub, Issue, PR 等の固有名詞はそのまま維持
- 見出しレベルとリスト構造を保持

**Dependencies**
- なし（独立した翻訳タスク）

**Implementation Notes**
- 製品コンセプトの説明であるため、技術的正確性よりも読みやすさを重視
- 日本語特有の表現（「ワンストップ自動化」等）は英語で自然な表現に意訳

#### TranslateTech

| Field | Detail |
|-------|--------|
| Intent | tech.md の全内容を英語に翻訳する |
| Requirements | 2.1, 2.2, 2.3, 2.4, 4.1, 4.2, 4.3, 4.4 |

**Responsibilities & Constraints**
- Architecture, Core Technologies, Key Libraries, Development Standards, Development Environment, Key Technical Decisions の各セクションを翻訳
- Key Libraries テーブルの「用途」「クレート」「パターン」カラムヘッダーとセル内容を英語化
- コードブロック内の日本語コメント（`# ビルド` 等）を英語に翻訳
- クレート名、コマンド、型名などの技術識別子はそのまま維持
- テーブルの列数・行数・アライメントを保持

**Dependencies**
- なし（独立した翻訳タスク）

**Implementation Notes**
- 技術用語が最も多いファイルであり、誤訳リスクが最も高い
- Rust エコシステムの用語（crate, derive, trait 等）は英語のまま
- 日本語のテーブルヘッダーを英訳する際にカラム幅の調整が必要になる場合がある

#### TranslateStructure

| Field | Detail |
|-------|--------|
| Intent | structure.md の全内容を英語に翻訳する |
| Requirements | 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 4.4 |

**Responsibilities & Constraints**
- Organization Philosophy, Directory Patterns, Naming Conventions, Code Organization Principles の各セクションを翻訳
- ディレクトリパス（`src/domain/`, `src/adapter/` 等）をそのまま維持
- 型名（`GitHubClientImpl`, `PollingUseCase` 等）、関数名、定数名をそのまま維持
- 命名規約の説明（snake_case, UpperCamelCase 等）はそのまま維持

**Dependencies**
- なし（独立した翻訳タスク）

**Implementation Notes**
- コード識別子が多く混在するため、翻訳対象と非翻訳対象の境界に注意
- 各レイヤーの Purpose, Contains, Rule 等の説明文のみを翻訳

## Testing Strategy

### 翻訳品質検証
- product.md: 全セクションが英語で記述されていること、固有名詞が維持されていること
- tech.md: テーブル構造が保持されていること、コードブロック内コメントが英語であること、技術用語が正確であること
- structure.md: ディレクトリパスとコード識別子が変更されていないこと
- 全ファイル: Markdown フォーマット（見出しレベル、リスト、テーブル）が保持されていること
- 全ファイル: 翻訳前と同等の情報量が維持されていること（セクション数、箇条書き項目数の一致）
