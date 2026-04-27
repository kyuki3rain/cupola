# タスク一覧

## T-1: 末尾一括ガイダンスの追加 (`src/bootstrap/app.rs`)

**ファイル**: `src/bootstrap/app.rs`
**箇所**: `Command::Doctor` ハンドラ内、Operational Readiness セクション表示ループの直後

**実装内容**:

`results` 全体を走査して Warn/Fail が1件以上あれば末尾ガイダンスを表示するロジックを追加する。

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

**完了条件**: Warn/Fail があれば末尾ガイダンスが表示され、全 OK の場合は表示されない

---

## T-2: `check_git` の remediation 更新 (`src/application/doctor_use_case.rs`)

**ファイル**: `src/application/doctor_use_case.rs`
**対象関数**: `check_git`

全3箇所の remediation を以下に統一:

```rust
// Before (3箇所)
remediation: Some("git をインストールしてください: https://git-scm.com/".to_string()),

// After (3箇所)
remediation: Some("`git` をインストールしてください (https://git-scm.com/)".to_string()),
```

**完了条件**: `check_git` の全エラーパスで新フォーマットが使用される

---

## T-3: `check_github_token` の remediation 更新 (`src/application/doctor_use_case.rs`)

**ファイル**: `src/application/doctor_use_case.rs`
**対象関数**: `check_github_token`

gh 未インストールパスの remediation を更新:

```rust
// Before
remediation: Some("gh CLI をインストールしてください: https://cli.github.com/".to_string()),

// After
remediation: Some("`gh` をインストールしてください (https://cli.github.com/)".to_string()),
```

**完了条件**: gh 未インストール時のみ変更、`gh auth login` パスは変更しない

---

## T-4: `check_claude` の remediation 更新 (`src/application/doctor_use_case.rs`)

**ファイル**: `src/application/doctor_use_case.rs`
**対象関数**: `check_claude`

全2箇所の remediation を以下に統一:

```rust
// Before (2箇所)
remediation: Some("https://claude.ai/code からインストールしてください".to_string()),

// After (2箇所)
remediation: Some("`claude` をインストールしてください (https://claude.ai/code)".to_string()),
```

**完了条件**: `check_claude` の全エラーパスで新フォーマットが使用される

---

## T-5: `check_env_allowlist` の remediation 更新 (`src/application/doctor_use_case.rs`)

**ファイル**: `src/application/doctor_use_case.rs`
**対象関数**: `check_env_allowlist`

ワイルドカードパスの remediation:
```rust
// Before
"cupola.toml の extra_allow から \"*\" を削除し、必要な変数名を明示的に指定してください"

// After
"cupola.toml の `extra_allow` から `\"*\"` を削除し、必要な変数名を明示的に指定してください"
```

危険パターンパスの remediation:
```rust
// Before
"不要であれば cupola.toml の extra_allow から削除してください"

// After
"不要であれば cupola.toml の `extra_allow` から削除してください"
```

**完了条件**: 2つのパスそれぞれで新フォーマットが使用される

---

## T-6: `check_labels` の remediation 更新 (`src/application/doctor_use_case.rs`)

**ファイル**: `src/application/doctor_use_case.rs`
**対象関数**: `check_labels`

`gh_not_installed_results` クロージャ内の2箇所 (`agent:ready ラベル` と `weight:* ラベル`) を更新:

```rust
// Before
remediation: Some("gh CLI をインストールしてください: https://cli.github.com/".to_string()),

// After
remediation: Some("`gh` をインストールしてください (https://cli.github.com/)".to_string()),
```

**完了条件**: gh 未インストールパスで新フォーマットが使用される（`auth_failed_results` パスは変更しない）

---

## T-7: `check_pending_ci_fix_limit_notifications` の remediation 更新 (`src/application/doctor_use_case.rs`)

**ファイル**: `src/application/doctor_use_case.rs`
**対象関数**: `check_pending_ci_fix_limit_notifications`

Warn パス:
```rust
// Before
"GitHub の通信状況を確認の上、cupola start で再度ポーリングを実行してください"

// After
"GitHub の通信状況を確認の上、`cupola start` で再度ポーリングを実行してください"
```

DB エラーパス:
```rust
// Before
"cupola init で DB を初期化してください"

// After
"`cupola init` で DB を初期化してください"
```

**完了条件**: 2つのパスそれぞれで新フォーマットが使用される

---

## T-8: 既存テストの更新

**ファイル**: `src/application/doctor_use_case.rs` (テストモジュール)

変更した remediation 文字列を検証しているテストを全て新フォーマットに更新する。

更新が必要なテストの探し方:
```bash
# 旧フォーマットの文字列を含むテストを特定
grep -n "git-scm.com\|cli.github.com\|claude.ai/code\|extra_allow から\|cupola start で\|cupola init で DB" src/application/doctor_use_case.rs
```

**完了条件**: `cargo test` が全テスト通過

---

## T-9: ビルド・テスト検証

**コマンド**:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

**完了条件**: 全てのコマンドがエラーなしで完了する
