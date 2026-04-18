# Research & Design Decisions

---
**Feature**: issue-164 — エージェントプロセスの完了を待つ graceful shutdown モード  
**Discovery Scope**: Extension（既存 shutdown 機構の拡張）  
**Key Findings**:
- 既存の `StopUseCase` は SIGTERM → 30秒待機 → SIGKILL の2段階 shutdown を既に実装済み
- タイムアウト値は `Duration::from_secs(30)` としてハードコードされており、設定から読めない
- ポーリングループの graceful_shutdown() は 10 秒待機（ハードコード）でセッション終了を待つ
- `stop --force` フラグは未実装
- 2回 Ctrl+C（SIGINT 二重検出）のロジックは未実装
- `Config` struct に `shutdown_timeout_secs` フィールドが存在しない

---

## Research Log

### 現状の shutdown フロー調査

- **Context**: Issue #164 の実装前に既存コードの shutdown 挙動を把握する必要があった
- **Sources Consulted**: `src/application/stop_use_case.rs`, `src/application/polling_use_case.rs`, `src/bootstrap/app.rs`, `src/domain/config.rs`, `src/bootstrap/config_loader.rs`
- **Findings**:
  - `StopUseCase` は SIGTERM 送信後 500ms ポーリングで死活確認し、タイムアウト後に SIGKILL を送る。タイムアウトは 30 秒ハードコード
  - ポーリングループの `graceful_shutdown()` は `session_mgr.kill_all()` でセッションを kill してから最大 10 秒待機
  - シグナル受信（SIGTERM/SIGINT/SIGHUP）は全て同じ「即 break」処理で区別なし
  - `stop` CLI サブコマンドに `--force` フラグはなく、`Cli::Stop` に引数なし
- **Implications**:
  - `shutdown_timeout_secs` を `Config` / `CupolaToml` に追加し、`StopUseCase` へ注入する必要がある
  - ポーリングループの graceful_shutdown 待機時間も同じ設定値を参照すべき
  - `stop --force` は別の信号経路（SIGKILL 直接）または フラグで制御

### 2回 Ctrl+C (SIGINT) ダブル検出の実装パターン調査

- **Context**: フォアグラウンドモードで 1 回目は graceful、2 回目は即時 kill を実装する方法
- **Sources Consulted**: tokio の `signal::ctrl_c()` ドキュメント、`tokio::select!` パターン
- **Findings**:
  - tokio の `ctrl_c()` は Future なので `tokio::select!` で複数回ループすると 2 回目を検出できる
  - 一般的なパターン: 1 回目の SIGINT で shutdown モードに入り、2 回目の SIGINT で強制終了
  - 状態変数 `shutdown_requested: bool` をループ外で保持し、フラグ分岐で対応可能
  - tokio 1.x では `signal::unix::signal(SignalKind::interrupt())` を使うと複数回受信できる（ctrl_c() は1回限りの Future であることに注意）
- **Implications**:
  - `ctrl_c()` の代わりに `signal::unix::signal(SignalKind::interrupt())` を使い、受信回数をカウントする
  - ポーリングループの `run()` 関数に `sigint_count` カウンタを追加する

### `stop --force` のシグナル戦略

- **Context**: `cupola stop --force` 実行時に即時終了させる方法
- **Sources Consulted**: 既存の `NixSignalSender`, `StopUseCase`
- **Findings**:
  - `StopUseCase` は `force: bool` フラグを受け取り、`true` のとき SIGTERM をスキップして直接 SIGKILL を送る設計が最もシンプル
  - または SIGKILL を先に送って 0ms 待機で終了確認する
- **Implications**:
  - `StopUseCase::execute()` に `force: bool` 引数を追加する（または設定ベース）
  - Bootstrap の `Cli::Stop` コマンドに `force: bool` フィールド追加

### `shutdown_timeout_secs = 0` の無限待機実装

- **Context**: `0` を「無限待機」と解釈するセマンティクスの実装
- **Sources Consulted**: Rust `Duration`, `Option<Duration>` パターン
- **Findings**:
  - `0` を `None`（タイムアウトなし）に変換し、`Option<Duration>` でポーリングに渡す設計が明快
  - `StopUseCase` のタイムアウトループで `None` の場合は SIGKILL 送信をスキップする
- **Implications**:
  - `Config::shutdown_timeout` を `Option<Duration>` として保持する（`0` → `None`）
  - デフォルト値を `Some(Duration::from_secs(300))` とする

---

## Architecture Pattern Evaluation

| オプション | 説明 | 長所 | リスク・制約 | 備考 |
|-----------|------|------|------------|------|
| `StopUseCase` に `force` フラグ追加 | execute() 引数で force/graceful を分岐 | 既存構造を最小限に変更 | execute シグネチャ変更でテスト修正が必要 | 採用 |
| 別 UseCase `ForceStopUseCase` を作成 | graceful と force を別クラスに分離 | 単一責任が明確 | コード重複・bootstrap 変更が増える | 不採用 |
| SIGKILL 直接送信 CLI | bootstrap から直接 `NixSignalSender::send_sigkill` | シンプル | ポート抽象化を迂回、テスト困難 | 不採用 |

---

## Design Decisions

### Decision: `shutdown_timeout_secs` のドメイン表現

- **Context**: `0` を無限待機とする設定値の内部表現
- **Alternatives Considered**:
  1. `u64` のまま保持し `0` を特殊値として都度チェック
  2. `Option<Duration>` に変換して保持（`0` → `None`）
- **Selected Approach**: `Option<Duration>` — `None` が「タイムアウトなし」を表す
- **Rationale**: `None` による型レベルの区別が明確で、マジックナンバー `0` のチェックが不要
- **Trade-offs**: `CupolaToml` → `Config` 変換ロジックで変換が必要
- **Follow-up**: `StopUseCase` と `PollingUseCase::graceful_shutdown` 両方が同じ `Option<Duration>` を参照すること

### Decision: 2回 Ctrl+C の実装方法

- **Context**: tokio の `ctrl_c()` Future は 1 回限りなので複数回検出には工夫が必要
- **Alternatives Considered**:
  1. ループ内で毎回 `ctrl_c()` を再生成（`tokio::select!` 分岐内）
  2. `signal::unix::signal(SignalKind::interrupt())` を使って Stream として複数回受信
- **Selected Approach**: `signal::unix::signal(SignalKind::interrupt())` を使いカウンタで 2 回目を検出
- **Rationale**: 再利用可能な Signal Stream により毎回 Future を再生成する必要がない
- **Trade-offs**: `tokio::signal::unix` を使うため Unix 専用（Windows は非対応だが Cupola は Linux/Mac 前提）
- **Follow-up**: daemon モードではすでに SIGTERM のみ使用しているため、フォアグラウンド限定の動作として分岐

### Decision: `StopUseCase` に `force` フラグを注入する方法

- **Context**: `--force` オプションをユースケース層に伝える設計
- **Alternatives Considered**:
  1. `execute(&self, force: bool)` — 呼び出し時引数
  2. `new(... force: bool)` — コンストラクタ時注入
- **Selected Approach**: `execute(&self, force: bool)` — 呼び出し時引数
- **Rationale**: UseCase インスタンスはタイムアウト設定など静的設定のみ保持し、動的フラグは呼び出し引数に分離するのがクリーン
- **Trade-offs**: 既存の `execute(&self)` シグネチャ変更が必要（テスト含む）
- **Follow-up**: 既存テストの `execute()` 呼び出し箇所を `execute(false)` に更新

---

## Risks & Mitigations

- **ポーリングループの graceful_shutdown 待機時間が config と不一致になるリスク** → `PollingUseCase` コンストラクタに `shutdown_timeout: Option<Duration>` を追加してハードコードを除去する
- **daemon モードでは SIGTERM が `stop` コマンド経由で来るため、ポーリングループ側の「待機モード」と stop コマンド側の「タイムアウト監視」が競合する可能性** → `StopUseCase` のタイムアウトをポーリングループの shutdown_timeout より十分長く設定する（例: shutdown_timeout_secs + 60 秒）
- **無限待機設定時に `cupola stop` がハングする** → `StopUseCase` 側でも `None` タイムアウトを尊重し、無限ループを `Ctrl+C` でキャンセル可能にする

## References

- tokio signal documentation — `signal::unix::Signal` stream ベースのシグナル受信
- 既存実装: `src/application/stop_use_case.rs` — SIGTERM → polling → SIGKILL フロー
- 既存実装: `src/application/polling_use_case.rs:306-334` — graceful_shutdown() の現状
