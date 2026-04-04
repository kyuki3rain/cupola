# 要件定義書

## はじめに

本仕様書は、cupola プロジェクトにおける Clean Architecture 違反の修正と `anyhow::Result` によるエラー型統一を対象とするリファクタリングの要件を定義する。

現在、`application` 層が `adapter` 層の具象型（`SqliteConnection`・`InitFileGenerator`・`NixSignalSender`）に直接依存しており、Clean Architecture の依存ルール（依存は内側に向かってのみ）に違反している。また `CupolaError` が定義されているにもかかわらずほぼ未使用であり、`anyhow` と `thiserror` が混在した中途半端なエラー戦略となっている。

本フェーズでは全違反を解消して見通しの良い状態を作る。型付きエラー設計は別 issue (#166) で実施する。

---

## 要件

### 要件 1: 未使用コードの削除

**目的:** 開発者として、使われていない型とファイルを削除したい。コードベースの見通しが良くなり、誤った依存の温床をなくせるから。

#### 受け入れ基準

1. The Cupola shall 未使用の `src/application/error.rs`（`CupolaError` 定義ファイル）をコードベースから削除する。
2. The Cupola shall 未使用の `src/domain/check_result.rs`（`DoctorCheckResult` と重複するデッドコード）をコードベースから削除する。
3. When `CupolaError` または `check_result` モジュールへの参照が他ファイルに残っている場合, the Cupola shall それらの参照もすべて削除しビルドエラーが発生しない状態にする。
4. The Cupola shall 上記削除後に `cargo build` が警告なしで成功する。

---

### 要件 2: `init_use_case.rs` の adapter 依存解消

**目的:** 開発者として、`init_use_case` が adapter 層の具象型に直接依存しない状態にしたい。ポートによる抽象化でテスタビリティと層分離が保たれるから。

#### 受け入れ基準

1. The Cupola shall `application/port/` に `DbInitializer` トレイト（または同等の名称）を定義し、スキーマ初期化操作を抽象化する。
2. The Cupola shall `application/port/` に `FileGenerator` トレイト（または同等の名称）を定義し、TOML テンプレート生成・steering コピー・gitignore 更新操作を抽象化する。
3. The Cupola shall `src/application/init_use_case.rs` から `SqliteConnection` および `InitFileGenerator` のインポートと直接使用を削除する。
4. When `InitUseCase` が実行される場合, the Cupola shall ポートトレイト経由のみで DB 初期化とファイル生成を行う。
5. The Cupola shall `bootstrap` 層で `SqliteConnection` と `InitFileGenerator` の具象型をポートとして注入する。
6. The Cupola shall `SqliteConnection` と `InitFileGenerator` が上記新ポートトレイトを実装する。

---

### 要件 3: `stop_use_case.rs` の `NixSignalSender` を adapter 層へ移動

**目的:** 開発者として、OS シグナル送信の実装を adapter 層に置きたい。application 層には `nix` クレートへの直接依存を持ち込まないようにするから。

#### 受け入れ基準

1. The Cupola shall `SignalPort` トレイト（シグナル送信の抽象）を `application/port/` に保持したまま、`NixSignalSender` の実装を `src/adapter/outbound/` に移動する。
2. The Cupola shall `src/application/stop_use_case.rs` から `nix` クレートのインポートおよび `NixSignalSender` 構造体定義を削除する。
3. The Cupola shall `bootstrap` 層で `NixSignalSender` を `SignalPort` として `StopUseCase` に注入する。
4. When `stop` コマンドが実行された場合, the Cupola shall 既存と同じ動作（PID ファイル読み込み → シグナル送信 → PID ファイル削除）を維持する。

---

### 要件 4: `doctor_use_case.rs` の `CommandRunner` ポート使用

**目的:** 開発者として、`doctor_use_case` 内の `std::process::Command` 直接呼び出しを `CommandRunner` ポート経由に置き換えたい。既存ポートを活用してテスタビリティを高めるから。

#### 受け入れ基準

1. The Cupola shall `src/application/doctor_use_case.rs` から `std::process::Command` の全直接使用（git・gh コマンド呼び出し等）を削除する。
2. The Cupola shall `DoctorUseCase` が `CommandRunner` ポートをフィールドとして受け取るよう変更する。
3. When `doctor` コマンドが実行された場合, the Cupola shall `CommandRunner` トレイト経由でのみ外部コマンドを呼び出す。
4. The Cupola shall `MockCommandRunner` を使った単体テストで `doctor_use_case` の動作を検証できる状態にする。
5. The Cupola shall `bootstrap` 層で `ProcessCommandRunner` を `CommandRunner` として `DoctorUseCase` に注入する。

---

### 要件 5: 全体エラー型を `anyhow::Result` に統一

**目的:** 開発者として、各層のエラー伝播を `anyhow::Result` に統一したい。エラー戦略を単純化し次フェーズ（型付きエラー）への移行準備を整えるから。

#### 受け入れ基準

1. The Cupola shall adapter 層の各アウトバウンドアダプタが外部エラーを `anyhow::Error` に `.context()` または `.with_context()` で変換して返す。
2. The Cupola shall application 層の use case が `anyhow::Result<T>` を戻り値型として使用する（`CupolaError` ではなく）。
3. If 外部ライブラリ固有のエラー型が application 層に漏れている場合, the Cupola shall adapter 層でラップして `anyhow::Error` として返す。
4. The Cupola shall `src/application/port/` に定義されたトレイトのメソッド戻り値型が `anyhow::Result<T>` を使用する（ただしポート固有エラー型が明確に必要な場合はこの限りでない）。
5. The Cupola shall `cargo build` および `cargo clippy -- -D warnings` が全違反修正後に警告なしで成功する。

---

### 要件 6: ビルド・テストの継続的な成功

**目的:** 開発者として、リファクタリング中もビルドとテストが常に通る状態を維持したい。デグレを防ぎ、CI が緑を保てるから。

#### 受け入れ基準

1. The Cupola shall 各タスク完了後に `cargo build` が成功する。
2. The Cupola shall 各タスク完了後に `cargo test` が全テストパスで成功する。
3. The Cupola shall `cargo clippy -- -D warnings` が警告なしで成功する。
4. If 既存のテストがリファクタリングによって壊れた場合, the Cupola shall 同等の動作を保証するようテストを更新する。
5. While テストが存在しない use case が改修対象である場合, the Cupola shall 最低限のユニットテストを追加してから修正を行う。
