# Implementation Plan

- [ ] 1. (P) product.md を英語に翻訳する
  - Product Overview の概要文を英語に翻訳する
  - Core Capabilities セクションの 4 つの機能説明（Issue 検知→設計自動生成、PR ベースのレビューフロー、ステートマシン駆動、責務分離）を英語に翻訳する
  - Target Use Cases セクションの 3 つのユースケースを英語に翻訳する
  - Value Proposition セクションの価値提案文を英語に翻訳する
  - 固有名詞（Cupola, Claude Code, cc-sdd, GitHub, Issue, PR）はそのまま維持する
  - 見出しレベル・リスト構造・太字マークダウンを保持する
  - 日本語特有の表現（「ワンストップ自動化」等）は英語で自然な表現に意訳する
  - _Requirements: 1.1, 1.2, 1.3, 4.1, 4.2, 4.3_

- [ ] 2. (P) tech.md を英語に翻訳する
  - Architecture セクションの 4 レイヤー説明を英語に翻訳する
  - Core Technologies セクションの技術選定説明を英語に翻訳する
  - Key Libraries テーブルのヘッダー（用途→Purpose、クレート→Crate、パターン→Pattern）とセル内容を英語に翻訳する。テーブルの列数・行数・アライメントを保持する
  - Development Standards セクション（Type Safety, Code Quality, Testing）を英語に翻訳する
  - Development Environment セクション（Required Tools, Common Commands）を英語に翻訳する
  - コードブロック内の日本語コメント（`# ビルド`、`# 全テスト実行` 等）を英語に翻訳する
  - Key Technical Decisions セクションの 3 つの技術判断を英語に翻訳する
  - クレート名（octocrab, rusqlite 等）、コマンド（cargo build 等）、型名は変更しない
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 4.1, 4.2, 4.3_

- [ ] 3. (P) structure.md を英語に翻訳する
  - Organization Philosophy セクションの設計思想を英語に翻訳する
  - Directory Patterns セクションの 4 レイヤー（Domain, Application, Adapter, Bootstrap）の Purpose, Contains, Rule 説明を英語に翻訳する
  - Naming Conventions セクションのファイル・型・関数・定数・ポート・アダプターの命名規約説明を英語に翻訳する
  - Code Organization Principles セクションの 4 つの原則を英語に翻訳する
  - ディレクトリパス（`src/domain/`, `src/adapter/` 等）をそのまま維持する
  - 型名（`GitHubClientImpl`, `PollingUseCase` 等）、関数名（`find_by_issue_number` 等）、定数名をそのまま維持する
  - 命名規約の記法（snake_case, UpperCamelCase, SCREAMING_SNAKE_CASE）はそのまま維持する
  - _Requirements: 3.1, 3.2, 3.3, 4.1, 4.2, 4.3_

- [ ] 4. 翻訳品質の検証
  - 3 ファイル全てが英語で記述されていることを確認する
  - 各ファイルのセクション数・箇条書き項目数が翻訳前と一致していることを確認する
  - Markdown フォーマット（見出しレベル、リスト、テーブル、コードブロック）が保持されていることを確認する
  - 固有名詞（Cupola, Claude Code, cc-sdd 等）とコード識別子が変更されていないことを確認する
  - spec ドキュメント（`.cupola/specs/`）が変更されていないことを確認する
  - _Requirements: 4.1, 4.2, 4.4_
