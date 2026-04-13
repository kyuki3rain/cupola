# Requirements Document

## Introduction

本仕様は、`cupola cleanup` コマンドがデーモン稼働中にエラー終了すべきというドキュメント上の仕様と、実際の実装（警告のみ表示してクリーンアップを継続する）のミスマッチを修正するためのものである。

現状のバグにより、デーモンが動作中にクリーンアップを実行した場合、DB・worktree・ブランチ・PIDファイルの不整合やデータ破損が発生するリスクがある。本修正により、ドキュメント（`docs/commands/cleanup.md`）に定義された正しい動作を実装する。

## Requirements

### Requirement 1: デーモン稼働状態の検出

**Objective:** As a Cupola CLI ユーザー, I want クリーンアップ実行前にデーモンの稼働状態を正確に検出すること, so that デーモン稼働中の誤ったクリーンアップによるデータ破損を防止できる。

#### Acceptance Criteria

1. When `cupola cleanup` が実行され、PIDファイルが存在し有効なPID値を含む場合, the Cleanup Command Handler shall `PidFilePort::read_pid()` を呼び出してPIDを取得する。
2. When PIDの取得に成功した場合, the Cleanup Command Handler shall `PidFilePort::is_process_alive(pid)` を呼び出してプロセスの生存を確認する。
3. If PIDファイルが存在しない場合, the Cleanup Command Handler shall デーモン非稼働とみなし、クリーンアップ処理を継続する。
4. If PIDファイルの読み取りにエラーが発生した場合, the Cleanup Command Handler shall デーモン非稼働とみなし、クリーンアップ処理を継続する。

### Requirement 2: デーモン稼働中のエラー終了

**Objective:** As a Cupola CLI ユーザー, I want デーモンが稼働中の場合にクリーンアップがエラーで終了すること, so that ドキュメントの仕様通りにデータ保護が保証される。

#### Acceptance Criteria

1. When デーモンが稼働中であることが確認された場合, the Cleanup Command Handler shall `eprintln!` を使用して標準エラー出力に `"Error: cupola is running (pid={pid}). Run \`cupola stop\` first."` というメッセージを出力する。
2. When デーモン稼働中のエラーメッセージを出力した場合, the Cleanup Command Handler shall `Err(...)` を返してメインプロセスが非ゼロの終了コードで終了する。
3. While デーモンが稼働中であると確認された場合, the Cleanup Command Handler shall クリーンアップのユースケース（`CleanupUseCase::execute()`）を呼び出さない。

### Requirement 3: デーモン非稼働時の正常動作

**Objective:** As a Cupola CLI ユーザー, I want デーモンが稼働していない場合に従来通りクリーンアップが実行されること, so that 安全な状態でのクリーンアップ機能が維持される。

#### Acceptance Criteria

1. If デーモンが稼働していないことが確認された場合, the Cleanup Command Handler shall 既存のクリーンアップ処理（DB接続・`CleanupUseCase`実行・結果出力）を継続して実行する。
2. If PIDファイルが存在するがプロセスが生存していない（ステールPIDファイル）場合, the Cleanup Command Handler shall デーモン非稼働とみなし、クリーンアップ処理を継続する。
