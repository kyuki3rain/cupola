# 要件定義書

## プロジェクト概要

`cupola doctor` コマンドのUX改善。出力末尾への一括ガイダンス追加と、各 remediation メッセージのフォーマット統一を行う。

## 背景・現状の課題

### 現状の remediation メッセージ

`src/application/doctor_use_case.rs` に散在する remediation 文字列は以下の問題を持つ:

1. **次のアクションが不明確**: 「修正後に `cupola doctor` を再実行して確認する」という情報がない
2. **フォーマット不統一**: バッククォートの有無、URL の表記形式、複数手順の区切り方が統一されていない

具体例:
```
git をインストールしてください: https://git-scm.com/        # URL が `:` 区切り
`gh auth login` を実行してください                          # OK (バッククォートあり)
https://claude.ai/code からインストールしてください          # ツール名のバッククォートなし
cupola.toml の extra_allow から "*" を削除し...             # key/value のバッククォートなし
```

## 機能要件

### FR-1: doctor 出力末尾への一括ガイダンス

- 実行時に1件でも `CheckStatus::Warn` または `CheckStatus::Fail` があれば、全セクション出力後に以下を表示する:
  ```
  ─────────────────────────────────────
  Fix the issues above, then run `cupola doctor` again to verify.
  ```
- 全結果が `CheckStatus::Ok` の場合は末尾ガイダンスを表示しない
- 実装箇所: `src/bootstrap/app.rs` の `Command::Doctor` ハンドラ末尾

### FR-2: remediation フォーマット統一

以下のルールで全 remediation 文字列を統一する:

| 種別 | フォーマット |
|------|-------------|
| 単一コマンド | `` `command` を実行してください `` |
| URL 付きインストール案内 | `` `tool` をインストールしてください (https://example.com/) `` |
| 複数手順 | `1. 〜してください\n2. 〜してください` |
| 設定ファイル編集指示 | `` cupola.toml の `key` から `value` を削除してください `` |

統一ルール:
- コマンド / ツール名 / キー名 / 値は **必ずバッククォート** で囲む
- URL は括弧内に `(https://...)` で記載
- 複数手順は `\n` + 番号で段階を明示
- 末尾は `ください` に統一

### FR-3: 既存テストの追従

`src/application/doctor_use_case.rs` および `src/bootstrap/app.rs` のテストで remediation 文字列を検証している箇所を、新フォーマットに合わせて更新する。

## 変更対象メッセージ一覧

| チェック | 条件 | 現状 | 変更後 |
|---------|------|------|--------|
| `git` | エラー/未インストール | `git をインストールしてください: https://git-scm.com/` | `` `git` をインストールしてください (https://git-scm.com/) `` |
| `github token` | gh 未インストール | `gh CLI をインストールしてください: https://cli.github.com/` | `` `gh` をインストールしてください (https://cli.github.com/) `` |
| `claude CLI` | エラー/未インストール | `https://claude.ai/code からインストールしてください` | `` `claude` をインストールしてください (https://claude.ai/code) `` |
| `env allowlist` | ワイルドカード `*` | `cupola.toml の extra_allow から "*" を削除し、必要な変数名を明示的に指定してください` | `` cupola.toml の `extra_allow` から `"*"` を削除し、必要な変数名を明示的に指定してください `` |
| `env allowlist` | 危険パターン | `不要であれば cupola.toml の extra_allow から削除してください` | `` 不要であれば cupola.toml の `extra_allow` から削除してください `` |
| `agent:ready ラベル` / `weight:* ラベル` | gh 未インストール | `gh CLI をインストールしてください: https://cli.github.com/` | `` `gh` をインストールしてください (https://cli.github.com/) `` |
| `ci-fix-limit notification` | Warn | `GitHub の通信状況を確認の上、cupola start で再度ポーリングを実行してください` | `` GitHub の通信状況を確認の上、`cupola start` で再度ポーリングを実行してください `` |
| `ci-fix-limit notification` | DB エラー | `cupola init で DB を初期化してください` | `` `cupola init` で DB を初期化してください `` |

上記以外の remediation（`\`cupola init\` を実行してください`、`\`gh auth login\` を実行してください` 等）はすでにフォーマット準拠のため変更不要。

## 非機能要件

### NFR-1: 後方互換性

- `DoctorCheckResult` 構造体・`CheckStatus` 列挙型の変更なし
- ユースケースの公開 API (`DoctorUseCase::run`) の変更なし
- 出力変更は `app.rs` の表示ロジックと `doctor_use_case.rs` の文字列定数のみ

### NFR-2: i18n との協調 (#331)

- 本 PR では日本語文字列のままフォーマットを統一する
- #331 で i18n 化する際は、統一済みの文字列を yaml キーに抽出するだけでよい状態にする

## 受け入れ条件

- [ ] `cupola doctor` で1件でも Warn/Fail があれば末尾に一括ガイダンスが表示される
- [ ] 全結果が OK の場合、末尾ガイダンスが表示されない
- [ ] 変更対象一覧の全メッセージが新フォーマットに準拠している
- [ ] 既存の doctor テストが新フォーマットで通過する
- [ ] `cargo test` が通過する
- [ ] `cargo clippy -- -D warnings` が通過する
