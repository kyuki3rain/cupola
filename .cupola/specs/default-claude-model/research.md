# Research & Design Decisions

## Summary
- **Feature**: `default-claude-model`
- **Discovery Scope**: Extension（既存システムへの設定フィールド追加）
- **Key Findings**:
  - Claude Code CLI は `--model` フラグでモデル指定が可能（例: `claude -p "prompt" --model sonnet`）
  - 既存の Config / CupolaToml / ClaudeCodeProcess は既に同様のオプショナル設定パターン（`max_concurrent_sessions` 等）を持ち、同じパターンで拡張可能
  - ClaudeCodeRunner trait の spawn シグネチャ変更により、全呼び出し元（PollingUseCase）の更新が必要

## Research Log

### Claude Code CLI の --model フラグ
- **Context**: cupola.toml の model 値を Claude Code プロセスに渡す方法の調査
- **Sources Consulted**: Issue #37 の技術的コンテキスト
- **Findings**:
  - `claude -p "prompt" --model sonnet` の形式で指定可能
  - モデル名は短縮形（sonnet, opus, haiku 等）で指定する
- **Implications**: `build_command` メソッドに `--model` 引数を追加するだけで実現可能

### 既存設定パターンの分析
- **Context**: 既存コードベースの設定追加パターンを把握し、一貫性のある実装方針を決定する
- **Sources Consulted**: `src/domain/config.rs`, `src/bootstrap/config_loader.rs`
- **Findings**:
  - CupolaToml では `Option<T>` で省略可能なフィールドを定義し、`into_config` でデフォルト値を適用する既存パターンがある
  - Config は非 Option の確定値を保持するパターン（例: `language: String`, `polling_interval_secs: u64`）
  - `max_concurrent_sessions` は `Option<u32>` だが、model は常にデフォルト値があるため `String` が適切
- **Implications**: `model` は CupolaToml では `Option<String>`、Config では `String`（デフォルト "sonnet"）とする

### ClaudeCodeRunner trait への影響
- **Context**: trait シグネチャ変更の影響範囲を特定する
- **Sources Consulted**: `src/application/port/claude_code_runner.rs`, spawn の呼び出し箇所
- **Findings**:
  - ClaudeCodeRunner trait は `spawn(&self, prompt, working_dir, json_schema)` を定義
  - 呼び出し元は `PollingUseCase::run` 内の1箇所のみ（polling_use_case.rs:527）
  - ClaudeCodeProcess が唯一の実装（テスト内の mock を除く）
  - PollingUseCase は既に `config: Config` を保持しており、`config.model` を参照可能
- **Implications**: trait に `model: &str` を追加するか、アダプター側で保持するかの選択が必要

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: trait に model 引数追加 | spawn メソッドに `model: &str` を追加 | 呼び出し元が明示的にモデルを制御可能、テストで異なるモデルを渡せる | trait シグネチャ変更により全実装・呼び出し元を更新 | 要件に合致 |
| B: アダプターに model を保持 | ClaudeCodeProcess に model フィールドを追加 | trait 変更不要、シンプル | PollingUseCase からモデル制御不可、将来の柔軟性低下 | 要件 3 に不適合 |

## Design Decisions

### Decision: trait に model 引数を追加する
- **Context**: model パラメータをどの層で管理するか
- **Alternatives Considered**:
  1. Option A — ClaudeCodeRunner trait の spawn に `model: &str` 引数を追加
  2. Option B — ClaudeCodeProcess のコンストラクタで model を受け取り内部保持
- **Selected Approach**: Option A — trait に引数を追加
- **Rationale**: 要件 3 に明示的に「trait の spawn メソッドに model パラメータを含む」と記載。アプリケーション層からモデルを制御可能にすることで、将来的なセッション単位のモデル変更にも対応可能
- **Trade-offs**: trait 変更によりコンパイル時に全実装の更新が強制される（Rust の型安全性が保証）
- **Follow-up**: 統合テスト内の mock 実装も更新が必要

## Risks & Mitigations
- model に無効な値が指定されるリスク — Claude Code CLI 側でバリデーションされるため、cupola 側では文字列をそのまま渡す（過度なバリデーションを避ける）
- trait 変更による既存テスト破損 — コンパイラが未更新箇所を検出するため、見落としリスクは低い
