# 設計書

## 概要

`cupola doctor` のUX改善として、以下2点を実装する:

1. **末尾一括ガイダンス**: Warn/Fail が1件以上あれば出力末尾に `cupola doctor` 再実行を促すメッセージを表示
2. **remediation フォーマット統一**: コマンド・ツール名・キー名へのバッククォート付加、URL 記法の統一

変更箇所はプレゼンテーション層 (`src/bootstrap/app.rs`) と文字列定数 (`src/application/doctor_use_case.rs`) のみ。アーキテクチャ変更や新しい型の追加はない。

---

## アーキテクチャ上の位置づけ

```
bootstrap/app.rs          ← ① 末尾ガイダンス出力ロジック追加
  └─ application/
       └─ doctor_use_case.rs  ← ② remediation 文字列の書き換え
            └─ doctor_result.rs  (変更なし)
```

`DoctorCheckResult` / `CheckStatus` / `DoctorSection` の型定義は変更しない。`DoctorUseCase::run()` の戻り値型・シグネチャも変更しない。

---

## 詳細設計

### ① 末尾一括ガイダンス (`src/bootstrap/app.rs`)

#### 現状のフロー

```
1. Start Readiness セクションを表示
2. Operational Readiness セクションを表示
3. has_failure (StartReadiness Fail) があれば Err を返す
```

#### 変更後のフロー

```
1. Start Readiness セクションを表示
2. Operational Readiness セクションを表示
3. [NEW] Warn/Fail が1件以上あれば末尾ガイダンスを表示
4. has_failure (StartReadiness Fail) があれば Err を返す
```

#### 実装

`results` 全体（Start Readiness + Operational Readiness）を走査し、1件でも `CheckStatus::Warn` または `CheckStatus::Fail` があれば以下を出力する:

```rust
let has_issues = results.iter().any(|r| {
    matches!(r.status, CheckStatus::Warn(_) | CheckStatus::Fail(_))
});
if has_issues {
    println!();
    println!("─────────────────────────────────────");
    println!("Fix the issues above, then run `cupola doctor` again to verify.");
}
```

`has_failure` の判定（Start Readiness の Fail のみ）は既存ロジックをそのまま維持する。

---

### ② remediation フォーマット統一 (`src/application/doctor_use_case.rs`)

#### 変更対象と変更内容

##### `check_git` (3箇所、内容は同一)

```rust
// Before
"git をインストールしてください: https://git-scm.com/"

// After
"`git` をインストールしてください (https://git-scm.com/)"
```

##### `check_github_token` — gh 未インストール (1箇所)

```rust
// Before
"gh CLI をインストールしてください: https://cli.github.com/"

// After
"`gh` をインストールしてください (https://cli.github.com/)"
```

##### `check_claude` (2箇所、内容は同一)

```rust
// Before
"https://claude.ai/code からインストールしてください"

// After
"`claude` をインストールしてください (https://claude.ai/code)"
```

##### `check_env_allowlist` — ワイルドカード

```rust
// Before
"cupola.toml の extra_allow から \"*\" を削除し、必要な変数名を明示的に指定してください"

// After
"cupola.toml の `extra_allow` から `\"*\"` を削除し、必要な変数名を明示的に指定してください"
```

##### `check_env_allowlist` — 危険パターン

```rust
// Before
"不要であれば cupola.toml の extra_allow から削除してください"

// After
"不要であれば cupola.toml の `extra_allow` から削除してください"
```

##### `check_labels` — gh 未インストール (2箇所、`agent:ready ラベル` と `weight:* ラベル`)

```rust
// Before
"gh CLI をインストールしてください: https://cli.github.com/"

// After
"`gh` をインストールしてください (https://cli.github.com/)"
```

##### `check_pending_ci_fix_limit_notifications` — Warn

```rust
// Before
"GitHub の通信状況を確認の上、cupola start で再度ポーリングを実行してください"

// After
"GitHub の通信状況を確認の上、`cupola start` で再度ポーリングを実行してください"
```

##### `check_pending_ci_fix_limit_notifications` — DB エラー

```rust
// Before
"cupola init で DB を初期化してください"

// After
"`cupola init` で DB を初期化してください"
```

#### 変更しない remediation (フォーマット準拠済み)

以下はすでにルール準拠のため変更しない:

| チェック | remediation |
|---------|-------------|
| `config` (NotFound) | `` `cupola init` を実行して cupola.toml を作成してください `` |
| `config` (ValidationFailed) | `cupola.toml の設定値を確認・修正してください` (汎用的指示のためバッククォート不要) |
| `github token` (auth fail) | `` `gh auth login` を実行してください `` |
| `database` | `` `cupola init` を実行してください `` |
| `assets` | `` `cupola init` を実行してください `` |
| `steering` | `` `cupola init` を実行するか、`/cupola:steering` でステアリングファイルを作成してください `` |
| `agent:ready ラベル` (auth failed) | `` `gh auth login` を実行してください `` |
| `weight:* ラベル` (auth failed) | `` `gh auth login` を実行してください `` |
| `agent:ready ラベル` (missing) | `` `gh label create agent:ready` を実行してください `` |
| `weight:* ラベル` (missing) | `` `gh label create weight:light && gh label create weight:heavy` を実行してください `` |

---

## テスト設計

### 末尾ガイダンス (app.rs)

`app.rs` に直接ユニットテストを書くより、既存の doctor テストで remediation 文字列を検証している箇所の更新が中心となる。末尾ガイダンスの出力確認は手動テストまたは統合テストで行う（現状の app.rs テストは CLI パース検証のみ）。

### `doctor_use_case.rs` の既存テスト更新

`src/application/doctor_use_case.rs` の `#[cfg(test)]` ブロック内で、変更した remediation 文字列を検証しているテストを特定し、新フォーマットに合わせて更新する。主な対象:

- `check_git` に関するテスト (remediation 文字列を `assert_eq!` で検証しているもの)
- `check_github_token` / `check_claude` に関するテスト
- `check_env_allowlist` に関するテスト
- `check_pending_ci_fix_limit_notifications` に関するテスト
- `check_labels` の gh 未インストールパスに関するテスト

---

## 変更しない設計判断

| 項目 | 理由 |
|------|------|
| `DoctorCheckResult` 型 | 変更するとドメイン型に影響し影響範囲が広がる |
| `DoctorUseCase::run()` シグネチャ | 戻り値 `Vec<DoctorCheckResult>` はそのまま |
| 末尾ガイダンスの英語表記 | イシューで「Fix the issues above...」と明示されているため |
| セパレータの文字数 | イシューで「─────────────────────────────────────」と明示されているため |
| 末尾ガイダンスの配置 | Operational Readiness 表示後・`Err` 返却前 |

---

## 将来の考慮事項

### #331 (i18n) との連携

本 PR では文字列を日本語のままフォーマット統一する。#331 で i18n 対応する際は:

1. 統一済み文字列を `locales/ja.yml` のキーに抽出
2. `remediation: Some("...".to_string())` を `remediation: Some(t!("cli.doctor.remediation.XXX").to_string())` に置換

フォーマットが統一されているため、抽出作業がシンプルになる。

### 将来の doctor チェック追加時のガイドライン

- `doctor` 再実行で確認できる修正 → 末尾ガイダンスで完結、個別に「再実行してください」は書かない
- `cupola start` など doctor 以外の後続アクションが必要な場合 → remediation に個別に明示する
  ```rust
  remediation: Some(
      "1. `cupola init` でデータベースを初期化してください\n2. `cupola start -d` でデーモンを起動してください".to_string()
  )
  ```
