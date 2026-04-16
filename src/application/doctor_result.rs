/// 診断セクション: Start Readiness か Operational Readiness か
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorSection {
    StartReadiness,
    OperationalReadiness,
}

/// 個別チェックのステータス
pub enum CheckStatus {
    Ok(String),
    Warn(String),
    Fail(String),
}

/// 1 件のチェック結果
pub struct DoctorCheckResult {
    pub section: DoctorSection,
    pub name: String,
    pub status: CheckStatus,
    /// 修復方法（None の場合は修復不要）
    pub remediation: Option<String>,
}
