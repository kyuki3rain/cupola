use std::path::Path;

/// doctor チェックに必要な最小設定情報
pub struct DoctorConfigSummary {
    pub owner: String,
    pub repo: String,
    pub default_branch: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("設定ファイルが見つかりません: {path}")]
    NotFound { path: String },
    #[error("設定ファイルの読み込みに失敗しました: {path}: {reason}")]
    ReadFailed { path: String, reason: String },
    #[error("設定ファイルのパースに失敗しました: {path}: {reason}")]
    ParseFailed { path: String, reason: String },
    #[error("必須フィールドが不足しています: {field}")]
    MissingField { field: String },
    #[error("設定のバリデーションに失敗しました: {reason}")]
    ValidationFailed { reason: String },
}

pub trait ConfigLoader: Send + Sync {
    fn load(&self, path: &Path) -> Result<DoctorConfigSummary, ConfigLoadError>;
}
