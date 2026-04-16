# リサーチ & 設計判断記録

---
**目的**: ディスカバリーの調査結果・アーキテクチャ調査・設計根拠を記録する。

---

## サマリー

- **フィーチャー**: `cupola init --upgrade` フラグの実装
- **ディスカバリースコープ**: Extension（既存のinitコマンドへの機能追加）
- **主要調査結果**:
  - `FileGenerator` トレイトの既存メソッドは `upgrade: bool` パラメータを受け取るよう拡張するのが最も自然
  - `.gitignore` のセクション置換は独自ロジックが必要（既存の追記ロジックとは異なる）
  - `CLAUDE_CODE_ASSETS` 配列で管理されるファイルがCupola管理ファイルの全体セット

## リサーチログ

### Cupola管理ファイルの列挙

- **コンテキスト**: `--upgrade` で上書き対象となるファイルを正確に把握する必要がある
- **参照元**: `src/adapter/outbound/init_file_generator.rs` の `CLAUDE_CODE_ASSETS`
- **調査結果**:
  - `CLAUDE_CODE_ASSETS` 配列には 24 ファイルが定義されている
  - 対象: `.claude/commands/cupola/*.md` (4ファイル)、`.cupola/settings/rules/*.md` (7ファイル)、`.cupola/settings/templates/specs/*.md|json` (6ファイル)、`.cupola/settings/templates/steering/*.md` (3ファイル)、`.cupola/settings/templates/steering-custom/*.md` (7ファイル)
  - `install_claude_code_assets()` は全ファイルを `if path.exists() { continue; }` でスキップしている
  - `.cupola/steering/` ディレクトリ作成は `install_claude_code_assets()` 内で行われているが、steering配下のファイルは生成していない
- **インパクト**: `upgrade=true` 時は `continue` を除去し、無条件で上書きすれば良い

### `.gitignore` のセクション置換設計

- **コンテキスト**: アップグレード時に `.gitignore` 内のCupola管理セクションを最新エントリで置換する必要がある
- **参照元**: `src/adapter/outbound/init_file_generator.rs` の `append_gitignore_entries()`
- **調査結果**:
  - 現在の実装は `GITIGNORE_MARKER` (`# cupola`) の有無をチェックし、存在すればスキップ
  - アップグレード時の置換には「マーカー行から次の空行またはファイル末尾まで」のセクション特定ロジックが必要
  - Cupolaの `GITIGNORE_ENTRIES` は `# cupola\n` で始まり末尾は `\n` で終わる固定ブロック
  - セクション外のユーザー定義エントリを保護するため、行単位での分割・再結合アプローチが適切
- **インパクト**: `upgrade=true` 時、`GITIGNORE_MARKER` で始まるセクションを見つけて置換する専用ロジックを実装する

### `FileGenerator` トレイトの拡張方式

- **コンテキスト**: アップグレードフラグをアダプター層に伝達する方式を決定する必要がある
- **参照元**: `src/application/port/file_generator.rs`、`src/application/init_use_case.rs`
- **調査結果**:
  - 現在のトレイトメソッド: `install_claude_code_assets(&self) -> Result<bool>` など
  - `InitUseCase` は `file_gen` フィールドを保持し、`run()` 内で各メソッドを呼び出す
  - アップグレードフラグは `InitUseCase` がコンストラクタで受け取り、`run()` 内でトレイトメソッドに渡すのが自然
- **インパクト**: 設計決定 #1 参照

### `bootstrap/app.rs` の Init ハンドラ

- **コンテキスト**: `upgrade` フラグが CLI から `InitUseCase` まで正しくスレッドされるか確認
- **参照元**: `src/bootstrap/app.rs` lines 93-153
- **調査結果**:
  - `Command::Init { agent }` をパターンマッチし、`InitUseCase::new(base_dir, db_existed, db, file_gen, runner, agent.into())` を生成
  - `upgrade` フィールドを `Command::Init` に追加し、`InitUseCase::new()` に渡す変更が必要
  - 出力メッセージ (`println!` 5行) は `InitReport` フィールドを参照しているため、report 構造体への変更に応じて更新が必要
- **インパクト**: 変更箇所は局所的。既存のステータス表示パターンと整合させる

## アーキテクチャパターン評価

| オプション | 説明 | 利点 | リスク / 制約 | 備考 |
|-----------|------|------|---------------|------|
| A: `upgrade: bool` をトレイトメソッドに追加 | `install_claude_code_assets(upgrade: bool)` など既存メソッドにパラメータを追加 | シンプル、一貫性が高い | 全テストの呼び出し箇所を更新が必要 | Issue の実装案と一致 |
| B: 上書き専用の新メソッドを追加 | `install_claude_code_assets_forced()` などを追加 | 後方互換 | トレイトが肥大化、重複ロジックが発生 | 採用しない |
| C: `InitUseCase` にフラグを持たせ、既存トレイトメソッドはそのまま | UseCase 内で upgrade 分岐し、別途 FileGenerator の `force_write` メソッドを呼ぶ | トレイト変更が最小 | アップグレードロジックが UseCase に漏れる | Clean Architecture 違反のリスク |

**選択**: Option A — メソッドに `upgrade: bool` を追加する。

## 設計決定

### 決定: `FileGenerator` トレイトメソッドに `upgrade: bool` を追加

- **コンテキスト**: アップグレード動作はファイル生成ロジック（adapter 層）に属するため、抽象化境界を維持しつつフラグを伝達する必要がある
- **検討した選択肢**:
  1. Option A — トレイトメソッドに `upgrade: bool` パラメータを追加
  2. Option B — 上書き専用の新メソッドをトレイトに追加
- **選択したアプローチ**: `install_claude_code_assets(upgrade: bool)` および `append_gitignore_entries(upgrade: bool)` とする。`generate_toml_template` と `generate_spec_directory(_at)` はユーザー所有ファイル操作のため変更なし。
- **根拠**: 動作の差分が `bool` 一つで表現できる場合、メソッド分割より引数追加の方がシンプルで、Rust の borrow checker とも相性が良い。テスト更新のコストは限定的。
- **トレードオフ**: 既存の全テストの呼び出し箇所を `false` に更新する必要がある点がコスト。
- **フォローアップ**: `MockFileGenerator` (テストサポート) がある場合は同様に更新する。

### 決定: `.gitignore` セクション置換の実装方針

- **コンテキスト**: `upgrade=true` かつマーカー既存の場合、セクション全体を最新エントリで置換する
- **検討した選択肢**:
  1. マーカー行から末尾まで一括置換（後続のユーザーエントリを削除してしまうリスク）
  2. マーカー行からその次の空行/ファイル末尾まで置換（Cupolaブロックのみ安全に差し替え）
- **選択したアプローチ**: 行単位でファイルを分割し、`# cupola` マーカー行から連続する非空行（Cupolaブロック）を特定して新エントリに差し替える。マーカー以降でも空行で区切られた後のユーザー行は保持する。
- **根拠**: `GITIGNORE_ENTRIES` は連続ブロックであり空行を含まないため、「マーカーから次の空行まで」でCupolaセクションを正確に特定できる。
- **トレードオフ**: ロジックが `append_gitignore_entries` に追加され複雑になるが、ユーザーデータ保護の観点で必要なコスト。
- **フォローアップ**: CRLF改行コード保持ロジックも upgrade パスで維持する。

## リスクと緩和策

- テスト更新漏れ: `FileGenerator` トレイトメソッドのシグネチャ変更により、`MockFileGenerator` や全テストの呼び出し箇所でコンパイルエラーが発生する。`cargo check` を早期に実行して漏れを検出する。
- `.gitignore` の置換ロジックのバグ: セクション境界の判定が誤るとユーザーエントリを消失させる可能性がある。プロパティベーステストでエッジケース（末尾改行なし、空ファイル、複数Cupolaブロックなど）を検証する。
- `upgrade=true` 時の `InitReport` 報告: 現在の `agent_assets_installed: bool` の意味が「新規インストール」から「変更あり」にシフトするが、アップグレード時は既存ファイルを上書きするため `true` が返る。報告メッセージを `upgrade` フラグに応じて切り替えることで意味の齟齬を回避する。

## 参照

- `src/adapter/inbound/cli.rs` — CLI フラグ定義
- `src/application/init_use_case.rs` — InitUseCase 実装
- `src/application/port/file_generator.rs` — FileGenerator ポートトレイト
- `src/adapter/outbound/init_file_generator.rs` — InitFileGenerator 実装
- `src/bootstrap/app.rs` — Init コマンドハンドラ
- `docs/commands/init.md` — 既存ドキュメント（--upgrade 仕様の原典）
