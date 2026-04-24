# replace-expect-with-context サマリ

## Feature
本番コードに残る `expect()` を `anyhow::Context` ベースのエラー伝播 (`.context()?`) に置換し、デーモン全体の panic クラッシュリスクを排除。併せて `clippy::expect_used = "deny"` lint で再発を静的に防止する。対象は `prompt.rs` 2 箇所 + `transition_use_case.rs` 1 箇所の計 3 箇所。

## 要件サマリ
- `build_session_config` の戻り値を `SessionConfig` → `anyhow::Result<SessionConfig>` に変更。DesignFixing/ImplementationFixing で `pr_number == None` なら `Err("fixing state requires pr_number in DB")`。
- `step7_spawn_processes` でエラー時は `warn!` ログ + `continue` で対象 Issue のみスキップし、他 Issue 処理を継続（Graceful Degradation）。
- `TransitionUseCase` の `reset_for_restart` 後の `find_by_id` を `.context("issue not found after reset_for_restart")?` に変更。`step6_apply_events` の既存 `if let Err(e)` ハンドラでキャッチ。
- `Cargo.toml` の `[lints.clippy]` に `expect_used = "deny"` を追加。
- `src/lib.rs` に `#![cfg_attr(test, allow(clippy::expect_used))]`、`tests/integration_test.rs` に `#![allow(clippy::expect_used)]` を追加してテストコードは許容。
- `session_manager.rs:86` の `expect("wait after kill")` は kill 直後で実質到達不可能のため変更対象外。

## アーキテクチャ決定
- **`anyhow::Error` 採用** (選択): application 層は既に `anyhow::Context` を利用しており一貫性が高い。カスタム `SessionConfigError` 型は変更範囲が増大しこのスコープではオーバーエンジニア。
- **Graceful Degradation で Issue 単位のスキップ**: 既存 `step7_spawn_processes` が `continue` パターンを複数箇所で使っており、自然に馴染む。panic → 全 Issue 停止を避け運用継続性を優先。
- **`step6_apply_events` のエラーハンドラは既存のまま活用**: `TransitionUseCase` 側で `?` 伝播するだけで呼び出し元は無変更、差分最小化。
- **`expect_used` lint + テストコード除外の 2 層構成**: lib クレート本体は `cfg_attr(test, allow(...))` で単体テストを除外、`tests/` 配下は `cfg_attr` が伝播しないため integration test に個別 `allow` を追加する必要がある点を調査で発見。

## コンポーネント
- `src/application/prompt.rs`:
  - `build_session_config` シグネチャ変更: `pub fn build_session_config(...) -> anyhow::Result<SessionConfig>`
  - `pr_number.expect(...)` を `pr_number.context("fixing state requires pr_number in DB")?` へ置換（DesignFixing/ImplementationFixing 両方）。
- `src/application/polling_use_case.rs` (`step7_spawn_processes`): `match build_session_config(...)` で `Err(e) => { tracing::warn!(...); continue; }`。
- `src/application/transition_use_case.rs`: `find_by_id(...)?.context("issue not found after reset_for_restart")?` へ置換。
- `Cargo.toml`: `[lints.clippy]` に `expect_used = "deny"` を追記。
- `src/lib.rs`: `#![cfg_attr(test, allow(clippy::expect_used))]` クレート属性追加。
- `tests/integration_test.rs`: `#![allow(clippy::expect_used)]` クレート属性追加。
- ユニットテスト: `build_session_config` に DesignFixing/ImplementationFixing + `pr_number=None` で `Err`、他状態では `Ok` を返すことを検証するテストを追加。

## 主要インターフェース
```rust
pub fn build_session_config(
    state: State,
    issue_number: u64,
    config: &Config,
    pr_number: Option<u64>,
    feature_name: Option<&str>,
    fixing_causes: &[FixingProblemKind],
) -> anyhow::Result<SessionConfig>;
```

## 学び / トレードオフ
- `Cargo.toml` の既存 `[lints.clippy] all = { level = "warn", priority = -1 }` 構成を発見し、単純な追記では priority が衝突する可能性があるため実設定形式に沿って追記する必要があった（research で明示化）。
- `cfg_attr(test, ...)` がクレートレベル属性として単体テストには効くが、`tests/` 配下は別クレート扱いのため伝播しない Rust の制約を踏まえ、integration test 側に明示的な `allow` を置く方針を採用。
- thiserror の厳格な型安全性は犠牲になるが、ユースケース上問題なし。将来的な `SessionConfigError` 導入は別リファクタとして切り出し可能。
- `session_manager.rs` の kill 直後 `expect` を意図的に残す判断（到達不可能性で正当化）は、lint deny の例外管理という別の課題を生むが、本スコープでは変更対象外として明確化。
