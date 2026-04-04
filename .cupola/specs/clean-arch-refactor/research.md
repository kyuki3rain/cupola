# リサーチ・設計判断ログ

---
**目的**: clean-arch-refactor フィーチャーの発見フェーズにおける調査結果、アーキテクチャ検討、設計判断の根拠を記録する。

---

## サマリー

- **フィーチャー**: `clean-arch-refactor`
- **調査スコープ**: Extension（既存システムへの改修）— コードベース全体の依存方向調査
- **主要な発見**:
  - `application/init_use_case.rs` が `adapter::outbound::SqliteConnection` と `adapter::outbound::InitFileGenerator` を直接インポートしており、Clean Architecture の依存ルール違反が確認された
  - `application/stop_use_case.rs` に `nix` クレートの直接使用と `NixSignalSender` 実装が含まれており、adapter 実装が application 層に流出している
  - `application/doctor_use_case.rs` の 5 箇所で `std::process::Command` を直接使用しており、既存の `CommandRunner` ポートが活用されていない
  - `application/error.rs` の `CupolaError` は定義されているが、実際の use case は `anyhow::Result` を使用しており事実上未使用
  - `domain/check_result.rs` は `DoctorCheckResult`（`doctor_use_case.rs` 内に独自定義済み）と重複するデッドコード
  - `CommandRunner` ポートはすでに完全に定義・実装されており、`MockCommandRunner` のテストサポートも存在する

---

## リサーチログ

### 依存方向の違反箇所の調査

- **コンテキスト**: Issue #165 の指摘を基に、application 層から adapter 層への直接依存を調査
- **調査対象ファイル**:
  - `src/application/init_use_case.rs` — `use crate::adapter::outbound::init_file_generator::InitFileGenerator` および `use crate::adapter::outbound::sqlite_connection::SqliteConnection` を直接インポート
  - `src/application/stop_use_case.rs` — `use nix::sys::signal::{Signal, kill}; use nix::unistd::Pid;` を使用して `NixSignalSender` を同ファイルに実装
  - `src/application/doctor_use_case.rs` — `std::process::Command::new("git")` / `std::process::Command::new("gh")` を 5 箇所で直接使用（free 関数 `check_git`, `detect_gh_presence`, `check_gh_label`, `check_weight_labels` 内）
- **影響**:
  - application 層のユニットテストで adapter 具象型が必要になり、テスタビリティが損なわれる
  - bootstrap でない箇所で DI のバイパスが発生している

### 既存ポートの調査

- **調査対象**: `src/application/port/` 配下の全トレイト定義
- **発見**:
  - `command_runner.rs`: `CommandRunner` トレイトと `MockCommandRunner` が完全実装済み。`doctor_use_case.rs` から使われていないだけ
  - `pid_file.rs`: `PidFilePort` トレイトが正しく定義・利用されている（`StopUseCase` はすでにこれを使用）
  - `SignalPort` は `stop_use_case.rs` 内に定義されているが、`application/port/` ディレクトリには置かれていない。プロジェクトの慣例（ポートは `port/` 配下）に従い移動する
- **影響**: `DbInitializer` ポートと `FileGenerator` ポートの新規追加が必要

### エラー型の使用状況調査

- **コンテキスト**: `CupolaError`（`application/error.rs`）の実際の使用状況を確認
- **発見**:
  - `CupolaError` は `error.rs` で定義されているが、`init_use_case`・`stop_use_case`・`doctor_use_case` はいずれも `anyhow::Result<T>` を使用して `CupolaError` を使っていない
  - `StopUseCase` は独自の `StopError`（thiserror）を使用しており、anyhow への統一は対象外（thiserror によるポート固有エラー型として正当）
  - adapter 層の各実装は既に `anyhow::Context` / `anyhow::Result` を使用している

---

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク/制限 | 備考 |
|-----------|------|------|------------|------|
| ポート追加（`DbInitializer`, `FileGenerator`） | application/port/ に新規 trait を定義し、具象型を bootstrap で注入 | CA 準拠、テスタビリティ向上 | `InitUseCase` 引数が増える | プロジェクト標準パターン |
| `InitUseCase` を bootstrap 層に移動 | use case ごと bootstrap に移動して違反を回避 | ファイル数増加なし | use case ロジックが bootstrap に漏れる、CA 違反継続 | 採用しない |
| `NixSignalSender` を adapter/outbound に移動 | `SignalPort` はそのまま application 内に残し、実装のみ移動 | 最小変更 | `SignalPort` 定義場所が `port/` 配下でない点が残る | 移行として許容 |
| `SignalPort` を `application/port/signal.rs` に移動 | プロジェクト慣例に完全準拠 | 一貫性向上 | `stop_use_case.rs` から import パスが変わる | 推奨（NixSignalSender 移動と同時実施） |

---

## 設計判断

### 判断: `DbInitializer` / `FileGenerator` ポートの設計

- **コンテキスト**: `InitUseCase` が `SqliteConnection::init_schema()` と `InitFileGenerator` の 3 メソッドを直接呼び出している
- **検討した代替案**:
  1. `InitRepository` として一括ポート化 — `init_schema` + ファイル生成を 1 つのトレイトにまとめる
  2. `DbInitializer` と `FileGenerator` を分離 — DB 操作とファイル操作の責務を分離
- **選択**: オプション 2（分離）
- **根拠**: DB 初期化とファイル生成は独立した外部システムへの操作であり、別々のポートで抽象化することで各モック実装が単純になる。また実装の入れ替え単位が明確になる
- **トレードオフ**: `InitUseCase::new` に引数が 2 つ増えるが、bootstrap での DI コードで吸収できる
- **フォローアップ**: 各ポートの実装である `SqliteConnection` と `InitFileGenerator` がトレイトを実装するかどうかを確認する

### 判断: `SignalPort` の移動

- **コンテキスト**: `SignalPort` は `stop_use_case.rs` 内に定義されており、`application/port/` には置かれていない
- **選択**: `SignalPort` を `application/port/signal.rs` に移動し、`NixSignalSender` を `adapter/outbound/nix_signal_sender.rs` に移動
- **根拠**: プロジェクト慣例では全ポートを `application/port/` に置く。`stop_use_case.rs` は `SignalPort` をインポートして使用するだけにすることで、他のポートと一貫した構造になる
- **トレードオフ**: ファイルが 2 つ追加されるが、責務の明確化に貢献する

### 判断: `DoctorUseCase` での `CommandRunner` 注入方式

- **コンテキスト**: `check_git`・`detect_gh_presence`・`check_gh_label`・`check_weight_labels` はすべて free 関数として定義されており、各関数内で `std::process::Command` を呼び出している
- **選択**: `CommandRunner` を `DoctorUseCase` のフィールドとして保持し、各 check 関数に `&dyn CommandRunner` を渡す
- **根拠**: 既存の `DoctorUseCase<C: ConfigLoader>` の構造を維持し、新たに `CommandRunner` フィールドを追加する設計が最小変更で済む。`DoctorUseCase<C: ConfigLoader, R: CommandRunner>` とする
- **トレードオフ**: 型パラメータが 1 つ増えるが、Rust の generics で問題なく表現できる

### 判断: `anyhow` 統一の範囲

- **コンテキスト**: `StopError` は `thiserror` による型付きエラーで、ポート固有エラーとして適切
- **選択**: `StopError` と `PidFileError` および `ConfigLoadError` は thiserror のままにし、use case の戻り値型を `anyhow::Result<T>` に統一することに留める。`CupolaError` のみ削除する
- **根拠**: Issue #165 の指摘通り「型付きエラー設計は別 issue (#166) で行う」という方針に従う。現時点では `CupolaError` の削除と、adapter 層での `anyhow::context` の徹底が目的

---

## リスクと緩和策

- **既存テストの破損**: `InitUseCase` のテストは `SqliteConnection` と `InitFileGenerator` を直接使用している。ポート化後はモック実装か、あるいは統合テストとして維持するかを検討する必要がある — 緩和策: 統合テストとして維持し、ポート実装の組み合わせテストとして再位置づけする
- **bootstrap の変更範囲**: `StopUseCase::new(pid_file, Duration)` が `NixSignalSender` を内部生成しているため、bootstrap での呼び出し方を変更する必要がある — 緩和策: `StopUseCase::with_signal_sender` がすでに存在するのでそちらに切り替えるか、`new` コンストラクタを bootstrap 層でのみ使うよう制限する
- **コンパイル連鎖**: 複数ファイルを同時に変更するとコンパイルエラーが連鎖しやすい — 緩和策: タスクを小さな単位（1 ファイル改修 + ビルド確認）に分割する

---

## 参考資料

- Clean Architecture 依存ルール（`.cupola/steering/tech.md`）
- プロジェクト構造規約（`.cupola/steering/structure.md`）
- Issue #165 — 本 issue（依存解消の方針）
- Issue #166 — フェーズ2: 型付きエラー設計（本 issue のスコープ外）
