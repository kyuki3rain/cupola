# clear-inputs-before-spawn

## Feature
`PollingUseCase::prepare_inputs` の冒頭で worktree 下 `.cupola/inputs/` を削除・再作成し、前回 Fixing で書き込まれた古い入力ファイル（例: `review_threads.json`）が次の spawn で Claude Code に誤読されるバグを修正する。

## 要件サマリ
- `prepare_inputs` 実行時、書き込み前に `.cupola/inputs/` 内の全ファイルを削除。
- ディレクトリ不存在時も正常完了し、空ディレクトリを再作成。
- クリア失敗時はエラーログを出し呼び出し元へエラー伝播、既存 `tracing::warn!` + `continue` パターンで当該 Issue をスキップ。
- クリア後は現在の `fixing_causes` / `State` に対応するファイルのみが存在:
  - `DesignRunning` → `issue.md`
  - `DesignFixing`/`ImplementationFixing` + `ReviewComments` → `review_threads.json`
  - 同 + `CiFailure` → `ci_errors.txt`
  - 同 + `Conflict` → `conflict_info.txt`
  - `fixing_causes` 空 → 後方互換で `review_threads.json`
- 削除対象は worktree 配下の `.cupola/inputs/` のみに限定。worktree 外に影響を与えない。
- 冪等かつ空ディレクトリでもエラーを発生させない。

## アーキテクチャ決定
- **関数配置**: `polling_use_case.rs` にインライン実装するのではなく、既存 `application/io.rs` に `clear_inputs_dir` を新設。理由: ファイル I/O ロジックを use case に漏らさず Clean Architecture の application レイヤー純粋性を維持、既存の `write_*_input` 集約パターンと一貫。
- **クリア手法**: 「個別ファイル列挙削除」ではなく `remove_dir_all` + `create_dir_all` を採用。理由: シンプルで確実、将来 inputs 配下にサブディレクトリが追加されても拡張不要。`ErrorKind::NotFound` のみ握りつぶし、それ以外は `with_context` でラップして伝播。
- **呼び出し位置**: `prepare_inputs` 内の `match issue.state { ... }` の直前に 1 行 `clear_inputs_dir(wt)?` を追加するだけの最小差分。既存 `write_*_input` の冗長な `create_dir_all` 呼び出しはそのまま残す（副作用なし）。

## コンポーネント
| Component | Layer | Intent |
|-----------|-------|--------|
| `clear_inputs_dir` | application/io.rs | `.cupola/inputs/` の削除・再作成 |
| `PollingUseCase::prepare_inputs`（更新） | application | クリア後に State/causes に応じたファイルを書き込み |

## 主要インターフェース
```rust
/// `.cupola/inputs/` を削除して空の状態で再作成する。
/// NotFound は無視、それ以外のエラーは伝播。
pub fn clear_inputs_dir(worktree_path: &Path) -> Result<()>
```
- 呼び出し: `prepare_inputs` 冒頭 `clear_inputs_dir(wt)?`

## 学び / トレードオフ
- `prepare_inputs` が `fixing_causes` に応じて必要ファイルだけ書き込む設計は、古いファイルを削除する責務を持たなかったため、state 遷移の連続実行で残留が発生する欠陥があった。書き込み側でなくクリア側に責務を集約することで、将来 writer 関数が増えても自動で恩恵を受けられる。
- `remove_dir_all` + `create_dir_all` 方式は、ユーザーが手動で置いたファイルがあった場合も問答無用で削除してしまうが、inputs ディレクトリはツール管理領域であり許容。
- `fixing_causes` が空のフォールバックで `review_threads.json` を書き出す既存挙動は後方互換のため維持。将来 fixing_causes の空状態が完全に排除されれば削除できる。
- `tempfile::TempDir` を使うユニットテストパターンを踏襲し、既存テストの体裁と一貫させる。
