# cupola-init-extension

## Feature
`cupola init` コマンドを拡張し、SQLite スキーマ初期化に加えて (1) `cupola.toml` 雛形生成、(2) `.cupola/steering/` テンプレートコピー、(3) `.gitignore` エントリ追記を単一コマンドで自動化。既存ファイルを上書きしない冪等設計。

## 要件サマリ
- 既存の SQLite スキーマ初期化を維持（`CREATE TABLE IF NOT EXISTS` による冪等動作）。
- `.cupola/cupola.toml` 未存在時のみ雛形を生成。必須 3 フィールド（`owner`/`repo`/`default_branch`）は空欄、オプションフィールドはコメントアウトで提示。
- `.cupola/settings/templates/steering/` にテンプレートがあり、かつ `.cupola/steering/` が空の場合のみ `product.md` / `structure.md` / `tech.md` をコピー。テンプレート不在（cc-sdd 未インストール）はログ出力してスキップ。
- `.gitignore` にマーカー `# cupola` が未存在なら cupola 管理ブロックを追記。ファイル不在時は新規作成。
- 何度実行しても破壊的変更なし。2 回目以降は全ステップがスキップされ正常終了。
- 対話入力・cc-sdd インストール・GitHub ラベル作成はスコープ外。

## アーキテクチャ決定
- **`InitUseCase` を application 層に抽出**: 代替案は `app.rs` 内インライン拡張。インラインは変更最小だが責務肥大化とテスト困難。Clean Architecture 準拠のため `InitUseCase` を新設し、SQLite 初期化も含めて統括。
- **ファイル操作の抽象化レベル**: `FileSystem` ポート定義せず `std::fs` を adapter 層内で直接使用。init は単純なファイル操作のみで、過度な抽象化はオーバーヘッドに見合わないと判断。ユニットテストは tempdir ベース、統合テストでカバー。後続仕様 `clean-arch-refactor` で `FileGenerator` ポートとして抽象化されることになる（この時点では未導入）。
- **`.gitignore` 重複検出**: 行単位チェックではなくマーカーコメント `# cupola` によるブロック単位検出を採用。ユーザーが手動編集しても安全で、シンプル。ユーザーがマーカーを削除した場合に再追記される副作用は許容。
- **ステップ間の独立性**: 各ステップは独立して冪等。失敗は fail-fast（anyhow::Result で伝播）とし、部分的初期化状態は再実行で回復。SQLite 初期化を最初に実行して既存動作を最優先維持。
- **`InitReport` 戻り値**: 各ステップの実行/スキップを `bool` フラグで集約。bootstrap 側のメッセージ出力に利用。
- **steering テンプレートの対象**: `product.md` / `structure.md` / `tech.md` の標準 3 ファイルのみ。`steering-custom/` は別管理でスコープ外。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `InitUseCase` | application | init の 5 ステップ（dir 作成 / SQLite / toml / steering / gitignore）を順次実行し `InitReport` を返す |
| `InitFileGenerator` | adapter/outbound | cupola.toml 生成、steering コピー、gitignore 追記の各冪等ファイル操作 |
| `SqliteConnection`（既存） | adapter/outbound | `init_schema()` によるスキーマ初期化（変更なし） |
| `bootstrap/app.rs`（更新） | bootstrap | `Command::Init` のインライン実装を `InitUseCase::run()` に置き換え、`InitReport` を元に完了メッセージを出力 |

## 主要インターフェース
```rust
pub struct InitUseCase { base_dir: PathBuf }
impl InitUseCase {
    pub fn new(base_dir: PathBuf) -> Self;
    pub fn run(&self) -> Result<InitReport>;
}

pub struct InitReport {
    pub db_initialized: bool,
    pub toml_created: bool,
    pub steering_copied: bool,
    pub gitignore_updated: bool,
}

pub struct InitFileGenerator { base_dir: PathBuf }
impl InitFileGenerator {
    pub fn generate_toml_template(&self) -> Result<bool>;
    pub fn copy_steering_templates(&self) -> Result<bool>;
    pub fn append_gitignore_entries(&self) -> Result<bool>;
}
```

### cupola.toml 雛形
```toml
owner = ""
repo = ""
default_branch = ""

# language = "ja"
# polling_interval_secs = 60
# max_retries = 3
# stall_timeout_secs = 1800
# max_concurrent_sessions = 3

# [log]
# level = "info"
# dir = ".cupola/logs"
```

### .gitignore ブロック
```
# cupola
.cupola/cupola.db
.cupola/cupola.db-wal
.cupola/cupola.db-shm
.cupola/logs/
.cupola/worktrees/
.cupola/inputs/
```
（WAL/SHM ファイル、worktrees、inputs、logs を含む）

## 学び / トレードオフ
- `std::fs` を直接使う判断は、後の clean-arch-refactor で Clean Architecture 違反として指摘され `FileGenerator` ポート化されることになる。当初は「単純なファイル操作」として許容したが、use case 側が adapter 具象に依存する構造を作ってしまった点は学び。
- マーカーコメント方式の gitignore 管理は、将来エントリ追加の差分更新が必要になった場合に「マーカーで囲まれたブロックを再生成する」ための基礎として機能する。
- テンプレートディレクトリパスのハードコード（`.cupola/settings/templates/steering/`）は cc-sdd との結合部。cc-sdd 側の変更があれば追従が必要。
- `InitReport` で各ステップの実行/スキップを可視化することで、ユーザーは複数回 init しても「何が行われたか」を明示的に確認できる。
- fail-fast 方針は init の途中失敗で部分的ファイル群が残る可能性があるが、冪等設計により再実行で修正可能。
- 並行実行は通常想定しないためファイル競合リスクは最小化。
