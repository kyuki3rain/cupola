# Research & Design Decisions

---
**Purpose**: 設計調査の記録、アーキテクチャ検討、設計根拠の保存

---

## Summary
- **Feature**: `ci-integration-test-and-security-audit`
- **Discovery Scope**: Simple Addition（既存 CI ワークフローへの拡張）
- **Key Findings**:
  - 統合テストは `tests/integration_test.rs`（17テスト、1137行）に存在し、`--lib` では実行されない
  - `rustsec/audit-check@v2.0.0` が `actions-rs/audit-check`（アーカイブ済み）の後継として推奨される
  - 既存の `release.yml` ではアクションをコミット SHA でピン留めする慣例が採用されており、`ci.yml` にも適用すべき

## Research Log

### 統合テストの実行状況

- **Context**: `cargo test --lib` は `src/` 内の `#[cfg(test)]` ブロックのみを実行し、`tests/` 配下は対象外
- **Sources Consulted**: cargo テストドキュメント、既存コード `tests/integration_test.rs`
- **Findings**:
  - `tests/integration_test.rs` は SQLite を使用（`SqliteConnection`, `SqliteIssueRepository`）
  - Mock アダプタを注入してユースケースを E2E 検証する設計
  - 並列実行時に SQLite ロック競合が発生するため `--test-threads=1` が必要
- **Implications**: `cargo test --test '*' -- --test-threads=1` で全統合テストを実行できる

### rustsec/audit-check の選定

- **Context**: `actions-rs/audit-check` はアーカイブ済みで使用不可
- **Sources Consulted**: Issue 本文、GitHub rustsec/audit-check リポジトリ
- **Findings**:
  - `rustsec/audit-check@v2.0.0` が公式後継
  - `issues: write` と `checks: write` パーミッションが必要
  - fork PR では GitHub Checks への書き込みが制限され、stdout にフォールバックする
  - 将来的な上位互換候補として `cargo-deny` がある（ライセンス・ban・ソースチェックも統合可能）
- **Implications**: 現時点では `rustsec/audit-check` が最もシンプルな選択肢

### アクション SHA ピン留め慣例

- **Context**: `release.yml` ではすべてのアクションをコミット SHA でピン留めしている
- **Sources Consulted**: `.github/workflows/release.yml`
- **Findings**:
  - 既存ワークフローは `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4` の形式を採用
  - サプライチェーンセキュリティのためのベストプラクティス
- **Implications**: 新規追加アクションも SHA ピン留めが望ましいが、`rustsec/audit-check@v2.0.0` のピン SHA は設計段階では未確定（実装時に確認）

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 既存 `check` ジョブに統合テストを追加 | 同一ジョブに統合テストステップを追加 | シンプル、キャッシュ共有 | ジョブが長くなる | 採用 |
| 統合テスト専用ジョブ | 別ジョブとして分離 | 並列実行可能 | キャッシュの重複、複雑化 | 不採用（単純さを優先） |
| `security_audit` 別ジョブ | セキュリティ監査を独立ジョブ化 | 独立パーミッション設定が可能 | ジョブ数増加 | 採用（パーミッション分離が必要） |

## Design Decisions

### Decision: 統合テストを `check` ジョブ内の独立ステップとして追加

- **Context**: 統合テストをどこで実行するか
- **Alternatives Considered**:
  1. 既存の `Test` ステップを拡張 — 単一ステップに両方を含める
  2. 同一ジョブ内に独立ステップとして追加 — ステップを分離
  3. 専用ジョブとして分離 — 並列実行
- **Selected Approach**: 既存 `check` ジョブ内に `Unit tests` と `Integration tests` を独立ステップとして追加
- **Rationale**: キャッシュ（`Swatinem/rust-cache`）を再利用でき、シンプルさを維持できる。統合テストが失敗した場合の視認性も向上する
- **Trade-offs**: ジョブが直列化されるが、総実行時間への影響は許容範囲内
- **Follow-up**: 統合テストの実行時間を計測し、必要であれば後でジョブ分離を検討

### Decision: security_audit を独立ジョブとして追加

- **Context**: `rustsec/audit-check` は `issues: write` と `checks: write` パーミッションが必要
- **Alternatives Considered**:
  1. `check` ジョブに統合 — パーミッション設定が全体に影響
  2. 独立ジョブ — パーミッション分離が可能
- **Selected Approach**: `security_audit` として独立ジョブを追加
- **Rationale**: パーミッションスコープを最小化するため、セキュリティ監査専用のパーミッションを持つジョブを分離する
- **Trade-offs**: ジョブ数が増加するが、最小権限の原則に従う
- **Follow-up**: 将来的に `cargo-deny` への移行時に同じ構造を維持できる

### Decision: security_audit の `continue-on-error` 設定

- **Context**: 新しい RUSTSEC アドバイザリがコード変更と無関係に CI を壊す可能性がある
- **Alternatives Considered**:
  1. `continue-on-error: true` — 監査失敗時も PR をブロックしない
  2. daily cron で別途実行 — PR CI とは分離
  3. デフォルト（fail-fast）— 脆弱性発見時に CI を失敗させる
- **Selected Approach**: デフォルト（fail-fast）採用。`continue-on-error` は設定しない
- **Rationale**: 要件 2.1〜2.6 は脆弱性検出時の明示的な失敗を前提としている。緩和策は必要に応じて後続 Issue で対応
- **Trade-offs**: アドバイザリ追加で突発的な CI 失敗が起きる可能性あり。将来的には cron 分離を検討

## Risks & Mitigations

- **新規アドバイザリによる突発的 CI 失敗** — 将来的に `continue-on-error: true` または daily cron での分離を検討。現時点では要件定義の範囲外
- **fork PR での `checks: write` 権限不足** — `rustsec/audit-check` は stdout フォールバックを持つため、ジョブ自体は継続する（要件 2.6 で確認済み）
- **統合テストの実行時間増大** — `--test-threads=1` により直列化されるが、現時点ではロック競合回避が優先

## References

- rustsec/audit-check GitHub Actions — セキュリティ監査アクション
- Cargo テストドキュメント（`--test` フラグ）
- 既存 `.github/workflows/release.yml` — SHA ピン留め慣例の参照元
