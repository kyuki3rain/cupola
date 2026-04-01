# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-01

### Added

- GitHub Issue 検知と cc-sdd による設計自動生成
- 設計/実装 PR の自動作成
- Review thread への自動修正・返信・resolve
- CI 失敗・conflict の自動検知と修正
- 同時実行数制限（max_concurrent_sessions）
- モデル指定（cupola.toml + Issue ラベル）
- `cupola doctor` / `cupola init` コマンド
- Graceful shutdown

[0.1.0]: https://github.com/kyuki3rain/cupola/releases/tag/v0.1.0
