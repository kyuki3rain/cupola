# Implementation Plan

- [x] 1. Skill ファイルの作成（`.claude/commands/cupola/` 配下）
- [x] 1.1 (P) spec-design skill の作成
  - cc-sdd の spec-requirements、spec-design、spec-tasks のプロンプトロジックを1ファイルに統合する
  - 引数として spec id（`issue-{number}`）を受け取り、steering コンテキストとルール・テンプレートを参照して requirements.md、research.md、design.md、tasks.md を一気通貫で生成する
  - spec.json の phase を `tasks-generated` に更新する指示を含める
  - 中間承認なしの一気通貫フローとして設計する
  - _Requirements: 1.1, 1.2, 1.6_

- [x] 1.2 (P) spec-impl skill の作成
  - cc-sdd の spec-impl.md をベースに namespace 参照を `/cupola:*` に変更する
  - TDD サイクル（RED → GREEN → REFACTOR → VERIFY → MARK）の指示を維持する
  - spec id と オプションのタスク番号を引数として受け取る
  - _Requirements: 1.1, 1.3_

- [x] 1.3 (P) spec-compress skill の作成
  - 完了済み spec を走査し、requirements/design/tasks の要点を summary.md に集約する skill を新規作成する
  - 元ファイルの削除と spec.json の phase を `archived` に更新する指示を含める
  - _Requirements: 1.1, 1.4_

- [x] 1.4 (P) steering skill の作成
  - cc-sdd の steering.md をベースに namespace 参照を `/cupola:*` に変更する
  - product.md、tech.md、structure.md の生成・更新機能を維持する
  - _Requirements: 1.1, 1.5_

- [x] 2. Domain 層の変更
- [x] 2.1 Issue エンティティの feature_name 型変更
  - `feature_name` を `Option<String>` から `String` に変更する
  - 初期化時に `issue-{number}` を設定する設計に合わせて、エンティティの生成ロジックを更新する
  - DB マイグレーションとして既存の NULL レコードに `'issue-' || github_issue_number` をデフォルト値として適用する
  - この変更に依存するコンパイルエラーを全て解消する（adapter 層の repository 実装、application 層の use case など）
  - _Requirements: 2.4_

- [x] 3. Application 層の変更
- [x] 3.1 prompt.rs の簡素化
  - `build_design_prompt` を `/cupola:spec-design issue-{number}` を1行渡すだけの簡潔な prompt に書き換える
  - `build_implementation_prompt` を `/cupola:spec-impl issue-{number}` を渡す prompt に書き換え、`feature_name` パラメータを `&str`（非 Option）に変更する
  - `build_session_config` の `feature_name` パラメータを `Option<&str>` から `&str` に変更する
  - `PR_CREATION_SCHEMA` から `feature_name` フィールドを削除する
  - `build_fixing_prompt` は変更しない
  - 既存テスト20件超を新しいシグネチャと prompt 内容に合わせて書き換える
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 3.2 polling_use_case での spec ディレクトリ初期化
  - FileGenerator trait に `generate_spec_directory` メソッドを追加し、adapter で実装した
  - テンプレートから spec.json を生成し `feature_name` に `issue-{number}` を設定
  - issue 本文を `PROJECT_DESCRIPTION` として requirements.md に埋め込む
  - 注: polling_use_case への呼び出し統合は別タスクで対応（worktree 内の issue body 取得が必要）
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 3.3 CompressUseCase の新規作成
  - `.cupola/specs/` 配下の完了 spec を検索し、存在しなければメッセージ表示して正常終了する
  - `CompressReport` として圧縮件数やスキップ理由を返す
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 4. Adapter / CLI 層の変更
- [x] 4.1 (P) compress サブコマンドの追加
  - `cli.rs` に `Compress` サブコマンドを追加する
  - `bootstrap/app.rs` で `CompressUseCase` の wiring を行い、コマンド実行時に呼び出す
  - _Requirements: 4.1_

- [x] 4.2 (P) FileGenerator の spec ディレクトリ生成実装
  - `init_file_generator.rs` に `generate_spec_directory` メソッドを実装する
  - テンプレートファイル（`init.json`、`requirements-init.md`）を読み込み、プレースホルダを置換して書き出す
  - 既に存在する場合は冪等に false を返す
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 5. kiro 参照の完全排除と設定更新
- [x] 5.1 kiro skill ディレクトリの削除
  - `.claude/commands/kiro/` ディレクトリを完全に削除する
  - _Requirements: 5.1_

- [x] 5.2 コードベース内の kiro 参照更新
  - `doctor_use_case.rs` の `/kiro:steering` 参照を `/cupola:steering` に変更する
  - `CLAUDE.md` の全 `/kiro:*` 参照を `/cupola:*` に更新し、新しいワークフローを反映する
  - `.claude/settings.local.json` の kiro skill permission を cupola skill に書き換える
  - `fallback_pr_body` の "cc-sdd" 参照を更新する
  - _Requirements: 5.2_

- [x] 5.3 (P) settings ファイルの維持確認
  - `.cupola/settings/rules/` と `.cupola/settings/templates/specs/` 配下のファイルが skill から正しく参照されることを確認する
  - テンプレート内の `/kiro:*` コメント（`requirements-init.md` 内の参照等）を更新する
  - _Requirements: 5.3, 1.6_

- [x] 6. 統合テストと動作確認
- [x] 6.1 全体ビルドとテスト実行
  - `cargo build`、`cargo clippy`、`cargo test` を実行して全てパスすることを確認する
  - prompt.rs の書き換えたテストが正しく動作することを重点的に確認する
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 6.2 (P) compress の結合動作確認
  - `cupola compress` コマンドが正常に起動し、完了 spec がない場合にメッセージ表示して終了することを確認する
  - CompressUseCase のユニットテスト4件を作成して通過を確認した
  - _Requirements: 4.1, 4.2, 4.3_
