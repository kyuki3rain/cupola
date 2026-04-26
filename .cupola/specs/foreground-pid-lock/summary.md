# foreground-pid-lock サマリー

## Feature
`foreground` モードに PID ファイル二重起動防止を追加。daemon と foreground で同一チェックロジックを共有し全組み合わせで相互排他を保証。

## 要件サマリ
1. foreground 起動時に PID ファイルを読み、生存プロセスがあれば `already running` エラー。ゾンビ PID は削除して継続。ファイルなしは通常起動。
2. PID チェック通過後に自プロセス PID を `<config_dir>/cupola.pid` に書き込む。失敗時は起動中断。
3. foreground 終了時（正常/エラー問わず）に PID ファイルを削除。削除失敗は握りつぶし元の結果を返す。
4. daemon ↔ foreground の全組み合わせで相互排他（同一パスを共有）。

## アーキテクチャ決定
- `start_daemon` のインライン PID チェックを共有ヘルパー関数 `check_and_clean_pid_file(pid_manager) -> Result<()>` として抽出し、`start_daemon` / `start_foreground` 双方から呼ぶ（DRY、Issue 要件「同じロジック共有」に直接対応）。
- 変更範囲は `src/bootstrap/app.rs` 単一ファイルに限定し、`PidFilePort` / `PidFileManager` / `apply_pid_cleanup` / `with_pid_file` の既存コンポーネントをそのまま再利用。
- foreground も `with_pid_file` + `apply_pid_cleanup` の二重保護パターンを採用（`daemon_child` と同じ）。
- PID 書き込みはロギング初期化前に行い `daemon_child` と一貫させる。
- TOCTOU 完全排除は Non-Goal（ポーリング間隔特性上実用リスク低）。

## コンポーネント
- `src/bootstrap/app.rs` 内
  - `check_and_clean_pid_file`（新規プライベートヘルパー）
  - `start_foreground`（PID 保護追加）
  - `start_daemon`（ヘルパー呼び出しへ置換）

## 主要インターフェース
- `fn check_and_clean_pid_file(pid_file_manager: &PidFileManager) -> Result<()>`

## 学び/トレードオフ
- 「直接コピー」案は DRY 違反で不採用、共有ヘルパー抽出で既存 daemon コードも整理される副次効果あり。
- `with_pid_file` に所有権を渡した後 `apply_pid_cleanup` にはパスのみ渡す制約がある。
- エラーメッセージは daemon の `"cupola daemon is already running"` と foreground の `"cupola is already running"` いずれも許容。
