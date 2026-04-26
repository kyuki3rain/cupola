# ci-integration-test-and-security-audit

## Feature
`.github/workflows/ci.yml` に (1) `tests/` 配下の統合テスト（17 本）の実行と (2) `rustsec/audit-check` によるサプライチェーンセキュリティ監査を追加する。既存の format / clippy / unit test 動作は維持。Rust ソース変更なし、CI YAML のみ変更。

## 要件サマリ
- `check` ジョブに `Unit tests`（`cargo test --lib -- --test-threads=1`）と `Integration tests`（`cargo test --tests -- --test-threads=1`）を独立ステップとして分離。失敗時はジョブ全体を失敗扱い。`ubuntu-latest` で実行。
- `security_audit` 独立ジョブを追加し `rustsec/audit-check@v2.0.0` を実行。`actions/checkout@v4` → audit-check の順で動作。`token: ${{ secrets.GITHUB_TOKEN }}` を渡す。
- 権限は `contents: read` / `issues: write` / `checks: write` のみ（最小権限）。`contents: read` は checkout に必要。
- `Cargo.lock` は既にコミット済みで、それを基準にアドバイザリを検索。
- fork PR では `checks: write` が制限されるが audit-check の stdout フォールバックでジョブ自体は継続。
- 既存環境変数 `CARGO_TERM_COLOR: always` / `RUSTFLAGS: "-D warnings"` を維持。

## アーキテクチャ決定
- **統合テスト配置**: 専用ジョブではなく既存 `check` ジョブの独立ステップとして追加。理由: `Swatinem/rust-cache` を再利用でき構成が単純、失敗ステップの視認性も向上。直列化による時間増は許容範囲。実行時間が問題になれば将来ジョブ分離を検討。
- **`--test-threads=1` 必須**: 統合テストは `SqliteConnection` を使い並列実行で SQLite ロック競合が発生するため。並列化は不採用。
- **security_audit をジョブ分離**: `check` に統合すると `issues: write` / `checks: write` が全ステップに波及するため最小権限原則に反する。ジョブ分離で権限スコープを限定。
- **audit-check の fail-fast**: `continue-on-error: true` や daily cron 分離は不採用。要件は脆弱性検出時の明示的失敗を前提。突発的 CI 失敗のリスクは認識しつつ、緩和策は後続 Issue 対応。
- **`rustsec/audit-check@v2.0.0`**: `actions-rs/audit-check` はアーカイブ済のため後継を採用。将来的な上位互換候補として `cargo-deny`（ライセンス・ban・source チェック統合）が存在するが、現時点では最小構成を優先。
- **SHA ピン留め**: 既存 `release.yml` の慣例に従い、新規追加アクションもコミット SHA でピン留めする方針（実装時に SHA 確認）。
- **ステップ名リネーム**: 既存 `Test` を `Unit tests` にリネームして `Integration tests` と対比させる。コマンドは維持して後方互換。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `Unit tests` step | CI / check job | 既存 `cargo test --lib` の継続（名称のみ変更） |
| `Integration tests` step | CI / check job | `cargo test --tests -- --test-threads=1` を追加 |
| `security_audit` job | CI | `rustsec/audit-check` による RUSTSEC チェック |

## 主要インターフェース
- Integration tests コマンド: `cargo test --tests -- --test-threads=1`
- Security audit step: `uses: rustsec/audit-check@<sha> # v2.0.0`、`with: token: ${{ secrets.GITHUB_TOKEN }}`
- security_audit ジョブ権限:
  ```yaml
  permissions:
    contents: read
    issues: write
    checks: write
  ```
- トリガー: 既存 `on: pull_request / push` を継承。

## 学び / トレードオフ
- `cargo test --lib` は `tests/` 配下を対象にしないため、単体テストだけ通っていた状態では重要な E2E シナリオ（state machine 遷移、セッション管理、コンカレント制限）がカバーされていなかった。
- SQLite 統合テストは並列化を諦め直列実行。テスト数増加で CI 時間がボトルネックになる可能性があり、将来は tempfile 化した DB 毎プロセス分離や専用ジョブ分離で緩和する余地あり。
- RUSTSEC アドバイザリ DB は外部要因で更新されるため、コード変更なしに CI が赤くなるリスクを抱える。緩和策（cron 分離 / continue-on-error）は明示的に後続課題とした。
- fork PR での `checks: write` 制限は audit-check の stdout フォールバックで救済される点を前提としており、将来 action 側が挙動変更した場合は再評価が必要。
- `cargo-deny` 移行は将来 Issue で対応。現状はジョブ構造（独立ジョブ + 最小権限）を維持したまま action 差し替えが可能。
