# Research & Design Decisions

---
**Purpose**: gh-token-integration フィーチャーに関する調査記録と設計判断の根拠を記録する。

---

## Summary
- **Feature**: `gh-token-integration`
- **Discovery Scope**: Extension（既存ロジックの外部クレートへの置き換え）
- **Key Findings**:
  - `resolve_github_token()` は `src/adapter/outbound/github_rest_client.rs:142-172` に実装されており、2段階フォールバック（`GITHUB_TOKEN` 環境変数 → `gh auth token` CLI）のみをサポートしている
  - `gh-token` クレート (v0.1, dtolnay作) は `anyhow::Result<String>` を返し、既存の `anyhow` ベースエラーハンドリングと直接互換性がある
  - 影響範囲は `github_rest_client.rs`（関数削除）と `bootstrap/app.rs`（呼び出し変更）の2ファイルのみ

## Research Log

### gh-token クレートの API と互換性

- **Context**: `gh_token::get()` のシグネチャと戻り値型の確認
- **Sources Consulted**: crates.io gh-token v0.1, dtolnay の実装（141行・1ファイル）
- **Findings**:
  - `gh_token::get() -> anyhow::Result<String>` を返す
  - フォールバック順序: `GH_TOKEN` → `GITHUB_TOKEN` → `~/.config/gh/hosts.yml` → `gh auth token`
  - 依存クレート: `serde_yaml` 0.9（archived だが機能的には安定）
  - ライセンス: MIT/Apache-2.0（cupola の Apache-2.0 と互換）
- **Implications**: 戻り値型が `anyhow::Result<String>` であるため、既存の `resolve_github_token()` と同一シグネチャ。`?` 演算子でそのまま伝搬可能。

### 既存コードの影響範囲

- **Context**: どのファイルが `resolve_github_token()` を使用しているかを確認
- **Sources Consulted**: `src/` ディレクトリのコード検索
- **Findings**:
  - `src/adapter/outbound/github_rest_client.rs:142-172` — 実装本体
  - `src/bootstrap/app.rs:59` — 呼び出し元（`let token = resolve_github_token()?;`）
  - `src/adapter/outbound/github_rest_client.rs:187` — テスト `resolve_token_from_env` が `resolve_github_token()` を直接テスト
- **Implications**: 変更後、テストは `gh_token::get()` の動作を間接的にカバーするため、既存テストは削除または置き換えが必要

### serde_yaml 0.9 の互換性

- **Context**: `gh-token` が依存する `serde_yaml` 0.9 は archived だが、cupola の既存依存と競合しないかを確認
- **Sources Consulted**: `Cargo.toml` の依存リスト
- **Findings**:
  - cupola の `Cargo.toml` には `serde_yaml` は含まれていない
  - `serde_yaml` 0.9 は archived（メンテナンス停止）だが、機能的に安定しており `gh-token` 用途では問題なし
  - dtolnay 自身が `gh-token` で使用しており、実績あり
- **Implications**: 依存追加に際して競合リスクなし

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 直接置き換え | `resolve_github_token()` を削除し、呼び出し元を `gh_token::get()` に変更 | 最小変更・シンプル | なし | 推奨アプローチ |
| ラッパー関数維持 | `resolve_github_token()` のシグネチャを維持しつつ内部を `gh_token::get()` に変更 | テスト変更不要 | 不必要な間接層を追加 | 不採用 |

## Design Decisions

### Decision: `resolve_github_token()` を完全削除して直接呼び出しに変更

- **Context**: `resolve_github_token()` と `gh_token::get()` が同一の `anyhow::Result<String>` を返すため、ラッパーを維持する理由がない
- **Alternatives Considered**:
  1. ラッパー関数として `resolve_github_token()` を維持し、内部実装のみ変更する
  2. `resolve_github_token()` を完全削除し、`bootstrap/app.rs` で `gh_token::get()` を直接呼び出す
- **Selected Approach**: Option 2（完全削除）
- **Rationale**: 本フィーチャーの主目的は「保守コスト削減」であり、ラッパー関数を維持するとその目的が達成されない。`gh_token::get()` は同一シグネチャを持つため、直接呼び出しで十分。
- **Trade-offs**: テスト `resolve_token_from_env` の削除/修正が必要だが、これは acceptable なコスト
- **Follow-up**: `gh_token::get()` のエラーメッセージが既存のものと同等かを確認

## Risks & Mitigations

- `serde_yaml` 0.9 (archived) への間接依存 — `gh-token` は141行の小さなクレートであり、実用上のリスクは低い。dtolnay が使用し続けている限り問題なし
- テスト `resolve_token_from_env` の削除 — `gh_token::get()` の動作は `GH_TOKEN` / `GITHUB_TOKEN` 環境変数テストで検証可能。ただし `gh_token` 内部のテストに依存することになる

## References
- gh-token crate (dtolnay) — `GH_TOKEN → GITHUB_TOKEN → hosts.yml → gh auth token` の4段階フォールバック実装
- ccforge での実績 — `../ccforge/src/adapter/outbound/github_auth.rs` で同クレートを使用済み
