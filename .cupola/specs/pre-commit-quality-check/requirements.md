# 要件定義書

## Project Description (Input)
commit前の品質チェック手順をcupola の design/implementation/fixing プロンプトに追加する

## はじめに

cupolaは、GitHub IssuesとPRをインターフェースとして、Claude Codeを自動制御し設計・実装を自動化するエージェントシステムである。現在、Claude Codeが `cargo fmt` / `cargo clippy` / `cargo test` を実行せずにcommit・pushするケースが発生しており、CIで失敗した壊れたPRが放置される問題がある。

本機能では、`src/application/prompt.rs` の設計・実装・レビュー対応の各プロンプト生成関数（`build_design_prompt`、`build_implementation_prompt`、`build_fixing_prompt`）に対して、commit前の品質チェック手順を明示的に追加する。これにより、Claude Codeが自律的にcommit前の品質チェック→修正ループを実行し、CIの失敗を未然に防ぐことを目的とする。

## Requirements

### Requirement 1: 設計プロンプトへの品質チェック手順の追加

**Objective:** 設計エージェントとして、commit前に品質チェックを実行したい。そうすることで、壊れたコードをpushせずに済む。

#### Acceptance Criteria

1. When `build_design_prompt` が呼び出された場合、the prompt shall commitステップの直前に以下の品質チェック手順を含む:
   - `cargo fmt` の実行
   - `cargo clippy -- -D warnings` の実行
   - `cargo test` の実行
2. If 品質チェックのいずれかが失敗した場合、the prompt shall Claude Code に修正してから再度チェックを実行するよう指示する
3. The prompt shall 品質チェックの全項目がパスした後にのみcommitを実行するよう指示する
4. The prompt shall 品質チェック手順がcommit / push 手順の直前に配置されている

### Requirement 2: 実装プロンプトへの品質チェック手順の追加

**Objective:** 実装エージェントとして、commit前に品質チェックを実行したい。そうすることで、CI で失敗するコードをpushせずに済む。

#### Acceptance Criteria

1. When `build_implementation_prompt` が呼び出された場合、the prompt shall commitステップの直前に以下の品質チェック手順を含む:
   - `cargo fmt` の実行
   - `cargo clippy -- -D warnings` の実行
   - `cargo test` の実行
2. If 品質チェックのいずれかが失敗した場合、the prompt shall Claude Code に修正してから再度チェックを実行するよう指示する
3. The prompt shall 品質チェックの全項目がパスした後にのみcommitを実行するよう指示する
4. The prompt shall `feature_name` が指定されている場合・指定されていない場合のどちらのパスにおいても、commitの直前に品質チェックを含む

### Requirement 3: レビュー対応プロンプトへの品質チェック手順の追加

**Objective:** レビュー対応エージェントとして、commit前に品質チェックを実行したい。そうすることで、レビュー修正によって新たに壊れた箇所をpushせずに済む。

#### Acceptance Criteria

1. When `build_fixing_prompt` が呼び出された場合、the prompt shall commitステップの直前に以下の品質チェック手順を含む:
   - `cargo fmt` の実行
   - `cargo clippy -- -D warnings` の実行
   - `cargo test` の実行
2. If 品質チェックのいずれかが失敗した場合、the prompt shall Claude Code に修正してから再度チェックを実行するよう指示する
3. The prompt shall 品質チェックの全項目がパスした後にのみcommitを実行するよう指示する

### Requirement 4: 既存テストとの整合性

**Objective:** 開発者として、既存のテストが引き続きパスすることを確認したい。そうすることで、プロンプト変更がリグレッションを引き起こしていないことを保証できる。

#### Acceptance Criteria

1. The prompt module shall 既存のすべてのユニットテスト（`design_running_returns_pr_creation_schema`、`implementation_running_returns_pr_creation_schema` 等）がパスし続ける
2. When 品質チェック手順がプロンプトに追加された後も、the prompt shall `output_schema` の種別（`PrCreation` / `Fixing`）が変更されない
3. The prompt shall commit前の品質チェックに関する手順が追加された各プロンプトに対して、テストで内容の存在を検証できる
