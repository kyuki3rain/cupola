# Research & Design Decisions

---
**Purpose**: discovery findings, architectural investigations, and rationale that inform the technical design.

---

## Summary

- **Feature**: issue-262 — パニック時 PID ファイル自動クリーンアップ
- **Discovery Scope**: Extension（既存 bootstrap/app.rs への機能追加）
- **Key Findings**:
  - `std::panic::set_hook` は `'static + Fn(&PanicHookInfo) + Send + Sync` なクロージャを要求するため、`PathBuf` を move キャプチャするだけで実装可能
  - `PidFileManager` は `pid_file_path: PathBuf` のみ保持するため、hook 内でクローン済みパスから新たな `PidFileManager` を生成できる
  - フォアグラウンド・デーモンの 2 モードで PID ファイル書き込み後にそれぞれ hook 設定が必要
  - tracing が初期化される前に panic が起きた場合も考慮が必要

## Research Log

### std::panic::set_hook の制約と移譲パターン

- **Context**: panic hook に PID ファイルパスを渡す方法の検討
- **Findings**:
  - `std::panic::take_hook()` で現在のデフォルトフックを取得し、クロージャ内でそれを呼び出すパターンで再伝播が実現できる
  - hook のクロージャは `'static` を要求するため、参照は使えない。`PathBuf` を move キャプチャする方法が最も単純
  - `catch_unwind` は panic hook を呼ばないため、ユニットテストでは hook 関数自体を直接呼び出すアプローチが有効
- **Implications**: `install_panic_hook(pid_path: PathBuf)` として `PathBuf` を受け取り、hook 内で `PidFileManager` を生成するシンプルな実装が適切

### tracing 未初期化時の panic ログ

- **Context**: `start_foreground` / `start_daemon_child` では PID ファイル書き込みの後に logging を初期化している。hook 設定時点では tracing subscriber が未設定の可能性がある
- **Findings**:
  - `tracing::error!` はサブスクライバ未登録時は no-op となるため、クラッシュ等は発生しない
  - ただし PID ファイル書き込み直後から logging 初期化前の窓はごく短く、実際には panic が起こりにくい
  - `eprintln!` を tracing と併用することで、サブスクライバ未登録時でも stderr に記録できる
- **Implications**: `tracing::error!` と `eprintln!` の両方を使用する

### 既存の `apply_pid_cleanup` との関係

- **Context**: `start_foreground` / `start_daemon_child` は既に `apply_pid_cleanup` を呼び出している
- **Findings**:
  - `apply_pid_cleanup` は `Result` パスで呼び出されるため、**正常終了・エラー終了**では PID ファイルを削除できる
  - しかし **panic** は `Result` パスを通らないため、`apply_pid_cleanup` は呼び出されない
  - panic hook による削除は `apply_pid_cleanup` と重複して呼ばれる可能性があるが、`delete_pid()` は「ファイルが存在しない場合は Ok」を返すため二重呼び出しは安全
- **Implications**: panic hook の追加は既存のクリーンアップパスを置き換えるものではなく補完するもの

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| PathBuf move キャプチャ | `PathBuf` を hook クロージャへ move | シンプル、`'static` 要件を満たす | なし | 採用 |
| `Arc<PidFilePort>` | trait object を Arc でラップして共有 | テスタビリティが高い | 過剰設計 | 不採用 |
| `Arc<PathBuf>` | パスを Arc 共有 | 若干のオーバーヘッド節約 | 可読性が低下 | 不採用 |

## Design Decisions

### Decision: install_panic_hook 関数の配置

- **Context**: hook 設定ロジックをどこに置くか
- **Alternatives Considered**:
  1. `bootstrap/app.rs` のプライベート関数 — 呼び出し元が同ファイルのため自然
  2. 新規モジュール `bootstrap/panic_hook.rs` — 分離が明確
- **Selected Approach**: `bootstrap/app.rs` のプライベート関数 `install_panic_hook(pid_path: PathBuf)` として実装
- **Rationale**: 変更対象が 1 ファイルのみで小規模。新モジュールを追加するほどの複雑さはない
- **Trade-offs**: コードが app.rs にまとまる一方、将来的な拡張には分離が望ましい

### Decision: デフォルト hook の保存と再呼び出し

- **Context**: panic の標準動作（スタックトレース出力）を失わないようにする
- **Selected Approach**: `std::panic::take_hook()` でデフォルト hook を取得し、クロージャに move キャプチャして hook 内で呼び出す
- **Rationale**: 既存のデフォルト動作を完全に保持しつつ追加処理を挿入できる最も安全なパターン
- **Trade-offs**: `take_hook` → `set_hook` の間は一時的に hook が設定されていない瞬間があるが、シングルスレッドで初期化するため実用上問題なし

## Risks & Mitigations

- hook 内での再 panic — `delete_pid()` の実装は I/O エラーを `PidFileError` として返すのみで内部で panic しない設計のため問題なし
- 複数回の hook 設定 — `install_panic_hook` は起動時に 1 回だけ呼ばれるため問題なし
- ロギング未初期化 — `eprintln!` フォールバックで対処

## References

- [std::panic::set_hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html)
- [std::panic::take_hook](https://doc.rust-lang.org/std/panic/fn.take_hook.html)
- Issue #258 — signal 送信失敗時の PID ファイル残存（類似ルートケース）
