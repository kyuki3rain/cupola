# 要件定義書

## はじめに

本仕様は、`cupola doctor` コマンド（PR #46 で実装済み）に対して Copilot レビューで指摘された 7 件の品質問題を修正するためのものである。対象領域は、テストの環境依存・アーキテクチャ違反・エラーハンドリングの改善にわたる。

## 要件

### Requirement 1: git テストの環境依存解消

**Objective:** 開発者として、git が未インストールの環境でも `cargo test` が失敗しないようにしたい。これにより CI 環境や git 非インストール環境での安定したテスト実行を保証できる。

#### 受け入れ条件

1. When `cargo test` が git 未インストール環境で実行された場合、`doctor` モジュールの git 関連テストは shall git の存在確認を先に行ってからアサートを実施するか、スキップする。
2. If git コマンドが PATH 上に存在しない場合、`doctor` モジュールの git 関連テストは shall `#[ignore]` またはランタイムスキップ（早期 `return`）で回避し、テストスイート全体を失敗させない。
3. `doctor` モジュールの git チェックテストは shall 環境依存のアサートを条件付きで実行し、git 存在時のみ結果を検証する。

---

### Requirement 2: toml / steering / db チェックのユニットテスト追加

**Objective:** 開発者として、`cupola.toml` の存在・内容および steering ディレクトリ・DB ファイルのチェックロジックを単体テストで検証したい。これにより回帰防止とロジック正確性を担保できる。

#### 受け入れ条件

1. When `check_toml` が `tempdir` 上に `cupola.toml` が存在する環境で呼び出された場合、the doctor テストは shall 必須フィールドが揃っていることを検証するアサートをパスする。
2. When `check_toml` が `tempdir` 上に `cupola.toml` が存在しない環境で呼び出された場合、the doctor テストは shall チェック失敗（`CheckStatus::Fail` 相当）を返すことを検証する。
3. When `check_toml` が必須フィールドを欠いた `cupola.toml` を持つ `tempdir` で呼び出された場合、the doctor テストは shall チェック失敗を返すことを検証する。
4. The steering チェックと db チェックのテストも shall `tempdir` を使った制御された環境で、存在する/しないの両ケースをカバーする。

---

### Requirement 3: gh 未インストールと認証失敗の区別

**Objective:** 開発者として、`gh` コマンドが未インストールの場合とログインが必要な場合で異なるガイダンスを表示させたい。これにより問題の特定と解決が素早く行える。

#### 受け入れ条件

1. When `gh` コマンドが PATH 上に存在しない場合、the `doctor` コマンドは shall `gh` のインストール手順（例: `brew install gh` またはドキュメント URL）を案内するメッセージを表示する。
2. When `gh` コマンドが存在するが認証が完了していない（`gh auth status` が失敗する）場合、the `doctor` コマンドは shall `gh auth login` を実行するよう案内するメッセージを表示する。
3. The `doctor` コマンドは shall 未インストールと認証失敗を同一メッセージで混同せず、それぞれ独立した診断結果として出力する。

---

### Requirement 4: DoctorUseCase の bootstrap 層依存解消（依存逆転）

**Objective:** アーキテクトとして、`DoctorUseCase` が `bootstrap::config_loader::load_toml` に直接依存しないようにしたい。これにより Clean Architecture の依存方向（application → bootstrap は禁止）を遵守できる。

#### 受け入れ条件

1. The `DoctorUseCase` は shall `bootstrap` クレートおよびモジュールを直接 `use` しない。
2. The 設定読み込み機能は shall `application` 層に定義された port trait（例: `ConfigLoader`）経由でのみ `DoctorUseCase` から利用される。
3. Where 表示ロジック（✅/❌ フォーマット文字列生成）が `DoctorUseCase` 内に存在する場合、the リファクタリングは shall それを `adapter/inbound`（CLI ハンドラ）側に移動する。
4. The `DoctorUseCase` のユニットテストは shall モックした port trait を注入することで bootstrap 層に依存せずにテスト可能である。

---

### Requirement 5: agent:ready ラベルチェックの JSON パース

**Objective:** 開発者として、`agent:ready` ラベルの存在確認を文字列マッチではなく JSON パースで厳密に行いたい。これにより誤検知（例: `not-agent:ready` を含む文字列のマッチ）を防止できる。

#### 受け入れ条件

1. When `gh` コマンドの stdout に `agent:ready` を名前として持つラベルオブジェクトが含まれる場合、the `doctor` コマンドは shall `serde_json` でパースして `name == "agent:ready"` を厳密に確認し、チェック成功とする。
2. If `gh` コマンドの stdout が `agent:ready` を含む文字列だが JSON オブジェクトの `name` フィールドに一致しない場合、the `doctor` コマンドは shall それを `agent:ready` ラベル存在の証拠とみなさない。
3. If `gh` コマンドの stdout が有効な JSON でない場合、the `doctor` コマンドは shall パースエラーをチェック失敗として扱い、ユーザーに適切なエラーメッセージを表示する。

---

### Requirement 6: steering チェックでファイルのみをカウント

**Objective:** 開発者として、steering ディレクトリに実際の `.md` ファイルが存在する場合のみチェックを成功させたい。これにより、ディレクトリのみ・隠しファイル（`.DS_Store`）のみの場合の誤検知を防ぐ。

#### 受け入れ条件

1. When steering ディレクトリのエントリを走査する際、the `doctor` コマンドは shall `entry.file_type()?.is_file()` でフィルタし、通常ファイルのみをカウント対象とする。
2. If steering ディレクトリが存在するが通常ファイルを含まない（空、または `.DS_Store` のようなシステムファイルのみ）場合、the `doctor` コマンドは shall このチェックを失敗として報告する。
3. If steering ディレクトリ内にサブディレクトリのみ存在し通常ファイルがない場合、the `doctor` コマンドは shall チェックを失敗として扱う。

---

### Requirement 7: std::process::exit(1) の Err 返却への変更

**Objective:** 開発者として、`doctor` コマンドの異常終了時に `std::process::exit(1)` を使わず `Err` を返すようにしたい。これにより `tracing` ロガーの guard の drop が保証され、ログが確実にフラッシュされる。

#### 受け入れ条件

1. The `doctor` サブコマンドのハンドラは shall `std::process::exit(1)` を呼び出さず、`Err(anyhow!(...))` を返して呼び出し元（`main`）に制御を委譲する。
2. The `main` 関数は shall `doctor` ハンドラから `Err` が返された場合に終了コード 1 で終了する処理を担う。
3. When `doctor` コマンドが異常終了する場合、the システムは shall `tracing` ロガーの guard が正常に drop され、未フラッシュのログが失われないことを保証する。
