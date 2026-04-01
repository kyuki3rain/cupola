# Requirements Document

## Project Description (Input)
プロンプトの品質チェックハードコードを削除し AGENTS.md 参照に変更する。design/implementation/fixing プロンプトから cargo fmt/clippy/test などの言語固有コマンドを削除し、汎用的な「AGENTS.md / CLAUDE.md の指示に従え」という指示に置き換える。

## Introduction

cupola が Claude Code に渡すプロンプト（`build_design_prompt`、`build_implementation_prompt`、`build_fixing_prompt`）には、品質チェックとして `cargo fmt`・`cargo clippy -- -D warnings`・`cargo test` がハードコードされている。これらは Rust プロジェクト固有のコマンドであり、他言語のプロジェクトでは意味をなさない。Claude Code は各プロジェクトの AGENTS.md や CLAUDE.md を自動的に読み込むため、プロンプト側では「プロジェクト規約に従え」という汎用指示を与えるだけで十分である。本仕様では、各プロンプト関数からハードコードされたコマンドを削除し、汎用的な品質チェック指示に置き換える変更を定義する。

## Requirements

### Requirement 1: design プロンプトからの言語固有コマンド削除

**Objective:** As a cupola ユーザー, I want design プロンプトが Rust 固有コマンドに依存しないようにしたい, so that Rust 以外のプロジェクトでも設計エージェントが正しく動作する

#### Acceptance Criteria
1. When `build_design_prompt` が呼ばれる, the Prompt Builder shall `cargo fmt`、`cargo clippy`、`cargo test` のいずれも含まないプロンプト文字列を返す
2. When `build_design_prompt` が呼ばれる, the Prompt Builder shall 「commit 前に AGENTS.md / CLAUDE.md に記載された品質チェックを実行し、全てパスしてから commit すること。失敗した場合は修正して再チェックすること。」に相当する汎用品質チェック指示を含むプロンプト文字列を返す
3. The Prompt Builder shall 既存の commit / push 手順（`git add`、`git commit`、`git push` の指示）を引き続き含むプロンプト文字列を返す

### Requirement 2: implementation プロンプトからの言語固有コマンド削除

**Objective:** As a cupola ユーザー, I want implementation プロンプトが Rust 固有コマンドに依存しないようにしたい, so that 多言語プロジェクトの実装エージェントが正しく動作する

#### Acceptance Criteria
1. When `build_implementation_prompt` が feature_name あり・なしのどちらで呼ばれる場合でも, the Prompt Builder shall `cargo fmt`、`cargo clippy`、`cargo test` のいずれも含まないプロンプト文字列を返す
2. When `build_implementation_prompt` が呼ばれる, the Prompt Builder shall 汎用品質チェック指示（AGENTS.md / CLAUDE.md に記載された品質チェックに従う旨）を含むプロンプト文字列を返す
3. The Prompt Builder shall `Closes #{issue_number}` の形式の issue 参照を引き続き含むプロンプト文字列を返す

### Requirement 3: fixing プロンプトからの言語固有コマンド削除

**Objective:** As a cupola ユーザー, I want fixing プロンプトが Rust 固有コマンドに依存しないようにしたい, so that Rust 以外のプロジェクトのレビュー対応エージェントが正しく動作する

#### Acceptance Criteria
1. When `build_fixing_prompt` が呼ばれる, the Prompt Builder shall `cargo fmt`、`cargo clippy`、`cargo test` のいずれも含まないプロンプト文字列を返す
2. When `build_fixing_prompt` が呼ばれる, the Prompt Builder shall 汎用品質チェック指示（AGENTS.md / CLAUDE.md に記載された品質チェックに従う旨）を含むプロンプト文字列を返す
3. The Prompt Builder shall review コメント・CI 失敗・コンフリクトの各ケースに応じた修正指示を引き続き含むプロンプト文字列を返す

### Requirement 4: テストの更新

**Objective:** As a 開発者, I want 品質チェック指示の内容を検証するテストが新仕様に追従していることを確認したい, so that リグレッションを防ぐ

#### Acceptance Criteria
1. When `cargo test` を実行した場合, the Test Suite shall `cargo fmt` / `cargo clippy` / `cargo test` の存在をアサートするテストが存在せず、全テストがパスする
2. When `cargo test` を実行した場合, the Test Suite shall design / implementation / fixing 各プロンプトが汎用品質チェック指示を含むことを検証するテストがパスする
3. The Test Suite shall 削除・変更した旧テストと同等以上のカバレッジを汎用指示の観点から提供する
