# config-validation

## Feature
`Config::validate()` を拡張し、文字列フィールドの空チェック、`polling_interval_secs` / `stall_timeout_secs` の絶対下限と相関チェック、`log_dir` の必須化を追加。さらに `Config.log_dir` の型を `Option<PathBuf>` → `PathBuf` に変更しデフォルト値 `".cupola/logs"` を持たせる。不正な設定での起動を早期に拒否する。Issue #130。

## 要件サマリ
- 文字列フィールド（`owner`/`repo`/`default_branch`/`language`/`model`）が空文字列ならエラー。評価順は定義順で早期リターン。
- `polling_interval_secs < 10` でエラー（秒/分取り違え対策、API rate limit 浪費防止）。
- `stall_timeout_secs < 60` でエラー、かつ `stall_timeout_secs > polling_interval_secs` を要求（相関チェック）。等値は NG。
- 相関チェックは両絶対下限を満たした後のみ評価（誤解を招くメッセージを避ける）。
- `log_dir` の型変更: `Option<PathBuf>` → `PathBuf`、default `".cupola/logs"`。空パスはエラー。`bootstrap/app.rs` の `is_none()` チェック削除、`init_logging(&Path)` に変更、戻り値を `WorkerGuard` に変更（常にファイルログ）。
- 既存 `max_concurrent_sessions == Some(0)` チェックを維持、新チェックと共存。
- エラーメッセージは英語でフィールド名と下限値を含める。

## アーキテクチャ決定
- **早期リターン継続**: エラー収集方式（Vec 蓄積）ではなく既存の早期リターンを踏襲。Requirement 1.6 で明示されている、かつ相関チェックの前提条件ガードを単純化できる。設定ミスは通常 1 箇所のため実用上の不便は小さい。
- **相関チェックのガード**: `stall <= polling` の比較は両絶対下限通過後のみ実施。絶対下限違反時に相関エラーを出すと修正順序が混乱するため。
- **5 フィールドの空チェック拡張**: requirements.md の Requirement 1 は当初 owner/repo 中心だったが、Issue #130 の意図に従い default_branch / language / model も同列にチェック対象化。設計段階でスコープ揃えをして実装の手戻りを防止。
- **空文字列判定に `trim()` を行わない**: 設定ファイルに空白文字列を書いた場合はユーザーの意図とみなす（過度な親切は避ける）。
- **`log_dir` の `Option` 廃止**: 空チェックを `validate()` に持ち込むなら、`Option` 経由の `None` パスを残すのは意味論が揺れるため、型自体を `PathBuf` に変更しデフォルト値を持たせる設計へ。daemon 起動時の nil チェックと `init_logging` の分岐も同時に除去できる。
- **評価順序**: log_dir → owner → repo → default_branch → language → model → polling → stall 絶対下限 → stall 相関 → max_concurrent_sessions。相関チェックは絶対下限通過後。
- **domain 層の純粋性維持**: `log_dir` のディレクトリ実在チェックは I/O のため `validate()` スコープ外。空パスチェックのみ。`max_retries` は `u32` 型保証、`log_level` は enum 保証で型レベル妥当性を確保。
- **スコープ**: 複数エラー同時収集、ディレクトリ実在確認、`max_retries` チェックはすべて Non-Goal。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `Config::validate` | domain | 全フィールドの妥当性検証（単一メソッド拡張） |
| `Config.log_dir` 型変更 | domain | `Option<PathBuf>` → `PathBuf`、default `".cupola/logs"` |
| `config_loader::into_config` | bootstrap | `log.dir` 未設定時のデフォルト値適用 |
| `bootstrap/app.rs`（更新） | bootstrap | daemon 起動時の `is_none()` 分岐削除 |
| `bootstrap/logging.rs::init_logging` | bootstrap | 引数 `Option<&Path>` → `&Path`、戻り値 `WorkerGuard` |

## 主要インターフェース
- `impl Config { pub fn validate(&self) -> Result<(), String>; }`（純粋関数、I/O なし）
- エラーメッセージ例:
  - `"owner must not be empty"`
  - `"polling_interval_secs must be at least 10"`
  - `"stall_timeout_secs must be at least 60"`
  - `"stall_timeout_secs must be greater than polling_interval_secs"`
  - `"log_dir must not be empty"`
- `init_logging(log_dir: &Path) -> WorkerGuard`

## 学び / トレードオフ
- デフォルト値（`polling_interval_secs=60`, `stall_timeout_secs=1800`）は全て新下限を満たすため、既存ユーザーへのデグレはない。
- 既存テストが使っていた `"o"` / `"r"` / `"main"` などのダミー値はすべて非空文字列のため、リグレッションなし。
- 早期リターンだとユーザーが 1 回の起動で複数エラーを確認できないが、設定ミスは通常単発で実害が小さい。将来的に UX 要求が高まれば `Vec<ValidationError>` 方式への移行が可能。
- `stall_timeout_secs > polling_interval_secs` を要求することで、stall 検出が 1 ポーリングサイクル未満で発火する病理ケースを排除。
- `log_dir` の型変更は波及範囲が広く（domain / bootstrap / logging）、`Option` のパスを残したままチェック追加する案より一貫性が高い判断だが、周辺コードの同時変更が必須。
