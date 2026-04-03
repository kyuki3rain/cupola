# 実装タスク一覧

## タスク概要

- **対象機能**: `task-weight-model-resolver`
- **合計**: 6 メジャータスク、15 サブタスク
- **要件カバレッジ**: 要件 1〜7 の全項目

---

- [x] 1. ドメイン型（TaskWeight / Phase / ModelConfig）の実装

- [x] 1.1 (P) TaskWeight enum を実装する
  - `Light` / `Medium`（デフォルト）/ `Heavy` の 3 バリアントを持つ enum を domain 層に追加する
  - `Default`、`Copy`、`Clone`、`PartialEq`、`Eq` を実装する
  - `serde` の `Serialize` / `Deserialize` を `rename_all = "lowercase"` で導出し、`"light"` / `"medium"` / `"heavy"` の文字列で往復できることを確認する
  - _Requirements: 1.1, 3.2_

- [x] 1.2 (P) Phase enum と State からの変換を実装する
  - `Design` / `DesignFix` / `Implementation` / `ImplementationFix` の 4 バリアントを持つ enum を domain 層に追加する
  - `Phase::from_state(state: State) -> Option<Phase>` を実装する（DesignRunning → Design、DesignFixing → DesignFix、ImplementationRunning → Implementation、ImplementationFixing → ImplementationFix、それ以外 → None）
  - `Phase::base(&self) -> Option<Phase>` を実装する（DesignFix → Some(Design)、ImplementationFix → Some(Implementation)、それ以外 → None）
  - _Requirements: 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_

- [x] 1.3 ModelConfig と resolve ロジックを実装する
  - `PerPhaseModels`（design / design_fix / implementation / implementation_fix の各 Option フィールド）を定義する
  - `WeightModelConfig`（Uniform(String) / PerPhase(PerPhaseModels)）を定義する
  - `ModelConfig`（default_model: String、light / medium / heavy: Option<WeightModelConfig>）を定義する
  - `ModelConfig::resolve(weight: TaskWeight, phase: Option<Phase>) -> &str` を 4 段フォールバックで実装する（exact_phase → base_phase → Uniform → global default の順）
  - phase が None の場合はグローバルデフォルト（`default_model`）を返す
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

- [x] 1.4 Issue エンティティの model フィールドを weight に置き換える
  - `Issue.model: Option<String>` を削除し `weight: TaskWeight` を追加する（1.1 が完了していること）
  - `weight` のデフォルト値は `TaskWeight::Medium` とする
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 2. Config の ModelConfig 埋め込みと TOML パース拡張

- [x] 2.1 Config に models フィールドを追加する
  - `Config` 構造体に `models: ModelConfig` フィールドを追加する（1.3 が完了していること）
  - `Config::default_with_repo` で `ModelConfig` のデフォルト（default_model = "sonnet"、weight 別設定なし）を設定する
  - `Config::validate()` で `default_model` が空文字列でないことを検証するロジックを追加する
  - _Requirements: 2.1, 2.5, 2.6_

- [x] 2.2 CupolaToml に TOML パース用型を追加して Config に変換する
  - `ModelTier`（untagged enum: Uniform(String) / PerPhase { design, design_fix, implementation, implementation_fix }）を定義する
  - `ModelsToml`（light / medium / heavy: Option<ModelTier>）を bootstrap/config_loader に定義する
  - `CupolaToml` に `models: Option<ModelsToml>` フィールドを追加する
  - `CupolaToml::into_config()` 内で `ModelsToml` → `ModelConfig` への変換ロジックを実装する
  - _Requirements: 2.7_

- [x] 3. SQLite アダプターの weight 対応

- [x] 3.1 DB スキーマを weight カラムに変更する
  - `init` サブコマンドが生成する SQLite スキーマの issues テーブルで `model TEXT` を `weight TEXT NOT NULL DEFAULT 'medium'` に置き換える
  - _Requirements: 4.1_

- [x] 3.2 SqliteIssueRepository の weight 読み書きを実装する
  - `task_weight_to_str` / `str_to_task_weight` のシリアライズ補助関数を実装する（3.1 が完了していること）
  - `save()` / `update()` クエリの `model` カラム参照を `weight` に変更する
  - `row_to_issue()` で `weight` カラムを `TaskWeight` に変換する処理を実装する
  - 不明な weight 文字列は `Err` を返す
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 4. ポーリングの label 検出とモデル解決の変更

- [x] 4.1 Step1 での weight ラベル検出を実装する
  - `label_to_weight(labels: &[String]) -> TaskWeight` ヘルパーを実装する（`weight:heavy` 優先、`weight:light` 次、どちらもなければ `Medium`）
  - 検出した Issue の `weight` フィールドに変換結果を設定して DB に保存する
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [x] 4.2 Step7 のモデル解決を config.models.resolve() 経由に変更する
  - `self.config.model` の直参照を `self.config.models.resolve(issue.weight, Phase::from_state(issue.state))` に置き換える（4.1 が完了していること）
  - `Phase::from_state()` が `None` を返す場合はグローバルデフォルトにフォールバックされることを確認する
  - _Requirements: 6.1, 6.2, 6.3, 6.4_

- [x] 5. init テンプレートと doctor コマンドの更新

- [x] 5.1 (P) cupola.toml テンプレートに models セクションの凡例を追加する
  - `InitFileGenerator` の `CUPOLA_TOML_TEMPLATE` に `[models]` セクションのコメント付き記法例（weight 別・phase 別）を追加する
  - _Requirements: 7.1_

- [x] 5.2 (P) DoctorUseCase の weight ラベルチェックを実装する
  - `check_model_labels()` を `check_weight_labels()` にリネームし、`weight:light` / `weight:heavy` ラベルの存在チェックに変更する
  - いずれかが欠落している場合は `CheckStatus::Warn` を返す
  - _Requirements: 7.2, 7.3_

- [x] 6. テスト

- [x] 6.1 (P) ドメイン型のユニットテストを実装する
  - `TaskWeight` の serde 往復テスト（全バリアント）
  - `Phase::from_state()` の全 State パターンテスト
  - `Phase::base()` の全 Phase パターンテスト
  - `ModelConfig::resolve()` の全 weight × phase 組み合わせテスト（解決例マトリクスを網羅）
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 2.3, 2.4, 2.5_

- [x] 6.2 (P) Config TOML パースと変換の統合テストを実装する
  - `model` 1 行のみの最小構成が正常にパースされることを確認する
  - `[models]` セクションの Uniform / PerPhase 両パターンのパーステスト
  - フォールバックチェーンが仕様通りに動作することを確認するテスト
  - `label_to_weight()` の各ラベル組み合わせテスト（both / light only / heavy only / none）
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 5.1, 5.2, 5.3, 5.4_

- [x] 6.3 (P) SQLite の weight 読み書き統合テストを実装する
  - インメモリ DB を使用して Issue の save → load の weight 往復テストを実装する
  - 不明な weight 文字列が Err を返すことを確認する
  - _Requirements: 4.1, 4.2, 4.3, 4.4_
