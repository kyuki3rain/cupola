# 要件定義書

## はじめに

本機能は、Cupola の状態機械において Fixing ↔ ReviewWaiting 間で発生しうる無限ループを防止するため、CI 失敗（CiFailure）またはコンフリクト（Conflict）を起因とする修正試行回数（ci_fix_count）に上限を設ける機構を追加する。上限到達時は Issue をキャンセルせず ReviewWaiting 状態のまま保持し、人間の手動介入を促す。

## 要件

### 要件 1: CI/Conflict 起因修正試行カウンターの管理

**目的:** 自動化エージェントとして、CI 失敗またはコンフリクトによる修正試行回数を追跡したい。それにより、修正が収束しない場合に自動処理を停止して人間の介入を求めることができる。

#### 受け入れ基準

1. The Cupola shall `ci_fix_count: u32` フィールドを Issue エンティティに保持し、デフォルト値を 0 とすること。
2. When step4_pr_monitoring の評価結果 causes が空（CI パス・未解決スレッドなし）の場合、the Cupola shall `ci_fix_count` を 0 にリセットすること。
3. When step4_pr_monitoring の評価結果が ReviewComments のみを含む場合、the Cupola shall `ci_fix_count` を 0 にリセットし、Fixing への遷移を行うこと。
4. When step4_pr_monitoring の評価結果が CiFailure または Conflict を含み、かつ `ci_fix_count` が上限未満の場合、the Cupola shall `ci_fix_count` を 1 増加させて Fixing へ遷移すること。
5. When step4_pr_monitoring の評価結果が CiFailure または Conflict を含み、かつ `ci_fix_count` が上限以上の場合、the Cupola shall `ci_fix_count` を変更せず Fixing への遷移を行わないこと。
6. When CiFailure と ReviewComments が同時に含まれる場合、the Cupola shall ReviewComments を含むとみなし `ci_fix_count` を 0 にリセットすること。

---

### 要件 2: 上限値の設定とオーバーライド

**目的:** 運用者として、CI/Conflict 修正試行の上限回数をプロジェクト要件に応じて設定可能にしたい。それにより、柔軟な運用が可能になる。

#### 受け入れ基準

1. The Cupola shall デフォルトの上限値として `max_ci_fix_cycles = 3` を Config に保持すること。
2. Where `max_ci_fix_cycles` が設定ファイル（TOML）で指定されている場合、the Cupola shall その値を優先して使用すること。
3. The Cupola shall `max_ci_fix_cycles` に正の整数のみを受け入れ、0 や負の値は設定エラーとして拒否すること。

---

### 要件 3: 上限到達時の通知と状態維持

**目的:** 人間レビュアーとして、CI/Conflict 修正が自動で解消できなかった事実を通知で受け取りたい。それにより、適切なタイミングで手動介入を行うことができる。

#### 受け入れ基準

1. When `ci_fix_count` が上限に達した場合、the Cupola shall Issue に「CI/Conflict の修正が上限に達しました。手動確認してください。」というコメントを 1 回だけ投稿すること。
2. While `ci_fix_count` が上限に達している場合、the Cupola shall Issue を ReviewWaiting 状態のまま維持し、PR を open のまま保持すること。
3. If 上限到達時に Cupola shall Issue を Cancelled 状態に遷移させないこと。
4. When 人間が手動で修正をプッシュし CI が通過した場合、the Cupola shall 次のポーリングサイクルで通常の ReviewWaiting → Fixing フローを再開すること。

---

### 要件 4: フェーズ変更時のカウンターリセット

**目的:** 自動化エージェントとして、フェーズ（例：Design → Implementation）が切り替わった際に過去の CI 修正カウントを引き継がないようにしたい。それにより、フェーズをまたいだ誤ったカウント累積を防止できる。

#### 受け入れ基準

1. When transition_use_case においてフェーズ変更（Design フェーズから Implementation フェーズへの遷移など）が発生した場合、the Cupola shall `ci_fix_count` を 0 にリセットすること。

---

### 要件 5: SQLite への永続化

**目的:** 自動化エージェントとして、プロセス再起動後も `ci_fix_count` の値が失われないようにしたい。それにより、再起動をまたいで修正試行回数が正確に追跡される。

#### 受け入れ基準

1. The Cupola shall SQLite の issues テーブルに `ci_fix_count INTEGER NOT NULL DEFAULT 0` カラムを追加すること。
2. When Issue の状態が更新される場合、the Cupola shall `ci_fix_count` の最新値を SQLite に永続化すること。
3. When Issue をデータベースから読み込む場合、the Cupola shall 保存された `ci_fix_count` の値を正確に復元すること。
4. Where 既存のデータベースに `ci_fix_count` カラムが存在しない場合、the Cupola shall マイグレーションにより DEFAULT 0 でカラムを追加し、既存レコードを壊さないこと。
