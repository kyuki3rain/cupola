# Implementation Plan

- [ ] 1. Linkヘッダーパース関数を実装する
- [ ] 1.1 `parse_link_header()` 関数を `github_rest_client.rs` に追加する
  - RFC 5988 形式の Link ヘッダー文字列（例: `<url>; rel="next", <url>; rel="last"`）を受け取る
  - 指定した `rel` に対応する URL を `Option<String>` で返す
  - `rel="next"` が存在しない場合は `None` を返す
  - _Requirements: 1.3_

- [ ] 1.2 (P) `parse_link_header()` のユニットテストを追加する
  - `rel="next"` を含む Link ヘッダーから URL を正しく抽出できる
  - `rel="next"` が存在しない場合に `None` を返す
  - `rel="next"` と `rel="last"` が混在する場合に `rel="next"` のみを返す
  - _Requirements: 3.4_

- [ ] 2. `TIMELINE_MAX_PAGES` 定数を定義する
  - `github_rest_client.rs` にモジュールレベルの定数 `TIMELINE_MAX_PAGES: usize = 10` を追加する
  - _Requirements: 2.3_

- [ ] 3. `fetch_label_actor_login()` にページネーションループを実装する
- [ ] 3.1 レスポンスの headers を body 消費前に取得するよう変更する
  - 既存の単一リクエスト処理を `loop` 構造に変更する
  - `resp.headers().get("link")` を `resp.json()` 呼び出しの前に実行し `String` としてクローンする
  - _Requirements: 1.4_

- [ ] 3.2 全ページのイベントを収集するループロジックを実装する
  - `all_events: Vec<serde_json::Value>` を用意し各ページの events を `extend` する
  - `parse_link_header()` で `rel="next"` URL を取得し、存在すれば次ページへ、なければ `break`
  - ページカウンターを用いて `TIMELINE_MAX_PAGES` に達したら `break` する
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.2_

- [ ] 4. ページネーションの統合テストを追加する
- [ ] 4.1 (P) 2ページにまたがるイベント取得テストを実装する
  - mock HTTP サーバー（`wiremock` または `mockito`）で 1 ページ目に `Link: <url2>; rel="next"` ヘッダーを付与したレスポンスを返す
  - ページ 1 とページ 2 のイベントが両方とも取得されることを検証する
  - _Requirements: 3.1_

- [ ] 4.2 (P) ページ 2 の `labeled` イベントから actor login を取得するテストを実装する
  - ページ 2 に `{ "event": "labeled", "label": { "name": "agent:ready" }, "actor": { "login": "user" } }` を含める
  - `fetch_label_actor_login()` が `Ok(Some("user"))` を返すことを検証する
  - _Requirements: 3.2_

- [ ] 4.3 (P) `rel="next"` なしのループ終了テストを実装する
  - Link ヘッダーなしのレスポンスを返す mock を用意する
  - ループが 1 回で終了し、単一ページのイベントだけで検索が完了することを検証する
  - _Requirements: 3.3_

- [ ] 4.4 (P) 最大ページ数上限テストを実装する
  - `TIMELINE_MAX_PAGES` + 1 ページ分のレスポンスを返す mock を用意する
  - 最大ページ数分のリクエストのみが送信され、超過分のリクエストが送信されないことを検証する
  - _Requirements: 3.5_
