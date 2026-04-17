# Requirements Document

## Introduction

GitHub の Timeline API は `per_page=100` のデフォルト制限を持つ。`fetch_label_actor_login()` は現在、最初の 100 件しか取得しないため、100 件を超えるタイムラインイベントを持つ issue では `agent:ready` ラベル付与イベントが取得範囲外に押し出され、trust 判定が silent 失敗する。本仕様は Link ヘッダーページネーションを実装し、全イベントを取得できるようにする。

## Requirements

### Requirement 1: Linkヘッダーページネーションの実装

**Objective:** Cupola システムの operator として、タイムラインイベント件数に関わらず `agent:ready` ラベル付与者を正しく検出したい。そうすることで、長寿命の issue でも trust 判定が正確に行われるようにする。

#### Acceptance Criteria

1. When `fetch_label_actor_login()` が呼び出された時、the GitHub Timeline API client shall `per_page=100` でリクエストし、Link ヘッダーの `rel="next"` が存在する限り次のページを取得し続ける。
2. When 全ページの取得が完了した時、the GitHub Timeline API client shall 全ページのイベントを結合してから逆順で `labeled` イベントを検索する。
3. If レスポンスの Link ヘッダーに `rel="next"` が存在しない場合、the GitHub Timeline API client shall ページネーションを終了し取得済みのイベントのみで検索を行う。
4. The GitHub Timeline API client shall `reqwest::Response` のヘッダーを body 消費前に保存し、headers の破棄を防ぐ。

### Requirement 2: 無限ループ防止と安全性

**Objective:** Cupola システムの operator として、ページネーションが異常な状態に陥っても安全に処理を終了させたい。そうすることで、予期しないループによるシステム停止を防ぐ。

#### Acceptance Criteria

1. The GitHub Timeline API client shall 最大ページ数（10ページ）の上限を設け、上限に達した場合はそれ以降のページを取得しない。
2. If 最大ページ数上限に達した場合、the GitHub Timeline API client shall 取得済みのイベントで検索を継続し、エラーとはしない。
3. The GitHub Timeline API client shall ページネーション上限値を定数として定義し、変更を容易にする。

### Requirement 3: テスト網羅性

**Objective:** Cupola の開発者として、ページネーション実装が正しく動作することをテストで検証したい。そうすることで、リグレッションを防ぎ実装の信頼性を確保する。

#### Acceptance Criteria

1. When mock HTTP サーバーが 2 ページ以上のレスポンスを返す場合、the test suite shall 2 ページ目以降のイベントも取得できることを検証する。
2. When ページ 2 に `labeled` イベントが存在する場合、the test suite shall 対応する actor ログインが返されることを検証する。
3. When `rel="next"` が存在しない場合、the test suite shall ループが終了し単一ページのイベントだけで検索が完了することを検証する。
4. When `Link: <url>; rel="next"` ヘッダーがレスポンスに含まれる場合、the test suite shall ヘッダーが正しくパースされ次ページ URL が抽出されることを検証する。
5. When 最大ページ数に達した場合、the test suite shall それ以降のページは取得せず取得済みイベントで検索を継続することを検証する。
