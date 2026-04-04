# Requirements Document

## Introduction
Cupola は現在 cc-sdd (MIT) の `/kiro:*` skill に依存しているが、Cupola 独自の拡張（spec 一気通貫生成、spec id の決定論的命名、spec 圧縮）を行うにあたり、`/cupola:*` namespace に移行し、spec ライフサイクルを再設計する。skill は `/cupola:spec-design`（requirements + design + tasks 一気通貫）、`/cupola:spec-impl`（TDD 実装）、`/cupola:spec-compress`（完了 spec 要約アーカイブ）、`/cupola:steering`（steering ファイル生成・更新）の4つに再構成する。

## Requirements

### Requirement 1: Skill ファイル再構成
**Objective:** As a Cupola 開発者, I want `/kiro:*` の11 skill を `/cupola:*` の4 skill に再構成したい, so that Cupola 独自のワークフローに最適化された skill 体系を持てる

#### Acceptance Criteria
1. The Cupola skill system shall `.claude/commands/cupola/` 配下に `spec-design.md`、`spec-impl.md`、`spec-compress.md`、`steering.md` の4ファイルを提供する
2. When `/cupola:spec-design issue-{number}` が実行された時, the spec-design skill shall requirements.md、research.md、design.md、tasks.md を一気通貫で生成し、spec.json の phase を `tasks-generated` に更新する
3. When `/cupola:spec-impl issue-{number}` が実行された時, the spec-impl skill shall TDD サイクル（RED → GREEN → REFACTOR → VERIFY → MARK）に従ってタスクを実装する
4. When `/cupola:spec-compress` が実行された時, the spec-compress skill shall 完了済み spec の requirements/design/tasks を要約して summary.md に集約し、元ファイルを削除する
5. When `/cupola:steering` が実行された時, the steering skill shall `.cupola/steering/` 配下に product.md、tech.md、structure.md を生成または更新する
6. The spec-design skill shall cc-sdd の既存ルール（ears-format.md、design-principles.md、tasks-generation.md、tasks-parallel-analysis.md）とテンプレート（requirements.md、design.md、tasks.md、research.md）をそのまま参照する

### Requirement 2: Spec ID の決定論的命名
**Objective:** As a Cupola 利用者, I want spec id が `issue-{number}` で一意に決定されるようにしたい, so that LLM による命名の衝突リスクを排除し、issue との紐付けを明確にできる

#### Acceptance Criteria
1. When issue の初期化フェーズが実行された時, the Cupola CLI shall `.cupola/specs/issue-{number}/` ディレクトリを自動作成する
2. When issue の初期化フェーズが実行された時, the Cupola CLI shall テンプレートから spec.json を生成し、`feature_name` に `issue-{number}` を設定する
3. When issue の初期化フェーズが実行された時, the Cupola CLI shall issue 本文を `PROJECT_DESCRIPTION` として requirements.md に埋め込む
4. The domain layer shall `Issue` エンティティの `feature_name` フィールドを `Option<String>` から `String` に変更し、初期化時に `issue-{number}` を確定させる

### Requirement 3: Prompt の簡素化
**Objective:** As a Cupola 開発者, I want prompt.rs が `/cupola:spec-design issue-{number}` を1行渡すだけにしたい, so that prompt 構築ロジックが簡潔になり、skill 側に責務を委譲できる

#### Acceptance Criteria
1. When design フェーズの prompt が構築される時, the prompt builder shall `/cupola:spec-design issue-{number}` を含む簡潔な指示を生成する
2. When implementation フェーズの prompt が構築される時, the prompt builder shall `/cupola:spec-impl issue-{number}` を含む簡潔な指示を生成する
3. The prompt builder shall `/kiro:spec-init`、`/kiro:spec-requirements`、`/kiro:spec-design`、`/kiro:spec-tasks` への参照を完全に削除する
4. When fixing フェーズの prompt が構築される時, the prompt builder shall 既存の fixing prompt をそのまま維持する（skill 不要）

### Requirement 4: compress サブコマンド
**Objective:** As a Cupola 利用者, I want `cupola compress` コマンドで完了 spec を要約アーカイブしたい, so that `.cupola/specs/` の肥大化を防ぎ、コンテキストウィンドウを節約できる

#### Acceptance Criteria
1. The Cupola CLI shall `compress` サブコマンドを提供する
2. When `cupola compress` が実行された時, the Cupola CLI shall Claude Code セッションを起動し `/cupola:spec-compress` skill を実行する
3. If 完了済み spec が存在しない場合, the Cupola CLI shall その旨を表示して正常終了する

### Requirement 5: kiro skill の完全廃止
**Objective:** As a Cupola 開発者, I want `/kiro:*` skill への依存を完全に排除したい, so that Cupola が独立した skill 体系を持ち、cc-sdd への runtime 依存がなくなる

#### Acceptance Criteria
1. The Cupola project shall `.claude/commands/kiro/` ディレクトリを削除する
2. The Cupola project shall コードベース内の全ての `/kiro:*` 参照を `/cupola:*` に更新する（prompt.rs、doctor_use_case.rs、CLAUDE.md、settings.local.json）
3. The Cupola project shall `.cupola/settings/` 配下のルール・テンプレートファイルを維持する（cc-sdd 由来だが、skill とは独立したリソースとして継続利用）
