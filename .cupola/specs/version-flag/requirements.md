# Requirements Document

## Introduction

cupola CLI に `--version` / `-V` フラグを追加し、ユーザーが実行中の cupola のバージョンを確認できるようにする。バグ報告・デバッグ・前提条件チェックなど、バージョン情報の確認が必要なあらゆる場面で利用される。

## Requirements

### Requirement 1: --version フラグによるバージョン表示

**Objective:** cupola ユーザーとして、`cupola --version` コマンドを実行したときにバージョン情報を確認したい。これにより、バグ報告やデバッグ時に実行環境のバージョンを正確に伝えられるようになる。

#### Acceptance Criteria

1. When ユーザーが `cupola --version` を実行したとき、the cupola CLI shall `cupola <バージョン番号>` の形式でバージョン情報を標準出力に表示する
2. When ユーザーが `cupola --version` を実行したとき、the cupola CLI shall 表示するバージョン番号として `Cargo.toml` の `version` フィールドの値を使用する
3. When ユーザーが `cupola --version` を実行したとき、the cupola CLI shall バージョン情報を表示した後、終了コード 0 で終了する

### Requirement 2: -V ショートフラグによるバージョン表示

**Objective:** cupola ユーザーとして、`cupola -V` のショートフラグでも同様にバージョン情報を確認したい。これにより、タイピングコストを削減して素早くバージョン確認ができるようになる。

#### Acceptance Criteria

1. When ユーザーが `cupola -V` を実行したとき、the cupola CLI shall `cupola --version` と同一の出力を標準出力に表示する
2. When ユーザーが `cupola -V` を実行したとき、the cupola CLI shall バージョン情報を表示した後、終了コード 0 で終了する

### Requirement 3: バージョン情報の自動取得

**Objective:** cupola の開発者として、バージョン番号を手動で更新する必要なく、`Cargo.toml` の `version` フィールドを唯一の真実のソースとして使いたい。これにより、バージョン番号の二重管理による不整合を防ぐ。

#### Acceptance Criteria

1. The cupola CLI shall `env!("CARGO_PKG_VERSION")` マクロを用いてコンパイル時に `Cargo.toml` の `version` フィールドからバージョン番号を取得する
2. The cupola CLI shall `Cargo.toml` の `version` フィールドが変更されたとき、再コンパイル後の `--version` / `-V` の出力に新しいバージョン番号を反映させる
