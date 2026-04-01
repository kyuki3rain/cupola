# 要件定義書

## はじめに

本仕様は、Cupola プロジェクトの CI パイプライン（`.github/workflows/ci.yml`）を強化するための要件を定義する。現在の CI は単体テスト（`--lib`）のみを実行しており、`tests/` 配下の統合テストが一切実行されていない。また、依存クレートに対するセキュリティ監査が未設定であり、サプライチェーンリスクが管理されていない。本機能追加により、統合テストの CI 実行と `rustsec/audit-check` を用いたセキュリティ監査を追加する。

## 要件

### 要件 1: 統合テストの CI 実行

**目的:** CI 担当者として、`tests/` 配下の統合テストを CI で自動実行したい。なぜなら、ステートマシン遷移・セッション管理・コンカレントセッション制限などの重要なシナリオが現在の CI で検証されておらず、リグレッションリスクがあるため。

#### 受け入れ基準

1. When CI パイプラインが起動したとき、the CI shall `cargo test --tests -- --test-threads=1` を実行し、`tests/` 配下のすべての統合テストを検証する
2. The CI shall 単体テストステップ（`cargo test --lib`）と統合テストステップ（`cargo test --tests -- --test-threads=1`）を独立したステップとして分離する
3. The CI shall 統合テストステップに `--test-threads=1` フラグを付与し、SQLite への同時アクセスによるロック競合を防止する
4. If 統合テストのいずれかが失敗したとき、the CI shall ジョブ全体を失敗として報告する
5. The CI shall 統合テストを既存の `check` ジョブと同一の `ubuntu-latest` 環境で実行する

---

### 要件 2: セキュリティ監査の追加

**目的:** 開発チームとして、依存クレートの RUSTSEC アドバイザリを CI で自動チェックしたい。なぜなら、既知の脆弱性を持つクレートが本番環境に混入するリスクを継続的に管理する必要があるため。

#### 受け入れ基準

1. The CI shall `rustsec/audit-check@v2.0.0` アクションを使用した独立したジョブ `security_audit` を CI ワークフローに追加する
2. The CI shall `security_audit` ジョブを `ubuntu-latest` で実行し、`actions/checkout@v4` でコードをチェックアウトする
3. The CI shall `rustsec/audit-check` に `secrets.GITHUB_TOKEN` を渡し、GitHub Checks への結果書き込みを可能にする
4. The CI shall `security_audit` ジョブに `permissions` として `contents: read`, `issues: write`, `checks: write` を付与する（`contents: read` は `actions/checkout` に必要な最小権限）
5. If `Cargo.lock` がリポジトリにコミットされているとき、the CI shall その `Cargo.lock` を基にアドバイザリチェックを実行する
6. Where fork からの PR である場合、the CI shall GitHub Checks への書き込みに失敗しても stdout 出力にフォールバックし、ジョブ自体は継続する

---

### 要件 3: 既存 CI チェックの継続動作

**目的:** 開発チームとして、今回の変更後も既存の品質チェックが引き続き正常に動作してほしい。なぜなら、フォーマットチェック・Clippy・単体テストは現在機能しており、退行させてはならないため。

#### 受け入れ基準

1. The CI shall `cargo fmt -- --check` によるフォーマットチェックを引き続き実行する
2. The CI shall `cargo clippy --all-targets` による静的解析を引き続き実行する
3. The CI shall `cargo test --lib -- --test-threads=1` による単体テストを引き続き実行する
4. When CI の全ステップが成功したとき、the CI shall パイプライン全体を成功として報告する
5. The CI shall 既存の `CARGO_TERM_COLOR: always` および `RUSTFLAGS: "-D warnings"` 環境変数設定を維持する
