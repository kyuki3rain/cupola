# 要件定義書

## はじめに

本ドキュメントは、Cupola における **TaskWeight × Phase によるモデル解決機構** の要件を定義する。

現在 `Issue.model: Option<String>` フィールドが存在するが、実際の読み書きは未実装であり、全 Issue・全フェーズで同一のトップレベル `model` 設定を使用している。本機能では、タスクの重さ（TaskWeight）と実行フェーズ（Phase）の 2 軸を用いて使用モデルを動的解決する機構を導入する。これにより、1 行設定で動作しつつ、必要に応じてフェーズ単位の細かいチューニングも可能にする。

## 要件

### 要件 1: TaskWeight ドメイン型の導入

**Objective:** Cupola 開発者として、Issue のタスク重さを型安全に表現したい。そうすることで、具体的なモデル名を Issue 側に持たせることなく、設定変更だけでモデルを切り替えられるようにしたい。

#### 受け入れ基準

1. The Cupola domain shall define a `TaskWeight` enum with variants `Light`, `Medium` (default), and `Heavy`.
2. The Cupola domain shall define a `Phase` enum with variants `Design`, `DesignFix`, `Implementation`, and `ImplementationFix`.
3. When `Phase::from_state(state)` is called with a valid workflow state, the Cupola domain shall return the corresponding `Phase` variant (`Some(Phase)`).
4. If `Phase::from_state(state)` is called with a state that has no corresponding phase (例: `Idle`, `Completed`, `Cancelled`), the Cupola domain shall return `None`.
5. When `phase.base()` is called on `Phase::DesignFix`, the Cupola domain shall return `Some(Phase::Design)`.
6. When `phase.base()` is called on `Phase::ImplementationFix`, the Cupola domain shall return `Some(Phase::Implementation)`.
7. When `phase.base()` is called on `Phase::Design` or `Phase::Implementation`, the Cupola domain shall return `None`.

---

### 要件 2: Config によるモデル解決機構

**Objective:** Cupola 運用者として、weight × phase の組み合わせに応じて使用モデルを柔軟に設定したい。そうすることで、軽量タスクにはコスト効率の高いモデル、重要フェーズには高性能モデルを自動的に割り当てられるようにしたい。

#### 受け入れ基準

1. The Cupola config shall support a global fallback `model` string (例: `model = "sonnet"`) as the minimum required configuration.
2. Where the `[models]` section is included in `cupola.toml`, the Cupola config shall accept per-weight model overrides using either a uniform string (`light = "haiku"`) or a per-phase table (`[models.heavy]`).
3. When `config.models.resolve(weight, phase)` is called, the Cupola config shall apply the following 4-step fallback chain in order:
   1. `models.<weight>.<exact_phase>`（最も具体的）
   2. `models.<weight>.<base_phase>`（`design_fix` → `design`、`implementation_fix` → `implementation`）
   3. `models.<weight>`（weight が Uniform 文字列の場合）
   4. `model`（グローバルデフォルト）
4. When `models.<weight>` is a Uniform string and `resolve(weight, phase)` is called for any phase, the Cupola config shall return that uniform string regardless of phase.
5. When `models.<weight>` is absent and `resolve(weight, phase)` is called, the Cupola config shall fall back to the global `model` value.
6. The Cupola config shall treat the global `model` as the final fallback for model resolution, so resolution shall succeed even when `models.<weight>` is not configured.
7. The Cupola config shall deserialize `ModelTier` as an untagged enum that accepts either a `String` (Uniform) or a struct with optional phase fields (`design`, `design_fix`, `implementation`, `implementation_fix`).

---

### 要件 3: Issue エンティティの変更

**Objective:** Cupola 開発者として、Issue エンティティから具体的なモデル名を排除し、TaskWeight のみを保持したい。そうすることで、モデル名変更時に Issue データの更新が不要になるようにしたい。

#### 受け入れ基準

1. The Cupola domain shall replace `Issue.model: Option<String>` with `Issue.weight: TaskWeight`.
2. The Cupola domain shall use `TaskWeight::Medium` as the default value for `Issue.weight`.
3. When an Issue entity is created without explicit weight assignment, the Cupola domain shall set `weight` to `TaskWeight::Medium`.

---

### 要件 4: DB スキーマの変更

**Objective:** Cupola 運用者として、Issue の weight 情報を SQLite に永続化したい。そうすることで、Cupola 再起動後も weight 設定が保持されるようにしたい。

#### 受け入れ基準

1. The Cupola SQLite adapter shall replace the `model TEXT` column with `weight TEXT NOT NULL DEFAULT 'medium'` in the issues table.
2. When saving an Issue to SQLite, the Cupola SQLite adapter shall serialize `TaskWeight::Light` as `"light"`, `TaskWeight::Medium` as `"medium"`, and `TaskWeight::Heavy` as `"heavy"`.
3. When reading an Issue from SQLite, the Cupola SQLite adapter shall deserialize the `weight` text column into the corresponding `TaskWeight` variant.
4. If the `weight` column contains an unrecognized string, the Cupola SQLite adapter shall return a deserialization error.

---

### 要件 5: GitHub Label による TaskWeight 指定

**Objective:** Cupola 運用者として、GitHub Issue に `weight:light` または `weight:heavy` ラベルを付けるだけで TaskWeight を指定したい。そうすることで、専用の設定ファイル変更なしにタスク単位でモデルを切り替えられるようにしたい。

#### 受け入れ基準

1. When the Cupola polling step detects an Issue with the `weight:light` label, the Cupola polling step shall set `Issue.weight` to `TaskWeight::Light`.
2. When the Cupola polling step detects an Issue with the `weight:heavy` label, the Cupola polling step shall set `Issue.weight` to `TaskWeight::Heavy`.
3. When the Cupola polling step detects an Issue with no `weight:*` label, the Cupola polling step shall set `Issue.weight` to `TaskWeight::Medium`.
4. When the Cupola polling step detects an Issue with both `weight:light` and `weight:heavy` labels simultaneously, the Cupola polling step shall prefer `TaskWeight::Heavy`.

---

### 要件 6: spawn 時のモデル解決

**Objective:** Cupola 運用者として、Issue の spawn 時に weight とフェーズから自動的に適切なモデルが選択されるようにしたい。そうすることで、フェーズごとに最適なモデルを手動指定することなく使えるようにしたい。

#### 受け入れ基準

1. When the Cupola polling step spawns a Claude Code session, the Cupola polling step shall resolve the model using `config.models.resolve(issue.weight, Phase::from_state(issue.state))`.
2. When `Phase::from_state(issue.state)` returns `None`, the Cupola polling step shall fall back to the global `model` value from config.
3. When the resolved model is determined, the Cupola polling step shall pass `--model <resolved_model>` to the Claude Code runner.
4. The Cupola polling step shall NOT use `config.model` directly as the spawn model; resolution must go through `config.models.resolve()`.

---

### 要件 7: init テンプレートおよび doctor コマンドの更新

**Objective:** Cupola 運用者として、`cupola init` で生成される設定テンプレートに `[models]` セクションの記法例が含まれてほしい。そうすることで、設定方法をドキュメントなしで理解できるようにしたい。

#### 受け入れ基準

1. When `cupola init` is executed, the Cupola CLI shall generate a `cupola.toml` template that includes commented examples of the `[models]` section (weight 別・phase 別の記法例).
2. When `cupola doctor` checks GitHub labels, the Cupola CLI shall verify the existence of `weight:light` and `weight:heavy` labels instead of the former `model:*` labels.
3. If `weight:light` or `weight:heavy` labels are absent from the repository, the Cupola CLI doctor shall report a warning indicating the missing labels.
