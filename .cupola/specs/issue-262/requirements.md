# Requirements Document

## Introduction

Cupola デーモンの polling loop が panic した場合、プロセスが異常終了し PID ファイルが残存してしまう。その結果、次回起動時に「cupola is already running」エラーが発生し、手動で PID ファイルを削除しなければ再起動できない問題がある。本仕様では `std::panic::set_hook` を用いてパニック発生時に PID ファイルを自動クリーンアップする機能を設計する。

## Requirements

### Requirement 1: パニック時 PID ファイル自動クリーンアップ

**Objective:** Cupola 運用者として、デーモンがパニックした際も PID ファイルが自動的に削除されることを望む。これにより手動介入なしに次回起動が可能になる。

#### Acceptance Criteria

1. When [デーモンの polling loop が panic を発生させた], the [Cupola daemon] shall [PID ファイルを削除する]
2. When [panic が発生した], the [Cupola daemon] shall [panic 情報 (メッセージ・ファイル・行番号) をエラーレベルのログとして記録する]
3. When [panic hook が実行された後], the [Cupola daemon] shall [デフォルト panic handler を呼び出し通常の panic 動作（スタックトレース出力・プロセス終了）を保つ]
4. If [PID ファイルの削除操作が失敗した], the [Cupola daemon] shall [削除エラーを無視し元の panic を再伝播させる]
5. The [Cupola daemon] shall [PID ファイルを書き込んだ直後にフォアグラウンドモードとデーモンモードの両方で panic hook を設定する]

### Requirement 2: テスト検証

**Objective:** 開発者として、パニック時 PID クリーンアップ機能が正しく動作することをテストで確認できることを望む。

#### Acceptance Criteria

1. When [パニックを意図的に発生させる], the [テストスイート] shall [PID ファイルが削除されていることを検証する]
2. When [panic hook が実行された], the [テストスイート] shall [panic メッセージがログに記録されることを確認する]
3. The [テストスイート] shall [install_panic_hook 関数を単体テストとして独立して検証できる形で実装する]
