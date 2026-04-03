# リサーチ & 設計決定ログ

---
**目的**: TaskWeight × Phase モデル解決機構の調査記録と設計判断の根拠を記録する。

---

## Summary

- **Feature**: `task-weight-model-resolver`
- **Discovery Scope**: Extension（既存システムへの拡張）
- **Key Findings**:
  - `Issue.model: Option<String>` は domain/adapter/DB の全層に定義済みだが、spawn 時に `self.config.model` が直接参照されており完全に未使用
  - `Config.model: String` は `CupolaToml.model: Option<String>` から変換され bootstrap 層で解決される
  - `DoctorUseCase` の `check_model_labels()` が `model:*` ラベルをチェックしており、`weight:*` への変更が必要

## Research Log

### 既存 Issue エンティティの調査

- **Context**: `Issue.model` フィールドの現状把握
- **Findings**:
  - `src/domain/issue.rs:18` — `pub model: Option<String>`
  - `src/adapter/outbound/sqlite_issue_repository.rs` — save/update/row_to_issue で model を読み書きするが、spawn 時に使われていない
  - `src/bootstrap/app.rs:591` — Issue 生成時 `model: None` がハードコードされている
- **Implications**: `model` フィールドを `weight: TaskWeight` に置き換えれば、デッドコードが消え設計意図が明確になる

### 既存 Config 構造の調査

- **Context**: `Config.model` の現在の取り扱いと拡張ポイント
- **Findings**:
  - `src/bootstrap/config_loader.rs:17` — `CupolaToml.model: Option<String>`
  - `src/bootstrap/config_loader.rs:62` — `model: self.model.unwrap_or_else(|| "sonnet".to_string())`
  - `src/domain/config.rs:24` — `Config.model: String`（グローバルデフォルト）
- **Implications**: `CupolaToml` に `models: Option<HashMap<String, ModelTier>>` を追加し、`Config` に `ModelConfig` 構造体を追加することで既存の `model` フォールバックを維持しながら拡張できる

### State → Phase マッピング

- **Context**: どの State がどの Phase に対応するかの確認
- **Findings**（`src/domain/state.rs`）:
  - `DesignRunning` → `Phase::Design`
  - `DesignFixing` → `Phase::DesignFix`
  - `ImplementationRunning` → `Phase::Implementation`
  - `ImplementationFixing` → `Phase::ImplementationFix`
  - その他（`Idle`, `Initialized`, `*ReviewWaiting`, `Completed`, `Cancelled`）→ `None`
- **Implications**: `Phase::from_state()` は spawn が必要な4状態のみ `Some` を返す設計が妥当

### Doctor コマンドの調査

- **Context**: `check_model_labels()` の現状確認
- **Findings**:
  - `src/application/doctor_use_case.rs:39` — `check_model_labels()` 関数が存在
  - GitHub CLI を使って `model:*` ラベルを確認している
- **Implications**: `check_weight_labels()` にリネームし、`weight:light` / `weight:heavy` ラベルチェックに変更

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Domain に ModelConfig を追加 | `Config` に `ModelConfig` を埋め込み、`resolve()` メソッドを持たせる | ドメイン層が解決ロジックを所有、テスト容易 | domain に serde 依存が増える可能性 | serde derive は既存の許容済み例外 |
| Application 層で解決 | PollingUseCase が直接 TOML 値を参照して解決 | レイヤー変更なし | アプリケーション層が設定詳細に依存（反クリーンアーキテクチャ） | 採用しない |
| bootstrap のみで解決 | spawn 直前に config_loader が model を文字列として渡す | 変更箇所が少ない | domain/application が weight を知れない | 採用しない |

## Design Decisions

### Decision: ModelConfig を domain 層に配置する

- **Context**: モデル解決ロジックをどの層に置くか
- **Alternatives Considered**:
  1. domain 層 — `Config` に `ModelConfig` を埋め込み `resolve()` を持たせる
  2. bootstrap 層 — `CupolaToml` がそのまま解決する
- **Selected Approach**: domain 層に `ModelConfig` を追加し、`Config.models: ModelConfig` として持たせる
- **Rationale**: 解決ロジックはビジネスルール（4段フォールバック）であり、domain に属するべき。bootstrap は TOML パース後に `ModelConfig` へ変換するのみ
- **Trade-offs**: domain に `serde` を使う必要があるが、既存 `Config` でも許容済み
- **Follow-up**: `ModelTier` の untagged enum パースが正しく機能するか統合テストで確認

### Decision: ModelTier は untagged enum で TOML パース

- **Context**: `light = "haiku"` と `[models.heavy] design = "opus"` の両記法を同一フィールドで受け付ける
- **Alternatives Considered**:
  1. `serde(untagged)` enum — 文字列ならUniform、テーブルならPerPhase
  2. 別フィールド2つ (`light_model`, `heavy_model`) — 複雑でユーザーフレンドリーでない
- **Selected Approach**: `ModelTier` untagged enum
- **Rationale**: Issue に記載された設計通り。`serde(untagged)` は TOML crate との組み合わせで実績あり
- **Trade-offs**: エラーメッセージがやや不明瞭になる場合があるが、許容範囲

### Decision: DB の weight カラムは TEXT NOT NULL DEFAULT 'medium'

- **Context**: 既存の `model TEXT` カラムを置き換える
- **Alternatives Considered**:
  1. NULL 許容のまま `weight TEXT DEFAULT 'medium'`
  2. `NOT NULL DEFAULT 'medium'` で明示的制約
- **Selected Approach**: `NOT NULL DEFAULT 'medium'`
- **Rationale**: `TaskWeight::Medium` がデフォルト値であり、NULL を許容する意味がない。既存ユーザーはいないため破壊的変更は許容

## Risks & Mitigations

- `serde(untagged)` と `toml` crate の組み合わせで PerPhase テーブルが正しくパースされるか確認が必要 — 単体テストで全パターンをカバーする
- `weight:light` と `weight:heavy` の両ラベルが付いたエッジケース — Heavy を優先する仕様で解決
- DB 再 init が必要になるため、既存ユーザー向けのマイグレーション案内は不要（後方互換なし）

## References

- Issue #121: TaskWeight × Phase によるモデル解決機構の導入
- 既存 spec: `issue-label-model-override`（本機能で置き換え）
