# リサーチ: doctor の再設計

## サマリー

- **Feature**: doctor の再設計: start readiness 基準 + remediation 表示
- **Discovery Scope**: Extension（既存 `doctor_use_case.rs` の再設計）
- **Key Findings**:
  - 現行の `DoctorCheckResult` は `section` と `remediation` を持たない。追加が必要
  - `ConfigLoader` ポートは parse のみで `into_config + validate` まで行っていない。エラー型の拡張が必要
  - `gh_token::get()` は `bootstrap/app.rs` のみで使われており、doctor からは呼ばれていない。`gh auth token` コマンドで代替するのが最小変更
  - `cupola init` が導入するアセットは `init_file_generator.rs` で確認済み（`.claude/commands/cupola/` と `.cupola/settings/` 配下のファイル）
  - CLI 表示ロジックは `bootstrap/app.rs` の `Command::Doctor` ブランチにあり、section + remediation 対応が必要

## リサーチログ

### 現行 doctor_use_case.rs の分析

- **Context**: 既存チェック項目の severity と構造を把握するため
- **Findings**:
  - 7 チェック: `check_toml`, `check_git`, `check_gh`, `check_gh_label`, `check_weight_labels`, `check_steering`, `check_db`
  - `check_steering` は空ディレクトリで FAIL を返す → 仕様では WARN に変更
  - `check_gh_label` (agent:ready) は FAIL を返す → 仕様では WARN に変更
  - `check_weight_labels` はすでに WARN を返している（変更不要）
  - `check_toml` は `ConfigLoader::load()` のみで validate 未実施
- **Implications**: `DoctorCheckResult` に `section` と `remediation` フィールドを追加し、全 check 関数のシグネチャを更新する

### ConfigLoader ポートの分析

- **Context**: config validate チェックを実装するために必要な変更を把握する
- **Findings**:
  - `ConfigLoader::load()` は `DoctorConfigSummary` を返す（parse のみ）
  - `ConfigLoadError` に `ValidationFailed` バリアントが存在しない
  - `Config::validate()` は `Result<(), String>` を返す（`src/domain/config.rs`）
  - `CupolaToml::into_config()` は `ConfigError` を返す
- **Implications**: `ConfigLoadError` に `ValidationFailed { reason: String }` バリアントを追加し、`TomlConfigLoader` の実装を `into_config + validate` まで実行するよう変更する

### gh_token::get() の代替手段

- **Context**: doctor から GitHub token readiness を確認する最小限の方法を探す
- **Findings**:
  - `gh_token::get()` は bootstrap 内部依存であり、application layer から直接呼ぶのはアーキテクチャ違反
  - `gh auth token` コマンドを実行し、成功かどうかで判定するのが最小変更かつクリーン
  - `gh auth status` とは異なり、`gh auth token` は実際にトークンを取得する
- **Implications**: `CommandRunner` ポートを使用して `gh auth token` を実行する `check_github_token` 関数を追加する

### claude CLI 確認方法

- **Context**: claude CLI の存在と実行可能性を確認する最小限の方法を決定する
- **Findings**:
  - `claude --version` は副作用なしで実行できる
  - `which claude` は POSIX 標準だが Windows 非対応のため不採用
  - `claude --version` の成功を存在確認として扱うのが適切
- **Implications**: `CommandRunner` で `claude --version` を実行し、成功可否で判定する

### init アセットの確認範囲

- **Context**: assets チェックでどのパスを確認すべきか特定する
- **Findings**:
  - `init_file_generator.rs` より、`init` が生成するアセット:
    - `.claude/commands/cupola/spec-compress.md`
    - `.claude/commands/cupola/spec-design.md`
    - `.claude/commands/cupola/spec-impl.md`
    - `.claude/commands/cupola/steering.md`
    - `.cupola/settings/rules/` 配下の複数ファイル
    - `.cupola/settings/templates/` 配下の複数ファイル
  - 個別ファイルではなくディレクトリの存在で確認するのが保守性が高い
  - 確認ポイント: `.claude/commands/cupola/` ディレクトリと `.cupola/settings/` ディレクトリ
- **Implications**: 2つのディレクトリパスの存在確認を `check_assets` 関数として実装する

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク | 結論 |
|--------|-------------|-----------|---------------------|-------|
| DoctorCheckResult にフィールド追加 | 既存構造体に `section` と `remediation` を追加 | 最小変更、後方互換 | 全 check 関数の更新が必要 | 採用 |
| DoctorResult を section ごとに分離 | `StartReadinessResult` と `OperationalReadinessResult` を別々に返す | 型安全 | 変更範囲が大きい | 不採用（過設計） |

## 設計上の決定

### Decision: Remediation の表現方法

- **Context**: 各チェック結果にどのような形で remediation を持たせるか
- **Alternatives Considered**:
  1. `Remediation` enum (`RunInit`, `ManualAction`, `Either`, `None`) + description String
  2. 単純な `Option<String>`
- **Selected Approach**: `Option<String>` として remediation メッセージを持たせる。コマンドや手順を含むテキストとして表現する
- **Rationale**: enum + description では表示側でマッピングが必要になり冗長。文字列で直接表現する方が柔軟で実装が簡潔
- **Trade-offs**: 型安全性は低いが、doctorの出力は人間向けテキストなので実用上の問題はない

### Decision: ConfigLoader ポートの拡張

- **Context**: validate まで実行するために ConfigLoadError にバリアントを追加する
- **Alternatives Considered**:
  1. `ConfigLoadError::ValidationFailed { reason: String }` を追加
  2. doctor use case 内で直接 `load_toml + into_config + validate` を呼ぶ
- **Selected Approach**: `ConfigLoadError::ValidationFailed` を追加し、`TomlConfigLoader` で validate まで実行する
- **Rationale**: application layer がブートストラップ具象型に依存しないようにするため。ポートの責務を「有効な設定が得られるか」にまで広げる

## リスクと軽減策

- 既存テスト数が多く、全テストの section 対応更新に漏れが出る可能性 → テスト更新は task に独立させ、チェックリストで確認する
- `gh auth token` コマンドの挙動が環境依存の可能性 → `MockCommandRunner` でユニットテストをカバーする

## 参考資料

- `.local/doctor-start-readiness-redesign.md` — 設計メモ（remediation taxonomy, check 分類案）
- `src/application/doctor_use_case.rs` — 現行実装
- `src/adapter/outbound/init_file_generator.rs` — init が生成するアセット一覧
- `src/bootstrap/app.rs` — `gh_token::get()` 使用箇所、doctor 表示ロジック
