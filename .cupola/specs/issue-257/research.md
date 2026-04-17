# Research & Design Decisions

---
**Purpose**: ページネーション実装の調査・設計根拠の記録

---

## Summary

- **Feature**: Timeline API Linkヘッダーページネーション対応
- **Discovery Scope**: Extension（既存システムへの機能追加）
- **Key Findings**:
  - `fetch_label_actor_login()` は `github_rest_client.rs` の単一メソッドで完結しており、変更スコープは小さい
  - `reqwest::Response` は body 消費後に所有権が移動するため、headers を先に取り出す必要がある
  - GitHub Timeline API はタイムラインを**古い順（昇順）**で返すため、`.rev()` で逆順走査することで最新の `labeled` イベントを先頭に取得できる
  - `list_open_issues()` で既にページネーションパターンが実装されており、同プロジェクト内に参考実装がある

## Research Log

### reqwest::Response のヘッダー取得タイミング

- **Context**: `resp.json().await` を呼ぶと body が消費され、以降 `resp` 自体にアクセスできなくなる
- **Findings**:
  - `reqwest::Response::json()` は `self`（値）を受け取るため、呼び出した時点で `Response` が move される。Rust の所有権モデルにより、以降 `resp` にアクセスしようとするとコンパイルエラーになる。「headers が空になる」という動的な問題ではなく、コンパイル時に強制される制約
  - 安全策: `let link = resp.headers().get("link").and_then(|v| v.to_str().ok()).map(String::from);` を body 消費前に実行する
- **Implications**: headers 取得 → body 消費の順序を厳守する実装が必要

### Linkヘッダーパース

- **Context**: GitHub API の Link ヘッダーは RFC 5988 に準拠した形式
- **Findings**:
  - 形式: `<https://api.github.com/...?page=2>; rel="next", <https://api.github.com/...?page=5>; rel="last"`
  - 外部クレートを使わずシンプルな文字列パースで対応可能
  - `rel="next"` セグメントを分割し、`<` と `>` で URL を抽出するだけでよい
- **Implications**: 専用クレート不要。ユーティリティ関数 `fn parse_link_header(header: &str, rel: &str) -> Option<String>` で実装する（`rel` を引数に取ることで `rel="next"` 以外にも対応可能）

### 既存のページネーションパターン（list_open_issues）

- **Context**: `list_open_issues()` が octocrab の `next_page()` を使ったページネーションを実装済み
- **Findings**:
  - octocrab は Timeline API には対応していないため、直接 `reqwest` を使う必要がある
  - `list_open_issues()` のループ構造（`loop { ... break; }`）と同パターンを採用可能
- **Implications**: `fetch_label_actor_login()` も同様のループ構造で実装する

### GitHub Timeline API の特性

- **Context**: タイムラインイベントの順序と総量
- **Findings**:
  - タイムラインは**古い順（昇順）**で返される（イベント発生順）
  - 最終ページに最新イベントが含まれるため、最悪ケースでは全ページ取得が必要
  - 最大ページ数（10ページ = 最大 1000 イベント）は実用的な上限として十分
- **Implications**: 全イベントを収集してから `.rev()` で逆順走査することで、最新の `labeled` イベントを先頭に取得できる。既存ロジックは昇順 API に対して正しく動作している

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| ループで全収集してから検索 | 全ページを Vec に収集後、逆順検索 | 既存コードとの整合性が高い | メモリ使用量が多い（1000件程度は許容範囲） | 採用 |
| ページごとに検索して早期終了 | 各ページで検索し見つかったら即返却 | メモリ効率が良い | タイムラインが昇順なので最新イベントは最終ページ付近に存在 → 最新ラベルを見つけるには全ページ取得が必要なケースが多く、早期終了の恩恵が限定的 | 不採用（実装複雑化に対し恩恵少） |

## Design Decisions

### Decision: headers の取得タイミング

- **Context**: `reqwest::Response` は body 消費後に借用不可
- **Alternatives Considered**:
  1. `resp.headers().get(...)` を body 消費後に呼ぶ — コンパイルエラー
  2. body 消費前に `let link_header = resp.headers().get("link").and_then(...).map(String::from)` — 採用
- **Selected Approach**: body 消費前に Link ヘッダー文字列を `String` にクローンして保存
- **Rationale**: Rust の所有権モデルに準拠した安全な実装
- **Trade-offs**: 毎リクエストで `String::clone` が発生するが、HTTP レイテンシに対して無視できるコスト
- **Follow-up**: コンパイル時にチェック可能

### Decision: parse_link_header の配置

- **Context**: Link ヘッダーパース関数をどこに置くか
- **Alternatives Considered**:
  1. `github_rest_client.rs` 内のプライベート関数 — 採用
  2. 汎用ユーティリティモジュール — オーバーエンジニアリング（使用箇所が1箇所のみ）
- **Selected Approach**: `github_rest_client.rs` 内のモジュールプライベート関数
- **Rationale**: 使用箇所が1箇所のみ、DRY 原則より YAGNI を優先
- **Trade-offs**: 再利用性は低いが、コードの局所性が高くレビューしやすい

### Decision: 最大ページ数定数

- **Context**: 無限ループ防止の上限値
- **Alternatives Considered**:
  1. マジックナンバー `10` をコード内に直書き — 不採用（可読性低）
  2. `TIMELINE_MAX_PAGES: usize = 10` 定数として定義 — 採用
  3. 設定ファイルで変更可能にする — オーバーエンジニアリング
- **Selected Approach**: モジュール内定数として定義（`const TIMELINE_MAX_PAGES: usize = 10`）
- **Rationale**: 変更時の発見容易性を確保しつつ、設定のオーバーヘッドを避ける

## Risks & Mitigations

- GitHub API レート制限: ページネーションにより最大10リクエスト追加 → Issue 257 で対象となる長寿命 issue は少ないため実用上問題なし
- Link ヘッダーフォーマット変更: GitHub が RFC 5988 準拠を変更する可能性は低い → テストで現フォーマットを検証することで検出可能

## References

- RFC 5988 Web Linking — Link ヘッダー形式の標準仕様
- GitHub REST API Docs: Issue Timeline Events — Timeline API の動作仕様
