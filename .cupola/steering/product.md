# Product Overview

GitHub Issue / PR を唯一の操作面とし、Claude Code + cc-sdd を駆動して設計・実装を自動化するローカル常駐エージェント。人間は Issue 作成・ラベル付与・PR レビューのみを行い、設計ドキュメント生成から実装、レビュー対応、完了 cleanup までを自動化する。

## Core Capabilities

- **Issue 検知 → 設計自動生成**: `agent:ready` ラベル付き Issue を polling で検知し、cc-sdd による requirements / design / tasks を自動生成
- **PR ベースのレビューフロー**: 設計 PR と実装 PR を自動作成し、review thread への修正・返信・resolve を自動化
- **ステートマシン駆動**: 10 状態のステートマシンで全工程を管理し、冪等な再実行と graceful shutdown をサポート
- **責務分離**: GitHub API 操作は全て Cupola が担当、Claude Code は git（commit/push）のみ

## Target Use Cases

- 個人・小規模チームでの反復的な機能開発の自動化
- Issue を起点とした設計→実装のワンストップ自動化
- レビュー指摘への自動修正・返信サイクル

## Value Proposition

GitHub の既存ワークフロー（Issue + PR + review）をそのまま活用し、専用 UI なしで設計・実装を自動化する。人間のレビュー承認を唯一のゲートとし、品質担保と自動化を両立する。
