# 要件定義書

## はじめに

Claude Code 子プロセスへの環境変数（env）継承を最小化することで、Cupola daemon の secrets（GH_TOKEN、AWS_*、OPENAI_API_KEY 等）が Claude Code プロセスに漏洩するリスクを低減する。`Command::env_clear()` でデフォルト全継承を断ち、whitelist 方式で必要な env だけを渡す。

本機能は多層防御の一層として位置付けられ、prompt 注入が起きた場合の環境変数漏洩出口を機械的に塞ぐことを目的とする。

## 要件

### Requirement 1: env_clear とベース allowlist の適用

**目的:** Cupola オペレーターとして、Claude Code 子プロセスに渡される環境変数を必要最小限に絞りたい。そのため、デフォルトの env 全継承を廃止し、hardcode されたベース allowlist のみを渡す仕組みを導入する。

#### 受け入れ基準

1. When [Claude Code 子プロセスが起動される], the ClaudeCodeProcess shall call `env_clear()` to remove all inherited environment variables from the child process.
2. The ClaudeCodeProcess shall always pass `HOME`、`PATH`、`USER`、`LANG`、`LC_ALL`、`TERM` from the parent process environment to the Claude Code child process, regardless of configuration.
3. If [any BASE_ALLOWLIST key is absent from the parent process environment], the ClaudeCodeProcess shall silently skip that key without error.

---

### Requirement 2: cupola.toml による追加 env 設定

**目的:** Cupola オペレーターとして、プロジェクトごとに必要な追加 env var（ANTHROPIC_API_KEY、DOCKER_HOST 等）を cupola.toml で宣言的に管理したい。そのため、`[claude_code.env]` TOML セクションで `extra_allow` リストを設定できるようにする。

#### 受け入れ基準

1. When [cupola.toml を読み込む時], the system shall parse the optional `[claude_code.env]` TOML section and load `extra_allow` as a list of pattern strings.
2. When [extra_allow が設定されている時], the ClaudeCodeProcess shall pass all env vars from the parent process whose keys match at least one extra_allow pattern, in addition to BASE_ALLOWLIST.
3. If [cupola.toml に `[claude_code.env]` セクションが存在しない場合], the system shall treat `extra_allow` as an empty list and pass only BASE_ALLOWLIST.
4. The system shall make `extra_allow` default to an empty list when the `[claude_code.env]` section is present but `extra_allow` is not specified.

---

### Requirement 3: ワイルドカードパターン（サフィックス `*`）

**目的:** Cupola オペレーターとして、`CLAUDE_*` のように prefix が共通する env var グループをまとめて許可したい。そのため、サフィックス `*` による prefix match をサポートする。

#### 受け入れ基準

1. When [extra_allow のパターンが `*` で終わる時], the system shall allow all env vars whose key starts with the prefix preceding the `*`.
2. When [extra_allow のパターンが `*` で終わらない時], the system shall only allow env vars whose key exactly matches the pattern string.
3. The system shall not support wildcard characters in positions other than the trailing suffix (e.g., `*_KEY` や `MY_*_VAR` はリテラル文字列として扱われる).

---

### Requirement 4: cupola init テンプレートへの統合

**目的:** Cupola オペレーターとして、`cupola init` で生成される cupola.toml に `[claude_code.env]` の雛形がコメントアウト状態で含まれていてほしい。そのため、初期テンプレートにセクションと候補パターンを追記する。

#### 受け入れ基準

1. When [`cupola init` が cupola.toml テンプレートを生成する時], the system shall include a `[claude_code.env]` section with `extra_allow` containing commented-out example entries: `ANTHROPIC_API_KEY`、`CLAUDE_*`、`OPENAI_API_KEY`、`DOCKER_HOST`.
2. The system shall include a comment in the generated template that explains the purpose of `extra_allow` and that wildcard suffix `*` is supported.

---

### Requirement 5: doctor コマンドへの env allowlist 統合

**目的:** Cupola オペレーターとして、`cupola doctor` で現在の env allowlist と危険な設定の有無を確認したい。そのため、doctor の StartReadiness セクションに env allowlist チェックを追加する。

#### 受け入れ基準

1. When [`cupola doctor` を実行する時], the system shall display the BASE_ALLOWLIST (hardcoded) and the configured `extra_allow` patterns under the StartReadiness section.
2. When [`extra_allow` に潜在的に危険な env var 名（例: `GH_TOKEN`、`AWS_SECRET_ACCESS_KEY`、`AWS_*`）が含まれる時], the doctor shall output a Warn result with a message identifying the sensitive patterns and recommending their removal if not needed.
3. If [`extra_allow` に危険パターンが含まれない場合], the doctor shall output an Ok result for the env allowlist check.
4. While [cupola.toml の読み込みに失敗している時], the doctor shall skip the env allowlist check without panicking.
