# リサーチ・設計判断記録

---
**目的**: ディスカバリー結果、アーキテクチャ検討、設計判断の根拠を記録する。

---

## サマリー
- **フィーチャー**: `issue-211` — fix(config): max_retries および数値設定フィールドのゼロ値バリデーション
- **ディスカバリースコープ**: Simple Addition（既存バリデーション関数への 1 行追加）
- **主な発見点**:
  - `Config::validate` は既に複数の数値フィールドをバリデートしているが、`max_retries` が欠落している
  - `max_retries = 0` の場合、`decide.rs` の全リトライ判定が初回から `true` になり、Issue が即座にキャンセルされる
  - 既存のバリデーションパターン（`if self.max_ci_fix_cycles == 0 { return Err(...) }`）をそのまま踏襲できる

## リサーチログ

### 既存バリデーション実装の調査

- **コンテキスト**: 新しいバリデーション追加前に、既存パターンとの整合性を確認する
- **調査対象**: `src/domain/config.rs` L51–86
- **発見点**:
  - `polling_interval_secs < 10` → `Err("polling_interval_secs must be at least 10")`（ゼロ含む最小値チェック）
  - `stall_timeout_secs < 60` → `Err("stall_timeout_secs must be at least 60")`（ゼロ含む最小値チェック）
  - `max_concurrent_sessions == Some(0)` → `Err("max_concurrent_sessions must be greater than 0")`（パターンマッチ）
  - `max_ci_fix_cycles == 0` → `Err("max_ci_fix_cycles must be greater than 0")`（直接比較）
  - `max_retries` — **バリデーションなし（欠落）**
- **影響**: `max_ci_fix_cycles` と同じパターンで追加すればよい

### 使用箇所の調査（decide.rs）

- **コンテキスト**: `max_retries = 0` がどのような副作用を引き起こすかを確認
- **発見点**:
  - `decide.rs` の 5 箇所（L198, L253, L387, L453, L601）で `consecutive_failures >= cfg.max_retries` を評価
  - `max_retries = 0` の場合、`0 >= 0` が初回から `true` → すべての Issue が最初の失敗でキャンセル
  - ランタイムロジックの変更は不要。バリデーション層のみ修正すればよい
- **影響**: バリデーション追加のみで問題を解決可能。runtime コードへの変更は不要

### テストパターンの調査

- **コンテキスト**: 既存テストの記述パターンを確認して整合性を保つ
- **発見点**:
  - `validate_rejects_zero_max_ci_fix_cycles` が `max_ci_fix_cycles` の同様テストとして存在
  - `Config::default_with_repo("o", "r", "main")` を使う簡潔なパターンが確立されている
  - `unwrap_err()` で `Err` を取り出し、直接文字列アサートするパターンが標準

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク | 備考 |
|-----------|------|------|--------|------|
| 直接 if チェック追加 | `if self.max_retries == 0 { return Err(...) }` | 既存パターンと一致、最小変更 | なし | 採用 |
| 最小値チェック（`< 1`） | `if self.max_retries < 1` | 意味的に同等 | 不整合感 | 不採用（`max_ci_fix_cycles` パターンに合わせて `== 0`） |

## 設計判断

### 判断: バリデーション追加位置

- **コンテキスト**: `max_ci_fix_cycles` チェック（L81–83）の直後に挿入するのが最も自然
- **選択したアプローチ**: L83 の後ろ（`self.models.validate()?` の前）に挿入
- **根拠**: 数値フィールドのバリデーションをまとめた箇所の末尾に追加することで可読性を保つ
- **トレードオフ**: なし（挿入位置による動作の違いなし）

### 判断: エラーメッセージ形式

- **コンテキスト**: 既存エラーメッセージの形式に合わせる
- **選択したアプローチ**: `"max_retries must be greater than 0"` — `max_ci_fix_cycles` と同形式
- **根拠**: 一貫したエラーメッセージ形式でユーザーが混乱しない

## リスクと軽減策

- **後方互換性**: デフォルト値 `max_retries = 3` はバリデーションを通過するため、既存ユーザーに影響なし
- **設定ファイル破壊リスク**: `max_retries = 0` を明示的に設定しているユーザーは起動時にエラーになるが、それが意図した保護動作
- **テスト漏れ**: 既存テストパターンを踏襲することでリスク最小化

## 参考資料

- `src/domain/config.rs` — 既存バリデーション実装
- `src/domain/decide.rs` — max_retries の使用箇所（L198, L253, L387, L453, L601）
