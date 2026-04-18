# Research & Design Decisions

---
- **Feature**: `issue-300` — `/cupola:spec-init` スキルの追加
- **Discovery Scope**: Simple Addition（既存パターンの踏襲）
- **Key Findings**:
  - Rust 側の `generate_spec_directory()` が完全な参照実装として存在する
  - 既存スキル群と同一の YAML frontmatter + Markdown 形式で実装可能
  - テンプレートファイル（init.json, requirements-init.md）は既存のものをそのまま流用できる
  - `CLAUDE_CODE_ASSETS` に1エントリ追加することで `cupola init` からインストール可能になる

---

## Research Log

### 既存の Rust 実装の調査

- **Context**: スキルが Rust 側と同一の出力形式を生成するために、参照実装の詳細を把握する必要があった。
- **Sources Consulted**: `src/adapter/outbound/init_file_generator.rs`
- **Findings**:
  - `generate_spec_directory(issue_number, issue_body, language)` が spec ディレクトリを初期化する
  - `init.json` テンプレートの `{{FEATURE_NAME}}`, `{{TIMESTAMP}}`, `{{LANGUAGE}}` を置換して `spec.json` を生成する
  - `requirements-init.md` テンプレートの `{{PROJECT_DESCRIPTION}}` を置換して `requirements.md` を生成する
  - ディレクトリが既存の場合は `Ok(false)` を返してスキップする（冪等）
  - テンプレートが存在しない場合はインラインのフォールバック値を使用する
- **Implications**: スキルは同じロジックを Markdown 指示書として記述すれば良い。テンプレートは既存のものを流用できる。

### 既存スキルのパターン調査

- **Context**: 新スキルを一貫したパターンで実装するために既存スキルの構造を調査した。
- **Sources Consulted**: `.claude/commands/cupola/spec-design.md`, `spec-impl.md`, `steering.md`, `fix.md`
- **Findings**:
  - 全スキルが `---\ndescription: ...\nallowed-tools: ...\nargument-hint: ...\n---` の YAML frontmatter を持つ
  - 引数は `$1`, `$2` で参照される
  - 処理ステップは `## Execution Steps` セクションに記述される
  - エラーハンドリングは `## Safety & Fallback` セクションに記述される
  - 出力の説明は `## Output Description` セクションに記述される
- **Implications**: spec-init.md も同一構造に従って記述する。

### CLAUDE_CODE_ASSETS の調査

- **Context**: `cupola init` で新スキルをインストールできるようにするための仕組みを調査した。
- **Sources Consulted**: `src/adapter/outbound/init_file_generator.rs:7-35`
- **Findings**:
  - `CLAUDE_CODE_ASSETS` は `&[(&str, &str)]` の静的スライスで、`(相対パス, コンテンツ)` のタプル配列
  - コンテンツは `include_str!()` マクロでコンパイル時に埋め込まれる
  - `install_claude_code_assets()` がこのスライスを走査して各ファイルを書き込む
  - `upgrade=false` の場合、既存ファイルはスキップされる
- **Implications**: `spec-init.md` のエントリを1行追加するだけで完了する。テスト（`install_claude_code_assets_creates_files` 等）は新スキルも自動的にカバーされる。

### 言語設定の解決方法

- **Context**: スキルがどこから言語設定を取得すべきかを検討した。
- **Sources Consulted**: `src/bootstrap/toml_config_loader.rs`, `.cupola/cupola.toml` の構造
- **Findings**:
  - `cupola.toml` は `language = "ja"` のようなフィールドを持つ可能性がある
  - スキル（Markdown）から TOML を解析するのは複雑なため、Bash の `grep` で簡易的に抽出するか、引数を優先する
  - Rust デーモンは `language` を設定から渡しているが、スキルではデフォルト `ja` が適切
- **Implications**: 引数 `$2` > `cupola.toml` の grep 抽出 > デフォルト `ja` の優先順位で解決する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 新スキルファイルのみ追加 | spec-init.md を `.claude/commands/cupola/` に追加 | シンプル、既存パターンに準拠 | `cupola init` でのインストールに Rust 変更が必要 | 採用 |
| Rust に委譲 | スキルから `cupola` CLI を呼び出す | Rust の実装を再利用できる | CLI の spec-init サブコマンドが存在しない | 否採用（過剰実装） |
| spec-design に統合 | spec-design スキルに init 処理を含める | スキル数が増えない | 単一責任原則に違反する | 否採用 |

## Design Decisions

### Decision: スキルが直接ファイルを生成する

- **Context**: スキルが spec ディレクトリを初期化する際、どのように実装するかの選択
- **Alternatives Considered**:
  1. Rust CLI (`cupola`) にサブコマンドを追加して呼び出す
  2. スキルが直接 Write ツールでファイルを生成する
- **Selected Approach**: スキルが直接 Write ツールでファイルを生成する
- **Rationale**: Rust CLI への変更はより大きなスコープになる。スキルは既存のテンプレートファイルにアクセスできるため、スキル内で完結させることができる。既存の spec-design.md も同様のパターンで spec.json を更新している。
- **Trade-offs**: Rust と Markdown の2実装が存在する（DRY 原則に反するが、責任分離により許容）
- **Follow-up**: 将来的に Rust CLI の `spec-init` サブコマンドとして統合できる（#298 の方向性に沿う）

### Decision: テンプレートファイルへのフォールバック

- **Context**: テンプレートファイルが存在しない場合の処理
- **Alternatives Considered**:
  1. テンプレートが存在しない場合にエラーを返す
  2. インラインのフォールバック値を使用する（Rust 実装と同様）
- **Selected Approach**: インラインのフォールバック値を使用する
- **Rationale**: Rust の `generate_spec_directory()` と同じ動作をすることで、テンプレートが壊れていても動作する。ロバスト性のためのフォールバック。
- **Trade-offs**: スキルファイルに若干の重複が生じる
- **Follow-up**: なし

## Risks & Mitigations

- Rust 側と生成形式が乖離するリスク — テンプレートファイルを共有することで緩和（スキルも init.json を参照）
- `cupola init` で新スキルがインストールされないリスク — CLAUDE_CODE_ASSETS にエントリを追加することで対処
- スキルの Markdown 指示が不明確で Claude が誤ったファイルを生成するリスク — 明示的な JSON 構造とフォールバック値をスキル内に記述することで軽減

## References

- `src/adapter/outbound/init_file_generator.rs` — Rust 側の参照実装
- `.cupola/settings/templates/specs/init.json` — spec.json テンプレート
- `.cupola/settings/templates/specs/requirements-init.md` — requirements.md テンプレート
- `.claude/commands/cupola/spec-design.md` — 既存スキルのパターン参照
