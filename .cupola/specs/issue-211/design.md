# 設計ドキュメント

## 概要

`Config::validate` 関数（`src/domain/config.rs`）に `max_retries` フィールドのゼロ値バリデーションを追加する。`max_retries = 0` の設定を起動時に即座に拒否することで、`decide.rs` のリトライ判定ロジックが誤動作するのを防ぐ。

**対象ユーザー**: Cupola を設定するエンジニアおよびシステム管理者が、誤った設定値による無音の動作不良を防止できる。

**影響**: `Config::validate` の実装に 1 行の条件チェックを追加し、既存のユニットテストスイートに 3 件のテストケースを追加する。既存の有効な設定（`max_retries >= 1`）への影響はない。

### Goals

- `max_retries = 0` を設定バリデーション層で拒否する
- 既存のバリデーションパターン（`max_ci_fix_cycles == 0` チェック）との一貫性を保つ
- 追加・拒否・デフォルト値の 3 パターンをテストで網羅する

### Non-Goals

- `decide.rs` のランタイムロジックの変更（バリデーション層のみの修正）
- `polling_interval_secs`・`stall_timeout_secs` など既にバリデーション済みのフィールドへの変更
- 設定ファイルのマイグレーション（既存の有効な設定は影響を受けない）

## アーキテクチャ

### 既存アーキテクチャ分析

`Config` は `src/domain/` に属するドメイン層の値オブジェクト。`validate` メソッドはフレームワーク非依存の純粋な Rust 実装であり、I/O なし・外部依存なし。

既存のバリデーション実装パターン:

```rust
// src/domain/config.rs L81-83（既存）
if self.max_ci_fix_cycles == 0 {
    return Err("max_ci_fix_cycles must be greater than 0".to_string());
}
```

この直後に、同パターンで `max_retries` のチェックを追加する。

### アーキテクチャパターン・境界マップ

本変更は **ドメイン層内の単一ファイル修正** であり、レイヤー境界をまたがない。

```
domain/config.rs
  └─ Config::validate()
       └─ [追加] max_retries == 0 チェック  ← 唯一の変更箇所
```

**アーキテクチャへの影響なし**: application・adapter・bootstrap 層は変更不要。

### テクノロジースタック

| レイヤー | 選択 | 役割 | 備考 |
|---------|------|------|------|
| Domain | Rust (純粋関数) | バリデーションロジック | 外部依存なし |
| Test | `#[cfg(test)] mod tests` | ユニットテスト | 既存パターン踏襲 |

## 要件トレーサビリティ

| 要件 | サマリー | コンポーネント | インターフェース |
|------|---------|--------------|---------------|
| 1.1 | max_retries = 0 の拒否 | `Config::validate` | `validate() -> Result<(), String>` |
| 1.2 | max_retries >= 1 の受け入れ | `Config::validate` | `validate() -> Result<(), String>` |
| 1.3 | デフォルト値（3）の受け入れ | `Config::validate` | `validate() -> Result<(), String>` |
| 1.4 | エラーメッセージの文字列 | `Config::validate` | `Err(String)` |
| 2.1 | ゼロ値拒否テスト | テストモジュール | `#[test]` |
| 2.2 | 最小有効値受け入れテスト | テストモジュール | `#[test]` |
| 2.3 | デフォルト値受け入れテスト | テストモジュール | `#[test]` |
| 2.4 | エラーメッセージ完全一致アサート | テストモジュール | `assert_eq!` |

## コンポーネントとインターフェース

### コンポーネントサマリー

| コンポーネント | レイヤー | 役割 | 要件カバレッジ |
|-------------|---------|------|-------------|
| `Config::validate` | domain | 設定値の整合性チェック | 1.1, 1.2, 1.3, 1.4 |
| テストモジュール（`config.rs #[cfg(test)]`） | domain（テスト） | バリデーションのユニットテスト | 2.1, 2.2, 2.3, 2.4 |

### Domain Layer

#### Config::validate

| フィールド | 詳細 |
|-----------|------|
| Intent | 設定値が有効な範囲内であることを保証する純粋関数 |
| Requirements | 1.1, 1.2, 1.3, 1.4 |

**責務と制約**
- `max_retries == 0` の場合に `Err` を返す
- 既存の他フィールドバリデーション（文字列空チェック、数値下限チェック）との順序を維持する
- バリデーションは `max_ci_fix_cycles` チェックの直後に挿入する（L83 の後）
- ドメイン層の純粋関数としての性質を維持する（I/O なし、副作用なし）

**依存関係**
- なし（純粋関数）

**Contracts**: Service [x]

##### サービスインターフェース

```rust
// 変更後の Config::validate のコントラクト（概要）
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        // ... 既存チェック ...
        if self.max_ci_fix_cycles == 0 {
            return Err("max_ci_fix_cycles must be greater than 0".to_string());
        }
        // [新規追加]
        if self.max_retries == 0 {
            return Err("max_retries must be greater than 0".to_string());
        }
        // ... 以降の既存チェック ...
    }
}
```

- 事前条件: `Config` 構造体が構築済みであること
- 事後条件: すべての必須フィールドが有効であれば `Ok(())` を返す。無効なフィールドがあれば最初のエラーの `Err(String)` を返す
- 不変条件: バリデーション実行中に `self` は変更されない

**実装ノート**
- 挿入位置: `max_ci_fix_cycles` チェック（L81–83）の直後、`self.models.validate()?` の前
- エラーメッセージ: `"max_retries must be greater than 0"`（既存メッセージ形式と一致）
- リスク: なし（1 行追加、既存テスト群は引き続きパス）

## エラーハンドリング

### エラー戦略

`validate()` は失敗時に `Err(String)` を返すフェイルファーストパターンを採用している。最初に検出した無効値でただちに `return Err(...)` する。

### エラーカテゴリと対応

**設定エラー（起動時のみ発生）**:
- `max_retries == 0` → `Err("max_retries must be greater than 0")`
- bootstrap 層でバリデーション結果を受け取り、エラー時は起動を中止してメッセージをログ出力する（既存の動作と同様）

### モニタリング

バリデーションエラーは bootstrap 層でキャッチされ、tracing によって構造化ログに出力される（既存の動作）。

## テスト戦略

### ユニットテスト

`src/domain/config.rs` の `#[cfg(test)] mod tests` に以下を追加:

1. `validate_rejects_zero_max_retries`: `max_retries = 0` でバリデーションが `Err` を返し、メッセージが `"max_retries must be greater than 0"` であることを検証
2. `validate_accepts_positive_max_retries`: `max_retries = 1` でバリデーションが `Ok(())` を返すことを検証
3. `validate_accepts_default_max_retries`: `Config::default_with_repo` 生成の設定がバリデーションを通過することを検証（`max_retries = 3`）

既存テストはすべて引き続きパスすること。
