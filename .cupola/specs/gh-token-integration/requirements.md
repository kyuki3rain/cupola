# 要件定義書

## プロジェクト説明（入力）
gh-token クレート導入による GitHub トークン取得ロジックの簡素化 - resolve_github_token() の自前実装を gh-token クレートに置き換え

## はじめに

現在、cupola の `src/adapter/outbound/github_rest_client.rs` には `resolve_github_token()` という自前の GitHub トークン取得関数が実装されている。この関数は `GITHUB_TOKEN` 環境変数と `gh auth token` CLI コマンドの2段階フォールバックのみをサポートしているが、dtolnay 作の `gh-token` クレート (v0.1) を使用することで、`GH_TOKEN` → `GITHUB_TOKEN` → `~/.config/gh/hosts.yml` → `gh auth token` の4段階フォールバックを実現できる。本フィーチャーは `resolve_github_token()` を `gh_token::get()` に置き換え、保守コストの削減とトークン取得の網羅性向上を目的とする。

## 要件

### 要件 1: gh-token クレートの依存追加

**目的:** 開発者として、信頼性の高い外部クレートを使用してトークン取得ロジックを管理したい。それにより、自前実装の保守負担を排除できる。

#### 受け入れ基準
1. The cupola shall include `gh-token` クレート (v0.1) を `Cargo.toml` の依存関係として追加されている。
2. The cupola shall `cargo build` が既存の依存関係との競合なく成功する。
3. The cupola shall `gh-token` クレートが依存する `serde_yaml` 0.9 との互換性が維持される。

---

### 要件 2: resolve_github_token() の置き換え

**目的:** 開発者として、トークン取得ロジックを `gh_token::get()` で統一したい。それにより、コードの複雑さを減らし一貫した挙動を保証できる。

#### 受け入れ基準
1. When `resolve_github_token()` が呼び出される場面において、the cupola shall `gh_token::get()` を使用してトークンを取得する。
2. The cupola shall `src/adapter/outbound/github_rest_client.rs` から自前実装の `resolve_github_token()` 関数を削除する。
3. The cupola shall `gh_token::get()` の戻り値を既存の `anyhow::Result<String>` インターフェースに適合させる。
4. The cupola shall `src/bootstrap/app.rs` でのトークン取得呼び出しが変更後も正常に動作する。

---

### 要件 3: トークン取得の4段階フォールバック

**目的:** エンドユーザーとして、環境に応じたさまざまな方法でトークンを設定したい。それにより、異なる環境（CI/CD、ローカル開発）でも柔軟に動作できる。

#### 受け入れ基準
1. When `GH_TOKEN` 環境変数が設定されている場合、the cupola shall その値を GitHub トークンとして使用する。
2. When `GH_TOKEN` が未設定で `GITHUB_TOKEN` 環境変数が設定されている場合、the cupola shall `GITHUB_TOKEN` の値を GitHub トークンとして使用する。
3. When 環境変数が未設定で `~/.config/gh/hosts.yml` が存在する場合、the cupola shall そのファイルからトークンを取得する。
4. When 上記すべてが利用不可で `gh auth token` コマンドが利用可能な場合、the cupola shall CLI コマンドでトークンを取得する。
5. If すべてのフォールバックが失敗した場合、the cupola shall 適切なエラーメッセージを表示してプロセスを終了する。

---

### 要件 4: エラーハンドリングの維持

**目的:** 開発者として、既存の `anyhow` ベースのエラーハンドリングとの一貫性を維持したい。それにより、エラー処理の変更による副作用を最小限に抑えられる。

#### 受け入れ基準
1. When `gh_token::get()` がエラーを返した場合、the cupola shall `anyhow::Error` に変換して上位に伝搬する。
2. If トークンが取得できない場合、the cupola shall ユーザーが次のアクションを理解できる明確なエラーメッセージを表示する。
3. The cupola shall 既存の `anyhow::Result<String>` を返す関数シグネチャを維持する（または互換性のある形に変更する）。

---

### 要件 5: 既存テストの維持

**目的:** 開発者として、変更後も既存テストが正常に動作することを確認したい。それにより、リグレッションを防止できる。

#### 受け入れ基準
1. The cupola shall `cargo test` がすべてのテストを成功させる。
2. When `resolve_github_token()` を参照していたテストがある場合、the cupola shall 置き換え後の関数に対応するよう修正される。
3. The cupola shall `cargo clippy -- -D warnings` がエラーなく完了する。
