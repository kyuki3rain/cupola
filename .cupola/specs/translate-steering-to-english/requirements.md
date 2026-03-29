# Requirements Document

## Introduction
`.cupola/steering/` 配下のドキュメント（product.md, tech.md, structure.md）を英語に翻訳する。OSS として公開済みのプロジェクトであり、README も英語化されているため、AI が参照する技術コンテキストとしての言語的一貫性を確保することが目的である。spec ドキュメントはレビュアー向けに日本語を維持する。

## Requirements

### Requirement 1: product.md の英語翻訳
**Objective:** プロジェクトの貢献者・利用者として、product.md が英語で記述されていることで、国際的な開発者が製品概要を理解できるようにしたい

#### Acceptance Criteria
1. The 翻訳プロセス shall product.md の全セクション（Core Capabilities, Target Use Cases, Value Proposition）を英語に翻訳する
2. The 翻訳結果 shall 翻訳前の product.md と同等の情報量を保持する
3. The 翻訳結果 shall OSS プロジェクトとして自然な英語表現を使用する

### Requirement 2: tech.md の英語翻訳
**Objective:** AI エージェントおよび開発者として、tech.md が英語で記述されていることで、技術スタックとアーキテクチャの理解を言語障壁なく行いたい

#### Acceptance Criteria
1. The 翻訳プロセス shall tech.md の全セクション（Architecture, Core Technologies, Key Libraries, Development Standards, Development Environment, Key Technical Decisions）を英語に翻訳する
2. The 翻訳結果 shall 技術用語（クレート名、コマンド、コード例）を正確に維持する
3. The 翻訳結果 shall テーブル形式（Key Libraries）の構造とレイアウトを保持する
4. The 翻訳結果 shall コードブロック内のコメントも英語に翻訳する

### Requirement 3: structure.md の英語翻訳
**Objective:** AI エージェントおよび開発者として、structure.md が英語で記述されていることで、プロジェクト構造と命名規約を言語障壁なく参照したい

#### Acceptance Criteria
1. The 翻訳プロセス shall structure.md の全セクション（Organization Philosophy, Directory Patterns, Naming Conventions, Code Organization Principles）を英語に翻訳する
2. The 翻訳結果 shall レイヤー名（domain, application, adapter, bootstrap）とディレクトリパスを正確に維持する
3. The 翻訳結果 shall 型名・関数名・定数名などのコード識別子を変更しない

### Requirement 4: 翻訳品質の保証
**Objective:** プロジェクトメンテナーとして、翻訳後のドキュメントが原文と同等の品質を持つことで、情報の欠落や誤解を防ぎたい

#### Acceptance Criteria
1. The 翻訳結果 shall 全ファイルで Markdown のフォーマット（見出しレベル、リスト、テーブル、コードブロック）を保持する
2. The 翻訳結果 shall 固有名詞（Cupola, Claude Code, cc-sdd 等）を翻訳せずそのまま維持する
3. If 翻訳により意味が曖昧になる場合, then the 翻訳プロセス shall 原文のニュアンスを優先して意訳する
4. The 翻訳結果 shall spec ドキュメント（`.cupola/specs/`）には影響を与えない
