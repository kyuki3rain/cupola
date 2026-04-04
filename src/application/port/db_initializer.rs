use anyhow::Result;

/// DB スキーマ初期化を抽象化する outbound ポート。
pub trait DbInitializer: Send + Sync {
    /// issues テーブルと execution_log テーブルを作成する（冪等）。
    fn init_schema(&self) -> Result<()>;
}
