# Implementation Plan

- [x] 1. `check` ジョブへの統合テスト対応
- [x] 1.1 既存の `Test` ステップを `Unit tests` にリネームする
  - `.github/workflows/ci.yml` の `Test` ステップの `name` を `Unit tests` に変更する
  - コマンド `cargo test --lib -- --test-threads=1` はそのまま維持する
  - `CARGO_TERM_COLOR: always` および `RUSTFLAGS: "-D warnings"` 環境変数が引き続き有効であることを確認する
  - _Requirements: 1.2, 3.1, 3.2, 3.3, 3.5_

- [x] 1.2 `Integration tests` ステップを `Unit tests` の直後に追加する
  - ステップ名を `Integration tests` とする
  - コマンドは `cargo test --tests -- --test-threads=1` とする
  - `--tests` フラグで `tests/` 配下のすべての統合テストを対象とし、`--test-threads=1` フラグにより SQLite への同時アクセスによるロック競合を防止する
  - このステップは既存の `check` ジョブ内に配置し、`ubuntu-latest` 環境で実行する
  - ステップが失敗した場合、ジョブ全体を失敗として報告するデフォルト動作を維持する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 3.4_

- [x] 2. `security_audit` ジョブを CI ワークフローに追加する
  - `.github/workflows/ci.yml` に `check` ジョブと並列して実行される新しいジョブ `security_audit` を定義する
  - `name: Security Audit`、`runs-on: ubuntu-latest` を設定する
  - ジョブスコープに `permissions: contents: read`, `issues: write`, `checks: write` を付与し、最小権限の原則を遵守する（`contents: read` は `actions/checkout` に必要）
  - ステップ 1: `actions/checkout` を既存 CI と同様にコミット SHA でピン留めしてコードをチェックアウトする（例: `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4`）
  - ステップ 2: `rustsec/audit-check` もコミット SHA でピン留めして RUSTSEC アドバイザリをチェックする。`with: token: ${{ secrets.GITHUB_TOKEN }}` を渡す（使用する SHA は実装時に確認）
  - `Cargo.lock` がリポジトリにコミット済みであるため、`rustsec/audit-check` はそれを基にアドバイザリを検索する
  - fork PR では `checks: write` 権限が制限されるが、`rustsec/audit-check` が stdout にフォールバックするため、ジョブ自体は継続する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_
