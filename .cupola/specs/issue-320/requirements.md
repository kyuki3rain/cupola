# 要件定義書

## プロジェクト概要（入力）
## 概要

Issue 本文の改変による prompt 注入攻撃を実装で塞ぐ。`agent:ready` ラベル付与時点の本文を **「内容承認のスナップショット」** として hash で固定し、以降の改変を検知してデーモンを停止する。

## 攻撃シナリオ

現状、`spawn_process` (`execute.rs:638`) は spawn のたびに `get_issue` を呼んで Issue 本文を re-fetch している。これは以下の攻撃を許す:

1. 攻撃者 (NONE association、外部ユーザー) が **無害そうに見える Issue** を作成
2. リポジトリ Owner / Member が内容を確認して `agent:ready` を付与 → trusted check 通過 → DesignRunning 開始
3. **攻撃者は自分の Issue 本文を GitHub UI で編集** (Issue author は author_association に関係なく自分の Issue 本文を常に編集可能)
4. 次サイクルの SpawnProcess (DesignFixing / ImplementationRunning / ImplementationFixing 等) で Cupola が re-fetch → 改変後本文を Claude Code に prompt として渡す
5. `--dangerously-skip-permissions` 付き Claude Code が悪意指示を実行 → トークン / 鍵 / ファイル流出

attacker は collaborator である必要すらない。Issue を立てた本人が後から編集できる。

---

## 要件

### 要件 1: Issue 本文ハッシュの永続化

**目的:** Cupola オペレーターとして、Issue 本文ハッシュをポーリングサイクルをまたいで永続化できるようにしたい。改変検知をサイクル間で機能させるため。

#### 受け入れ条件

1.1. The system shall `issues` テーブルに nullable な `body_hash TEXT` カラムを保持する。

1.2. The system shall `Issue` ドメインエンティティに nullable な `body_hash: Option<String>` フィールドを含める。

1.3. The system shall スパース DB 更新をサポートするために `MetadataUpdates` に nullable な `body_hash: Option<Option<String>>` フィールドを含める。`Some(Some(hash))` は値を設定し、`Some(None)` はクリアし、`None` は変更しない。

1.4. When the `issues` table lacks the `body_hash` column on startup, the system shall データロスなしに冪等で後方互換のマイグレーションによってカラムを追加する。

---

### 要件 2: SpawnInit 時のハッシュ計算・保存

**目的:** Cupola オペレーターとして、初期化時に承認された Issue 本文の SHA-256 ハッシュを保存したい。承認スナップショットを確立するため。

#### 受け入れ条件

2.1. When the `SpawnInit` effect is executed and the issue body is successfully fetched from GitHub, the system shall 本文の SHA-256 hex ダイジェストを計算し、`IssueRepository.update_state_and_metadata` を介して `body_hash` に永続化する。

2.2. When the hash is saved during `SpawnInit`, the system shall インメモリの `Issue.body_hash` フィールドを更新して保存された値を反映させ、現在のポーリングサイクルに更新済みハッシュが反映されるようにする。

---

### 要件 3: SpawnProcess 前のハッシュ比較

**目的:** Cupola オペレーターとして、各 spawn 前に Issue 本文を保存済みハッシュと照合したい。改変された本文を拒否するため。

#### 受け入れ条件

3.1. While `Issue.body_hash` is `Some(saved_hash)`, when `SpawnProcess` is about to execute, the system shall 現在の Issue 本文を再取得してその SHA-256 hex ダイジェストを計算する。

3.2. If the computed hash differs from the stored `body_hash`, the system shall これを改変イベントとして扱い、`BodyTamperedError` を返して spawn を中断する。

3.3. While `Issue.body_hash` is `None`, the system shall ハッシュ比較をスキップして通常通り spawn を続行する（本機能導入前に初期化された Issue の後方互換動作）。

---

### 要件 4: 改変検知時の応答

**目的:** Cupola オペレーターとして、改変検知時にシステムが自動的にキャンセルして通知してほしい。改変後の悪意ある指示が Claude Code に到達しないようにするため。

#### 受け入れ条件

4.1. If body tampering is detected, the system shall Issue の状態をデータベース上で `Cancelled` に遷移させる。

4.2. If body tampering is detected, the system shall GitHub Issue から `agent:ready` ラベルを削除する（ベストエフォート；ラベル削除の失敗は `warn!` ログに記録され、処理を中断しない）。

4.3. If body tampering is detected, the system shall GitHub Issue にキャンセル理由と処理再開に必要な手順を説明する通知コメントを投稿する（ベストエフォート；コメント失敗は `warn!` ログに記録され、処理を中断しない）。

4.4. If body tampering is detected, the system shall Issue 番号と改変イベントの説明を含む `warn!` トレースイベントを発火する。

---

### 要件 5: 再 approve フロー

**目的:** 信頼されたユーザーとして、改変後の内容を確認してから `agent:ready` を再付与することで処理を再開したい。正当な内容変更を処理再開可能にするため。

#### 受け入れ条件

5.1. When `agent:ready` is re-applied to a Cancelled issue by a trusted user, the system shall 初期化フロー（SpawnInit エフェクト）を再開し、現在の Issue 本文を再取得して更新済みハッシュを新しい承認スナップショットとして保存する。

5.2. When the re-approve flow completes successfully, the system shall 新しく保存されたハッシュと照合しながら後続の spawn を続行する。

---

### 要件 6: SECURITY.md の更新

**目的:** 開発者・オペレーターとして、SECURITY.md が Issue 本文承認の信頼モデルを正確に説明していてほしい。セキュリティ境界を明確に伝えるため。

#### 受け入れ条件

6.1. The system documentation shall `agent:ready` ラベルの付与者がラベル付与時点での Issue 本文内容の承認に責任を負うことを記載する。

6.2. The system documentation shall Issue の作成者が `author_association` に関係なく自分の Issue 本文を編集できることを記載する。

6.3. The system documentation shall ハッシュベースの本文改変検知メカニズムと検知時のシステム動作（Cancelled 遷移、ラベル削除、コメント通知）を説明する。

6.4. The system documentation shall 進行中の変更要求は Issue 本文の編集ではなく PR レビューコメント経由で送信すべきことを記載する。

6.5. The system documentation shall Issue 本文は各 spawn で再取得されるが、SHA-256 ハッシュが保存値と一致しない場合は拒否されることを説明する。

6.6. The system documentation shall collaborator 以上のユーザーも Issue 本文の編集権限を持ち、collaborator ステータスを付与することは実質的に信頼境界を広げる行為であることを記載する。
