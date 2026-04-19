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

1.1. The system shall maintain a nullable `body_hash TEXT` column in the `issues` table.

1.2. The system shall include a nullable `body_hash: Option<String>` field in the `Issue` domain entity.

1.3. The system shall include a nullable `body_hash: Option<Option<String>>` field in `MetadataUpdates` to support sparse DB updates; `Some(Some(hash))` sets the value, `Some(None)` clears it, and `None` leaves it unchanged.

1.4. When the `issues` table lacks the `body_hash` column on startup, the system shall add the column via an idempotent, backward-compatible migration without data loss.

---

### 要件 2: SpawnInit 時のハッシュ計算・保存

**目的:** Cupola オペレーターとして、初期化時に承認された Issue 本文の SHA-256 ハッシュを保存したい。承認スナップショットを確立するため。

#### 受け入れ条件

2.1. When the `SpawnInit` effect is executed and the issue body is successfully fetched from GitHub, the system shall compute a SHA-256 hex digest of the body and persist it in `body_hash` via `IssueRepository.update_state_and_metadata`.

2.2. When the hash is saved during `SpawnInit`, the system shall update the in-memory `Issue.body_hash` field to reflect the stored value so that the current polling cycle reflects the updated hash.

---

### 要件 3: SpawnProcess 前のハッシュ比較

**目的:** Cupola オペレーターとして、各 spawn 前に Issue 本文を保存済みハッシュと照合したい。改変された本文を拒否するため。

#### 受け入れ条件

3.1. While `Issue.body_hash` is `Some(saved_hash)`, when `SpawnProcess` is about to execute, the system shall re-fetch the current issue body and compute its SHA-256 hex digest.

3.2. If the computed hash differs from the stored `body_hash`, the system shall treat this as a tampering event and abort the spawn by returning `BodyTamperedError`.

3.3. While `Issue.body_hash` is `None`, the system shall skip the hash comparison and proceed with the spawn normally (backward-compatible behavior for issues initialized before this feature).

---

### 要件 4: 改変検知時の応答

**目的:** Cupola オペレーターとして、改変検知時にシステムが自動的にキャンセルして通知してほしい。改変後の悪意ある指示が Claude Code に到達しないようにするため。

#### 受け入れ条件

4.1. If body tampering is detected, the system shall transition the issue state to `Cancelled` in the database.

4.2. If body tampering is detected, the system shall remove the `agent:ready` label from the GitHub issue (best-effort; label removal failure is logged as `warn!` and does not abort).

4.3. If body tampering is detected, the system shall post a notification comment on the GitHub issue explaining the reason for cancellation and the steps required to resume processing (best-effort; comment failure is logged as `warn!` and does not abort).

4.4. If body tampering is detected, the system shall emit a `warn!` tracing event including the issue number and a description of the tampering event.

---

### 要件 5: 再 approve フロー

**目的:** 信頼されたユーザーとして、改変後の内容を確認してから `agent:ready` を再付与することで処理を再開したい。正当な内容変更を処理再開可能にするため。

#### 受け入れ条件

5.1. When `agent:ready` is re-applied to a Cancelled issue by a trusted user, the system shall restart the initialization flow (SpawnInit effect), which re-fetches the current issue body and saves the updated hash as the new approval snapshot.

5.2. When the re-approve flow completes successfully, the system shall continue subsequent spawns comparing against the newly saved hash.

---

### 要件 6: SECURITY.md の更新

**目的:** 開発者・オペレーターとして、SECURITY.md が Issue 本文承認の信頼モデルを正確に説明していてほしい。セキュリティ境界を明確に伝えるため。

#### 受け入れ条件

6.1. The system documentation shall state that the `agent:ready` label applicant bears responsibility for approving the issue body content at the time of labeling.

6.2. The system documentation shall state that Issue authors can edit their own issue body regardless of `author_association`.

6.3. The system documentation shall explain the hash-based body tampering detection mechanism and the system's behavior upon detection (Cancelled transition, label removal, comment notification).

6.4. The system documentation shall state that in-progress change requests should be submitted via PR review comments, not by editing the issue body.

6.5. The system documentation shall explain that the issue body is re-fetched on each spawn but rejected if the SHA-256 hash does not match the saved value.

6.6. The system documentation shall note that collaborator+ users also have issue body edit access, and that granting collaborator status effectively extends the trust boundary.
