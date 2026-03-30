# Requirements Document

## Introduction
cupola.toml に `model` 設定を追加し、Claude Code 起動時に `--model` フラグとして渡す機能を実装する。これにより、ユーザーはプロジェクト単位でデフォルトの Claude モデルを指定でき、コストや速度の要件に応じたモデル選択が可能になる。

## Requirements

### Requirement 1: cupola.toml での model 設定
**Objective:** ユーザーとして、cupola.toml に `model` を指定したい。プロジェクトごとにデフォルトの Claude モデルを設定できるようにするため。

#### Acceptance Criteria
1. When `model = "opus"` が cupola.toml に記述されている場合、Cupola shall Config の model フィールドに "opus" を格納する
2. When `model` が cupola.toml に記述されていない場合、Cupola shall デフォルト値 "sonnet" を使用する
3. The CupolaToml shall `model` フィールドを `Option<String>` として定義する

### Requirement 2: Config への model フィールド追加
**Objective:** 開発者として、Config 値オブジェクトに model 情報を保持したい。アプリケーション層以降で統一的にモデル名を参照できるようにするため。

#### Acceptance Criteria
1. The Config shall `model: String` フィールドを保持する
2. When `default_with_repo` で Config を生成する場合、Cupola shall model のデフォルト値を "sonnet" に設定する
3. When `into_config` で CupolaToml を Config に変換する場合、Cupola shall toml の model 値を Config に反映する

### Requirement 3: ClaudeCodeRunner trait への model パラメータ追加
**Objective:** 開発者として、ClaudeCodeRunner trait の spawn メソッドに model パラメータを渡せるようにしたい。アダプター層でモデル指定を実装できるようにするため。

#### Acceptance Criteria
1. The ClaudeCodeRunner trait shall spawn メソッドの引数に `model: &str` を含む
2. The ClaudeCodeRunner の全ての実装 shall 更新された trait シグネチャに準拠する

### Requirement 4: Claude Code プロセスへの --model フラグ付与
**Objective:** ユーザーとして、指定したモデルで Claude Code が起動されることを期待する。コスト・速度の要件に合ったモデルが使用されるようにするため。

#### Acceptance Criteria
1. When ClaudeCodeProcess が spawn される場合、Cupola shall `--model` フラグと指定されたモデル名をコマンド引数に追加する
2. When model に "sonnet" が指定されている場合、Cupola shall `--model sonnet` を引数に含める
3. When model に "opus" が指定されている場合、Cupola shall `--model opus` を引数に含める

### Requirement 5: 既存機能との互換性
**Objective:** ユーザーとして、model 設定を追加しても既存の機能が壊れないことを期待する。後方互換性を確保するため。

#### Acceptance Criteria
1. When model が cupola.toml に未指定の場合、Cupola shall 既存の動作と同等に "sonnet" をデフォルトとして動作する
2. The 既存の全てのテスト shall model フィールド追加後もパスする
