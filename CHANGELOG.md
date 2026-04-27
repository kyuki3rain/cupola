# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- DB マイグレーション回帰テスト (`tests/migrations/`) を追加 ([#327])
  - `SqliteConnection::dump_schema()` テスト用ヘルパーを追加 (`#[cfg(test)]`)
  - `v0001-initial` fixture と `current-schema.sql` スナップショットを追加
  - `migrate_v0001_is_idempotent` / `fixture_reaches_current_schema` テストを追加

### Note
- スキーマを変更する PR では `tests/migrations/fixtures/` に新規 fixture を追加すること
  (`docs/tests/migration-test.md` の section 3 / 5 を参照)

## [0.1.0] - 2026-04-01

### Added

- Automated design generation from GitHub Issues using bundled Cupola skills
- Automatic creation of design/implementation PRs
- Automated fixes, replies, and resolution for review threads
- Automated detection and fixing of CI failures and conflicts
- Concurrent session limit (`max_concurrent_sessions`)
- Model selection (cupola.toml + Issue labels)
- `cupola start --daemon`: Option to run as a background daemon
- `cupola stop`: Subcommand to stop a running daemon
- `cupola doctor` / `cupola init` commands
- Graceful shutdown

[0.1.0]: https://github.com/kyuki3rain/cupola/releases/tag/v0.1.0
