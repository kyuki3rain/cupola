# Research & Design Decisions

---
**Purpose**: 調査結果、アーキテクチャ調査、技術設計の根拠を記録する。
---

## Summary

- **Feature**: `issue-265` — CheckResult / DoctorCheckResult の統合
- **Discovery Scope**: Extension（既存コードへの軽微なリファクタリング）
- **Key Findings**:
  - `domain/check_result.rs` は commit 88066b1 にて削除済み（問題の主要部分は解決済み）
  - `doctor_use_case.rs` は992行のファイルで、型定義・ユースケースロジック・個別チェック関数・テストが混在している
  - 外部参照は `bootstrap/app.rs` の2箇所のみ（`CheckStatus`, `DoctorSection`）

## Research Log

### `domain/check_result.rs` の削除状況確認

- **Context**: Issue で言及されている `domain/check_result.rs` の削除状況を確認
- **Sources Consulted**: `git log -- src/domain/check_result.rs`, `find src/ -name "*.rs"`
- **Findings**:
  - 削除コミット: `88066b1` ("refactor: Clean Architecture 違反修正 + anyhow エラー統一")
  - 削除理由: "doctor 以外で使われていない未使用コード"
  - `domain/mod.rs` に `check_result` モジュールの参照なし
  - `grep -r "CheckResult\|CheckStatus"` で残存参照は `doctor_use_case.rs` と `bootstrap/app.rs` のみ
- **Implications**: Issue の主要課題（二重定義）は解決済み。残作業は型の専用モジュール分離のみ

### `doctor_use_case.rs` の現状分析

- **Context**: `doctor_use_case.rs` の構造と、型定義の移動が適切かを評価
- **Sources Consulted**: `src/application/doctor_use_case.rs` を直接読解
- **Findings**:
  - ファイルサイズ: 992行
  - 含まれる要素:
    1. `DoctorSection` enum（StartReadiness / OperationalReadiness）
    2. `CheckStatus` enum（Ok / Warn / Fail）
    3. `DoctorCheckResult` struct
    4. `DoctorUseCase<C, R>` struct と `impl`
    5. 個別チェック関数 8件（`check_config`, `check_git`, `check_github_token`, `check_claude`, `check_db`, `check_assets`, `check_steering`, `check_labels`）
    6. `#[cfg(test)] mod tests` ブロック（約550行）
  - steering の原則「One file, one responsibility」からは型定義とユースケースロジックを分離すべき
- **Implications**: `doctor_result.rs` を新規作成して型を移動するのが適切

### 外部参照箇所の調査

- **Context**: 型定義移動に伴う影響範囲を特定
- **Sources Consulted**: `grep -r "doctor_use_case::" src/`
- **Findings**:
  - `bootstrap/app.rs` L37: `use crate::application::doctor_use_case::{CheckStatus, DoctorUseCase};`
  - `bootstrap/app.rs` L169: `use crate::application::doctor_use_case::DoctorSection;`
  - `doctor_use_case.rs` 内のテストは `use super::*;` でアクセス（移動後も同様に動作可）
- **Implications**: `bootstrap/app.rs` の2箇所のインポートを更新するだけで済む

### ステアリング原則との整合確認

- **Context**: Clean Architecture 原則に基づき、適切なモジュール配置を検討
- **Sources Consulted**: `.cupola/steering/tech.md`, `.cupola/steering/structure.md`
- **Findings**:
  - 型定義の移動先は `application/` 層内（アーキテクチャ層の変更なし）
  - `doctor_result.rs` は `CheckStatus` などを定義するだけのシンプルなファイル
  - `application/doctor/` サブディレクトリ化は過剰設計（型定義1ファイル程度で不要）
  - `mod.rs is re-export only` の原則に従い、`doctor_result` は独立ファイルとして配置
- **Implications**: `src/application/doctor_result.rs` として作成するのが最もシンプルで適切

## Architecture Pattern Evaluation

| オプション | 説明 | 利点 | リスク/制限 |
|-----------|------|------|------------|
| 現状維持 | `doctor_use_case.rs` に型定義を残す | 変更なし | 単一責務違反、992行の大ファイル |
| `doctor_result.rs` 分離 | 型定義のみ別ファイルへ移動 | 責務が明確、steringの原則に沿う | 軽微なインポート更新が必要 |
| `application/doctor/` サブモジュール化 | ディレクトリとして分割 | 将来の拡張性 | 現状の規模には過剰設計 |

**選択**: `doctor_result.rs` 分離（シンプルで原則に沿った最小変更）

## Design Decisions

### Decision: 型定義の分離先ファイル名

- **Context**: `CheckStatus`/`DoctorCheckResult`/`DoctorSection` の移動先
- **Alternatives Considered**:
  1. `doctor_result.rs` — doctor ドメインの結果型として命名
  2. `doctor_types.rs` — 型定義であることを明示
  3. `check_result.rs` — ドメイン層にあった名前の踏襲（ただし application 層内）
- **Selected Approach**: `doctor_result.rs`
- **Rationale**: doctor ユースケースの結果型を示す名前として直感的。ドメイン層の `check_result.rs` と混同しない
- **Trade-offs**: `doctor_use_case.rs` が `doctor_result.rs` に依存するシンプルな依存関係が生まれる
- **Follow-up**: `bootstrap/app.rs` のインポートパス更新

### Decision: `doctor_use_case.rs` が型を再エクスポートするか

- **Context**: `bootstrap/app.rs` の修正量を最小化するための設計選択
- **Alternatives Considered**:
  1. `doctor_use_case.rs` で `pub use doctor_result::*` を再エクスポート → bootstrap 修正不要
  2. `bootstrap/app.rs` のインポートを直接更新 → より透明な依存関係
- **Selected Approach**: 再エクスポートなし。`bootstrap/app.rs` を直接更新
- **Rationale**: 再エクスポートは型の所在を曖昧にする。`bootstrap` は全層を知っているため、直接参照が適切
- **Trade-offs**: `bootstrap/app.rs` 修正が必要だが、変更は2行のみ

## Risks & Mitigations

- テストが `use super::*` で型にアクセスしている → `doctor_use_case.rs` が `use crate::application::doctor_result::*` を追加することで解決
- `cargo clippy` のデッドコード警告 → 型に `pub` を付けて外部公開することで回避

## References

- commit 88066b1: `domain/check_result.rs` 削除コミット
- `.cupola/steering/structure.md`: "One file, one responsibility: State in state.rs, Event in event.rs — separated"
- `.cupola/steering/tech.md`: Clean Architecture 4層構造の説明
