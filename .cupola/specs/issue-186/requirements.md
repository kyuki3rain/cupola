# 要件定義: doctor の再設計

## はじめに

`cupola doctor` を `cupola start` の readiness 基準で再設計する。現状の doctor は cupola.toml、git、gh、ラベル、steering、DB を一律に確認しているが、`cupola start` の起動可否という観点で整理されていない。本仕様では診断結果を「Start Readiness（起動可否）」と「Operational Readiness（運用品質）」の2群に分類し、各チェック結果に remediation（修復方法）を付加する。

## 要件

### Requirement 1: 診断セクション分類

**Objective:** 開発者として、`doctor` の出力が Start Readiness と Operational Readiness の2群に明確に分けられることを望む。それにより、何を直せば `cupola start` できるかを一目で把握できる。

#### 受け入れ条件

1. When `cupola doctor` が実行される, the doctor system shall 出力を "Start Readiness" セクションと "Operational Readiness" セクションの2群に分けて表示する
2. The doctor system shall Start Readiness セクションに FAIL が1件以上ある場合、`cupola start` が現状不可能であることをユーザーに伝える
3. The doctor system shall FAIL を `cupola start` が高確率で即失敗する項目にのみ割り当て、それ以外の問題は WARN として扱う
4. The doctor system shall WARN のみの場合は `cupola start` は可能であるが運用上の注意が必要であることを示す

### Requirement 2: Start Readiness チェック

**Objective:** 開発者として、`cupola start` の成功に必要な全前提条件が事前に検査されることを望む。それにより、起動失敗の原因を事前に特定できる。

#### 受け入れ条件

1. When `cupola.toml` が存在しない, the doctor system shall Start Readiness セクションで FAIL を返す
2. When `cupola.toml` が TOML としてパースできる, the doctor system shall `into_config + Config::validate()` まで実行し、validate 失敗時に FAIL を返す
3. When git CLI がシステムにインストールされていない, the doctor system shall Start Readiness セクションで FAIL を返す
4. When `gh auth token` 相当の GitHub トークン取得に失敗する, the doctor system shall Start Readiness セクションで FAIL を返す（`gh auth status` が成功していても）
5. When `claude` CLI が存在しないまたは実行不可能である, the doctor system shall Start Readiness セクションで FAIL を返す
6. When `.cupola/cupola.db` が存在しない, the doctor system shall Start Readiness セクションで FAIL を返す

### Requirement 3: Operational Readiness チェック

**Objective:** 開発者として、起動後に問題化しやすい運用上の前提条件も確認されることを望む。それにより、起動後のトラブルを事前に防ぐことができる。

#### 受け入れ条件

1. When `.claude/commands/cupola/` または `.cupola/settings/` が欠落している, the doctor system shall Operational Readiness セクションで WARN を返す（FAIL ではない）
2. When `.cupola/steering/` にファイルが存在しない, the doctor system shall Operational Readiness セクションで WARN を返す（FAIL ではない）
3. When `agent:ready` ラベルがリポジトリに存在しない, the doctor system shall Operational Readiness セクションで WARN を返す（FAIL ではない）
4. When `weight:light` または `weight:heavy` ラベルが欠落している, the doctor system shall Operational Readiness セクションで WARN を返す

### Requirement 4: Remediation 表示

**Objective:** 開発者として、各チェック結果に具体的な修復方法が示されることを望む。それにより、`doctor` を実行するだけで次のアクションが即座に分かる。

#### 受け入れ条件

1. The doctor system shall 各チェック結果に remediation を付加して表示する
2. When チェック結果が `cupola init` で修復できる, the doctor system shall remediation として `cupola init` の実行を提示する
3. When チェック結果が手動対応を必要とする, the doctor system shall remediation として具体的なコマンドまたは手順（例: `gh auth login`, `brew install claude`）を提示する
4. When チェック結果が `cupola init` または手動対応のいずれかで修復できる, the doctor system shall 両方の選択肢を remediation として提示する
5. The doctor system shall `cupola init` 未実行、`init` 部分失敗、手動設定不足を区別できる remediation を表示する

### Requirement 5: テストカバレッジ

**Objective:** 開発者として、新仕様に合わせた網羅的なテストが整備されることを望む。それにより、今後の変更による回帰を防止できる。

#### 受け入れ条件

1. The doctor system shall 既存ユニットテストが新しいセクション構造（`DoctorSection`）と severity に合わせて更新される
2. The doctor system shall 新規チェック項目（claude CLI、GitHub token readiness、assets 存在確認）に対するユニットテストが追加される
3. When `DoctorUseCase::run()` が実行される, the doctor system shall Start Readiness と Operational Readiness の両セクションのチェック結果を含むリストを返す
4. The doctor system shall 各チェックの FAIL・WARN・OK の境界条件がテストでカバーされる
