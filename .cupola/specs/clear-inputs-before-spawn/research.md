# Research & Design Decisions

---
**Purpose**: 設計判断の根拠と調査結果を記録する。

---

## Summary
- **Feature**: `clear-inputs-before-spawn`
- **Discovery Scope**: Extension（既存システムへの拡張）
- **Key Findings**:
  - `prepare_inputs` は `fixing_causes` に含まれる種類のファイルのみを書き込み、他のファイルを削除しないため、前回の Fixing で書かれた残留ファイルが問題を起こす
  - `io.rs`（application レイヤー）に全ての入力ファイル書き込み関数が集約されており、クリア関数も同ファイルに追加するのが一貫性のある設計
  - 各 `write_*_input` 関数は既に `create_dir_all` を呼ぶが、クリア処理はその前に一度だけ行う必要がある

## Research Log

### prepare_inputs の既存実装分析
- **Context**: バグの根本原因を理解するため実装を調査
- **Sources Consulted**: `src/application/polling_use_case.rs:796-879`, `src/application/io.rs`
- **Findings**:
  - `prepare_inputs` は `issue.state` の `match` で分岐し、対応するファイルのみ書き込む
  - `State::DesignFixing` / `State::ImplementationFixing` では `fixing_causes` に含まれる種別のファイルのみ書き込む（`ReviewComments` → `review_threads.json`、`CiFailure` → `ci_errors.txt`、`Conflict` → `conflict_info.txt`）
  - `.cupola/inputs/` ディレクトリは各 `write_*` 関数が `create_dir_all` で作成するが、既存ファイルの削除は行わない
- **Implications**: `prepare_inputs` の先頭でディレクトリごとクリアするのが最もシンプルで確実な解決策

### io.rs モジュール構造分析
- **Context**: クリア関数の配置場所を決定するための調査
- **Sources Consulted**: `src/application/io.rs`
- **Findings**:
  - `io.rs` は application レイヤーに属し、ファイル I/O に関する全関数を集約している
  - `write_issue_input`, `write_review_threads_input`, `write_ci_errors_input`, `write_conflict_info_input` の4関数が存在
  - 各関数がそれぞれ `create_dir_all` を呼んでいる（冗長だが問題にはなっていない）
  - テストは `tempfile::TempDir` を使ったユニットテストで構成されている
- **Implications**: `clear_inputs_dir` 関数を `io.rs` に追加し、`polling_use_case.rs` から呼び出す設計が自然

## Architecture Pattern Evaluation

| オプション | 説明 | 強み | リスク / 制限 |
|-----------|------|------|--------------|
| A: `clear_inputs_dir` を io.rs に追加 | `io.rs` にクリア関数を追加し、`prepare_inputs` の先頭で呼ぶ | 既存パターンとの一貫性、テスト容易性 | なし |
| B: `prepare_inputs` にインライン実装 | `polling_use_case.rs` 内に直接 `remove_dir_all` + `create_dir_all` を記述 | 変更ファイルが1つ | ファイル I/O ロジックが use case に漏れる（Clean Architecture 違反） |

選択: **オプション A**

## Design Decisions

### Decision: クリア関数を io.rs に配置

- **Context**: ファイル I/O 操作の責任を application/io.rs に集約するプロジェクト方針
- **Alternatives Considered**:
  1. io.rs に `clear_inputs_dir` 関数を追加
  2. `polling_use_case.rs` にインライン実装
- **Selected Approach**: `pub fn clear_inputs_dir(worktree_path: &Path) -> Result<()>` を `io.rs` に追加し、`prepare_inputs` の先頭で呼び出す
- **Rationale**: ファイル I/O ロジックを use case に漏らさないことで Clean Architecture の application レイヤーの純粋性を保つ
- **Trade-offs**: 変更ファイルが2つになるが、責任分離の明確さを優先する
- **Follow-up**: テスト追加（ディレクトリが存在する場合・しない場合の両ケース）

### Decision: `remove_dir_all` + `create_dir_all` によるクリア実装

- **Context**: Rust 標準ライブラリで利用可能なディレクトリ操作 API の選択
- **Alternatives Considered**:
  1. `remove_dir_all` + `create_dir_all`（ディレクトリごと削除・再作成）
  2. ディレクトリ内ファイルを列挙して個別削除
- **Selected Approach**: `remove_dir_all` でディレクトリを削除し、`create_dir_all` で再作成
- **Rationale**: シンプルで確実。個別削除ではサブディレクトリへの対応が必要になる可能性があるが、現状 inputs/ は平坦なファイルのみ
- **Trade-offs**: ディレクトリが存在しない場合も正常動作させるため、`remove_dir_all` のエラーは `ignore` する（`std::io::ErrorKind::NotFound` の場合）
- **Follow-up**: エラー処理の確認（NotFound は無視、それ以外は伝播）

## Risks & Mitigations
- ディレクトリが存在しない場合に `remove_dir_all` がエラーを返す — `ErrorKind::NotFound` を無視することで対応
- 将来 inputs/ にサブディレクトリが追加された場合も `remove_dir_all` で一括クリアできるため拡張性は問題なし
