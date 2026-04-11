# リサーチ・設計決定ログ

---
**Purpose**: 発見事項、アーキテクチャ調査、設計根拠を記録する。

---

## サマリー

- **Feature**: issue-198 — PIDファイルフォーマット・statusコマンド出力の3バグ修正
- **Discovery Scope**: Extension（既存システムの拡張・バグ修正）
- **Key Findings**:
  - `PidFilePort` トレイトにモードパラメータが存在せず、`write_pid` は1行のみ書き込む
  - `handle_status` 関数が `Daemon:` プレフィックスを使用しており、ドキュメントの `Process:` と乖離している
  - セッション数ラベルが `Running:` であり、ドキュメントの `Claude sessions:` と異なる
  - レガシーPIDファイル（1行フォーマット）への後方互換性が必要

## リサーチログ

### 既存実装の調査

- **Context**: ドキュメント仕様と実装の差分を特定するために既存コードを調査
- **Sources Consulted**: `src/application/port/pid_file.rs`, `src/adapter/outbound/pid_file_manager.rs`, `src/bootstrap/app.rs`
- **Findings**:
  - `PidFilePort::write_pid(pid: u32)` は `writeln!(file, "{pid}")` で1行のみ書き込む（行:33）
  - `PidFilePort::read_pid()` は `content.trim().parse::<u32>()` で1行のみ読み取る
  - `handle_status` の `read_pid()` → `writeln!(out, "Daemon: running (pid={pid})")` がプレフィックス誤り（行:682）
  - セッション数ラベルは `Running:` を使用（行:718-719）
  - PID範囲バリデーション（`pid == 0 || pid > i32::MAX`）は `read_pid_and_mode` でも継承が必要
- **Implications**:
  - `write_pid` を残しつつ新メソッド `write_pid_with_mode` を追加してトレイトを拡張する
  - `read_pid_and_mode` は1行ファイルを graceful に処理（`(pid, None)` を返す）

### アーキテクチャ境界の確認

- **Context**: `ProcessMode` 型をどのレイヤーに配置するかを検討
- **Findings**:
  - `PidFilePort` は `application/port/` に存在するが、`ProcessMode` はプロセス起動モードという純粋な業務概念
  - ただし本プロジェクトではドメインエンティティとポートが密結合の傾向があり、`application/port/pid_file.rs` 内で定義するのが実用的
  - `bootstrap/app.rs` が `ProcessMode` を参照するため、`pub` での公開が必要
- **Implications**:
  - `ProcessMode` は `application/port/pid_file.rs` に同居して定義する

## アーキテクチャパターン評価

| オプション | 説明 | 強み | リスク・制限 | 採用 |
|-----------|------|------|------------|------|
| 既存トレイト拡張 | `write_pid_with_mode` / `read_pid_and_mode` を追加、`write_pid` を残存 | 後方互換性を保つ。既存のモック実装への影響を最小化 | トレイトのサイズが増加する | ✅ 採用 |
| 新トレイト作成 | `PidFilePortV2` などの新トレイトを定義 | クリーンな分離 | DI配線の変更が必要、オーバーエンジニアリング | ❌ |
| `write_pid` を置き換え | シグネチャ変更 `write_pid(pid, mode)` | シンプル | 既存テスト・モック実装への破壊的変更が多い | ❌ |

## 設計決定

### Decision: `ProcessMode` の配置

- **Context**: `ProcessMode` は `bootstrap/app.rs` で生成され、`application/port/pid_file.rs` のトレイトで受け取る
- **Alternatives Considered**:
  1. `domain/` に配置 — ProcessModeはドメインの純粋概念として定義
  2. `application/port/pid_file.rs` に同居 — ポート定義と同じファイルに置く
- **Selected Approach**: `application/port/pid_file.rs` に同居（Issueの修正プランに倣う）
- **Rationale**: ドメイン層への変更範囲を最小化し、既存のポートファイル構成を維持する
- **Trade-offs**: ドメインエンティティを厳密に分離する観点では次善策だが、実用的かつ一貫性がある
- **Follow-up**: 将来的にProcessModeがより広域で使われる場合はdomain層への移動を検討

### Decision: レガシーファイルの後方互換処理

- **Context**: デプロイ済み環境には1行フォーマットのPIDファイルが存在する可能性がある
- **Alternatives Considered**:
  1. エラーを返す — 古いファイルを強制的に削除させる
  2. `(pid, None)` を返してinfoログ — ノンブレーキングな移行
- **Selected Approach**: `(pid, None)` を返してinfoログを出力
- **Rationale**: ユーザーが意識しなくても次回起動時に新フォーマットで上書きされる。エラーにすると既存の動作を壊す
- **Trade-offs**: 一時的に `unknown` モードが表示される移行期間が生じる（許容範囲）

## リスクと対策

- **`read_pid_and_mode` でPID範囲バリデーションが欠落するリスク** — `read_pid` と同一のバリデーション（`pid == 0 || pid > i32::MAX`）を継承して実装することで対処
- **`handle_status` の stale 判定ロジック変更漏れのリスク** — TOCTOU対策の `read_pid()` 呼び出し部分は `read_pid_and_mode()` に切り替えることで統一

## 参考資料

- [docs/commands/start.md](https://github.com/kyuki3rain/cupola/blob/main/docs/commands/start.md#L80) — PIDファイルフォーマット仕様
- [docs/commands/status.md](https://github.com/kyuki3rain/cupola/blob/main/docs/commands/status.md#L24) — status出力フォーマット仕様
