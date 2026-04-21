# リサーチ・設計調査ノート

---
**目的**: 設計判断の根拠となる調査結果、アーキテクチャ検討、トレードオフを記録する。
---

## サマリー

- **フィーチャー**: `issue-316` — PostCiFixLimitComment 通知欠落防止（best-effort 失敗時の再発火）
- **ディスカバリースコープ**: Extension（既存システムへの拡張）
- **主要知見**:
  - `ci_fix_count += 1` は decide の MetadataUpdates に含まれ、execute 成否に関わらず persist される
  - `Effect::PostCiFixLimitComment` の発火条件が `== max` のため、count が max+1 になった次サイクルで条件不成立となり再発火不可能
  - `DoctorUseCase` は現在 sync な `run(&self, path)` を持ち、外部コマンド実行（`CommandRunner`）とファイルシステム確認のみ行う。DB クエリは未対応
  - `update_state_and_metadata` は動的 SQL を用いて `MetadataUpdates` の `Some` フィールドのみ更新する柔軟な設計

## リサーチログ

### 既存の ci_fix_count フロー調査

- **コンテキスト**: バグ再現の確認
- **調査対象ファイル**:
  - `src/domain/decide.rs:679-704` (CI failure / conflict 経路)
  - `src/application/polling/execute.rs:228-234` (PostCiFixLimitComment ハンドラ)
  - `src/domain/metadata_update.rs` (MetadataUpdates 構造体)
  - `src/adapter/outbound/sqlite_issue_repository.rs:216-309` (update_state_and_metadata)
- **知見**:
  - `decide` が `metadata_updates.ci_fix_count = Some(prev.ci_fix_count + 1)` を設定
  - `persist` フェーズでこれが DB に書き込まれる（execute 成否に関わらず）
  - execute で `PostCiFixLimitComment` が失敗しても `ci_fix_count` は max+1 として確定
  - 次サイクルでは `ci_fix_count == max+1 ≠ max_ci_fix_cycles` → 条件不成立 → 通知欠落
- **影響**: この問題は `ci_fix_limit_notified` フラグ追加だけでは解決しない。発火条件を `==` から `>=` に変更することも必要

### 発火条件の変更方針検討

- **コンテキスト**: Issue 本文の疑似コードは `== max` を示すが、受け入れ条件「投稿失敗時は次サイクルで再発火する」と矛盾する
- **知見**:
  - `== max` のままだと: 失敗後 count = max+1 になり、次サイクルで `max+1 ≠ max` → 再発火不可
  - `>= max` に変更すると: 失敗後 count = max+1 でも `max+1 >= max && !notified` → 再発火可能
  - `notified = true` が二重防御として機能し、成功後は count がいくら大きくなっても再発火しない
- **結論**: 発火条件を `>= max && !notified` に変更する。Issue 疑似コードの `==` は「happy path での動作説明」であり、再発火要件と整合させるには `>=` が正しい

### execute 側フラグ書き込みの設計選択肢（Issue #316 より）

- **コンテキスト**: `ci_fix_limit_notified` をどのタイミングで `true` に書くか
- **検討した選択肢**:
  - **(A) execute 成功後に repo を直接呼ぶ**: `PostCiFixLimitComment` 成功後に `update_state_and_metadata` 呼び出し。シンプルだが Decide/Persist/Execute の責務分離が崩れる
  - **(B) decide が「失敗時に false に巻き戻す」追加 Effect を出す**: 設計が複雑化
  - **(C) ci_fix_count を +1 する処理も execute 成功時に移す**: 現行設計を大きく変えるため不採用
- **採用**: **(A) を採用**。execute.rs の `PostCiFixLimitComment` ハンドラ内で match して成功時のみ `issue_repo.update_state_and_metadata` を呼ぶ

### DoctorUseCase への DB クエリ追加

- **コンテキスト**: doctor コマンドに pending 通知件数チェックを追加するための方法
- **現状**: `DoctorUseCase<C: ConfigLoader, R: CommandRunner>` は sync な `run()` を持ち、外部コマンド呼び出しとファイルシステム確認のみ
- **選択肢**:
  1. `IssueRepository` を第三型パラメータとして追加し `run` を async 化
  2. `DoctorUseCase::run` に `pending_count: usize` を追加引数として受け取る（bootstrap が事前計算）
  3. 同期専用ポートトレイト `PendingNotificationCounter` を新設
- **採用**: 選択肢 1（async化 + `IssueRepository` 型パラメータ追加）。既存パターン（ジェネリクスによるポート注入）と一致し、テスタビリティも高い。bootstrap の呼び出し側も async であるため影響小。

### status コマンドでの警告表示箇所

- **コンテキスト**: status 出力は `bootstrap/app.rs` の `handle_status` 関数が担当（line 735-811）
- **知見**:
  - config（`max_ci_fix_cycles`）と issue 一覧（`issue_repo.find_all()`）に既にアクセス可能
  - 既存の表示ループ内で各 issue をチェックして `⚠` を付加するだけでよい
  - `DoctorUseCase` のような大きな変更は不要

## アーキテクチャパターン評価

| 選択肢 | 説明 | 強み | リスク・制約 |
|--------|------|------|-------------|
| execute 成功後 repo 直接呼び出し (A) | execute ハンドラ内で成功確認後に update_state_and_metadata | シンプル、最小スコープ | 責務分離の観点ではやや惜しい |
| 追加 Effect で巻き戻し (B) | decide が SetCiFixLimitNotified Effect も出す | 設計純粋性 | 複雑化、不採用 |
| ci_fix_count も execute 移動 (C) | persist を使わず execute が全部担当 | 一貫性 | 設計変更大、不採用 |

## 設計決定

### 決定: 発火条件を `>= max` に変更する

- **コンテキスト**: 受け入れ条件「投稿失敗時は次サイクルで Decide が再発火する」を満たすため
- **検討した代替**:
  1. `== max` のまま + `!notified` 追加 → 再発火不可（count が max+1 になるため）
  2. `>= max` + `!notified` → 再発火可能、success 後は `notified=true` で停止
- **採用アプローチ**: `prev.ci_fix_count >= cfg.max_ci_fix_cycles && !prev.ci_fix_limit_notified`
- **根拠**: 全受け入れ条件を充足できる唯一の条件式
- **トレードオフ**: 再試行ごとに `ci_fix_count` がインクリメントされ続けるが、`notified=true` になった後は停止するため問題なし
- **フォローアップ**: 単体テストで `count=max+1 && notified=false` のシナリオを必ず検証する

### 決定: DoctorUseCase を async 化し IssueRepository を追加

- **コンテキスト**: doctor チェックに DB クエリが必要
- **採用アプローチ**: `DoctorUseCase<C, R, I: IssueRepository>` に拡張し `run` を `async fn` に変更
- **根拠**: 既存パターンとの一貫性、モック注入によるテスタビリティ
- **トレードオフ**: bootstrap 側の呼び出しを `await` に変更する必要があるが影響は軽微

## リスクと緩和策

- **risk**: execute 側から `issue_repo.update_state_and_metadata` を呼ぶ際、issue_repo が `execute` 関数のシグネチャに渡されていない場合 → `execute` 関数のシグネチャを確認し必要なら引数追加
- **risk**: `row_to_issue` の列インデックスずれ → SELECT クエリの列順序と `row_to_issue` の実装を同時に変更し、統合テストで検証
- **risk**: `DoctorUseCase::run` を async 化することで既存テストが同期呼び出しできなくなる → `#[tokio::test]` に移行

## 参考

- `docs/architecture/effects.md:56` — PostCiFixLimitComment の既知リスク記述
- `src/domain/decide.rs:679-704` — 現行発火条件
- `src/application/polling/execute.rs:228-234` — PostCiFixLimitComment ハンドラ
- `src/adapter/outbound/sqlite_issue_repository.rs:216-309` — update_state_and_metadata 実装
- `src/adapter/outbound/sqlite_connection.rs:57-128` — init_schema + マイグレーション実装
- Issue #338 — 長期的な EffectLog port 抽象化の構想
