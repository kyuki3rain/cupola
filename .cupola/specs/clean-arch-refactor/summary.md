# clean-arch-refactor

## Feature
Cupola の Clean Architecture 依存ルール違反を一括解消し、use case の戻り値型を `anyhow::Result` に統一するリファクタリング。型付きエラー設計は後続 Issue #166 に委ね、本仕様では「見通しの良い状態」を作ることにフォーカス。Issue #165。

## 要件サマリ
- **未使用コード削除**: `src/application/error.rs`（`CupolaError`）と `src/domain/check_result.rs`（`DoctorCheckResult` と重複するデッドコード）を削除し、参照も全除去。
- **`InitUseCase` のポート化**: `SqliteConnection` / `InitFileGenerator` への直接依存を排除し、新規 `DbInitializer` / `FileGenerator` ポートを `application/port/` に追加。bootstrap で具象型を注入。
- **`NixSignalSender` の adapter 移動**: `nix` クレート依存を application 層から排除。`SignalPort` を `application/port/signal.rs` へ、実装を `adapter/outbound/nix_signal_sender.rs` へ移動。
- **`DoctorUseCase` の CommandRunner 注入**: `std::process::Command` の直接使用 5 箇所を既存 `CommandRunner` ポート経由に置換。`MockCommandRunner` でテスト可能に。
- **エラー型統一**: use case の戻り値を `anyhow::Result<T>` に統一。adapter 層は `.context()` / `.with_context()` で外部エラーを anyhow にラップ。ただし `StopError` / `PidFileError` / `ConfigLoadError` は port 固有エラーとして thiserror のまま維持。
- `cargo build` / `cargo test` / `cargo clippy --all-targets -- -D warnings` を全て警告なしで成功させる。

## アーキテクチャ決定
- **`DbInitializer` と `FileGenerator` の分離**: 一括 `InitRepository` でなく 2 つに分離。DB 初期化とファイル生成は独立外部システムへの操作であり、モック実装が単純になる。`InitUseCase::new` の引数が増えるが bootstrap で吸収。
- **`SignalPort` 位置の統一**: `stop_use_case.rs` 内に定義されていた `SignalPort` を `application/port/signal.rs` に移動。他ポートと慣例統一。`NixSignalSender` 移動と同時実施。
- **`DoctorUseCase` の型パラメータ方式**: `DoctorUseCase<C: ConfigLoader>` を `DoctorUseCase<C: ConfigLoader, R: CommandRunner>` に拡張。既存構造を保ちながら最小変更で CommandRunner を注入。check 関数は `&dyn CommandRunner` を受け取る free 関数形式を維持。
- **エラー統一の範囲限定**: `anyhow` は use case 戻り値と adapter エラーラッピングに限定。`StopError` など type-safe な port 固有エラーは維持。これは Issue #165 の「型付きエラー設計は #166 で実施」方針に従うため。
- **`std::fs::create_dir_all` は許容**: ファイルシステム基本操作はポート抽象化対象外とする（過度な抽象化回避）。
- **`InitUseCase::new(base_dir)` → `InitUseCase::new(base_dir, db_init, file_gen)` への非互換変更**: bootstrap と既存テストを同時更新することで吸収。
- **既存テストの方針**: `InitUseCase` の既存テストは `SqliteConnection` + `InitFileGenerator` の具象を使う統合テストとして維持。モック化は必須ではない。
- **段階的移行**: フェーズ 1 削除 → フェーズ 2 Signal 関連 → フェーズ 3 Doctor → フェーズ 4 Init → フェーズ 5 最終確認、の順でコンパイル連鎖を最小化。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `DbInitializer` trait | application/port | DB スキーマ初期化の抽象 |
| `FileGenerator` trait | application/port | TOML テンプレート / steering コピー / gitignore 更新の抽象 |
| `SignalPort` trait（移動） | application/port | OS シグナル送信の抽象 |
| `InitUseCase<D, F>` | application | ポート依存化された init ユースケース |
| `StopUseCase<P, S>` | application | `SignalPort` の import パス更新のみ |
| `DoctorUseCase<C, R>` | application | `CommandRunner` を注入された doctor ユースケース |
| `SqliteConnection`（拡張） | adapter/outbound | `DbInitializer` を実装 |
| `InitFileGenerator`（拡張） | adapter/outbound | `FileGenerator` を実装 |
| `NixSignalSender`（移動） | adapter/outbound | `SignalPort` の nix クレート実装 |

## 主要インターフェース
```rust
// application/port/db_initializer.rs
pub trait DbInitializer: Send + Sync {
    fn init_schema(&self) -> anyhow::Result<()>;
}

// application/port/file_generator.rs
pub trait FileGenerator: Send + Sync {
    fn generate_toml_template(&self) -> anyhow::Result<bool>;
    fn copy_steering_templates(&self) -> anyhow::Result<bool>;
    fn append_gitignore_entries(&self) -> anyhow::Result<bool>;
}

// application/port/signal.rs（移動）
pub trait SignalPort: Send + Sync {
    fn send_sigterm(&self, pid: u32) -> Result<(), StopError>;
    fn send_sigkill(&self, pid: u32) -> Result<(), StopError>;
}
```
- `InitUseCase::new(base_dir, db_init, file_gen)`
- `DoctorUseCase::new(config_loader, command_runner)`
- `StopUseCase::with_signal_sender(pid_file, NixSignalSender, Duration)` を bootstrap で使用（内部で NixSignalSender を生成する `new` は削除）

## 学び / トレードオフ
- `FileGenerator` の 3 メソッドは意味的には別操作だが、いずれも init でのみ使用される小さな集合のため 1 トレイトに集約。後で分解が必要になった場合は比較的容易。
- `InitUseCase` のテストを統合テストとして維持する方針は、ポート実装と具象実装の組み合わせ検証としての価値を再定義した判断。モック化は doctor / stop 側にとどめた。
- `StopError` 等の type-safe エラーを維持したことで、`anyhow` 化が中途半端に見えるが、「型付きエラーは #166」という明確な線引きで説明できる。
- `DoctorUseCase` に generic parameter を追加すると呼び出し側（bootstrap）の型記述が増えるが、Rust の型推論とコンストラクタ DI でほとんど隠蔽できる。
- リファクタリング中のコンパイル連鎖を避けるため、タスクを「1 ファイル改修 + ビルド確認」の細粒度に分解する運用が有効。
- CI ゲートとして `-D warnings` を通すことでリファクタリング中のデッドコード警告・未使用 import の残骸を確実に検出できる。
