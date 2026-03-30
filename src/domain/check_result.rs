/// チェック項目の結果ステータス
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    /// チェック成功
    Pass,
    /// チェック失敗
    Fail,
    /// 前提条件未達によりスキップ
    Skipped,
}

/// 個々のチェック項目の結果
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// チェック名
    pub name: String,
    /// チェック結果ステータス
    pub status: CheckStatus,
    /// 失敗時の修正手順（失敗時のみ設定）
    pub remedy: Option<String>,
}

impl CheckResult {
    pub fn pass(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Pass,
            remedy: None,
        }
    }

    pub fn fail(name: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Fail,
            remedy: Some(remedy.into()),
        }
    }

    pub fn skipped(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Skipped,
            remedy: None,
        }
    }

    pub fn is_failed(&self) -> bool {
        self.status == CheckStatus::Fail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_result_has_no_remedy() {
        let result = CheckResult::pass("git");
        assert_eq!(result.name, "git");
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.remedy.is_none());
        assert!(!result.is_failed());
    }

    #[test]
    fn fail_result_has_remedy() {
        let result = CheckResult::fail("git", "git をインストールしてください");
        assert_eq!(result.name, "git");
        assert_eq!(result.status, CheckStatus::Fail);
        assert_eq!(
            result.remedy.as_deref(),
            Some("git をインストールしてください")
        );
        assert!(result.is_failed());
    }

    #[test]
    fn skipped_result_has_no_remedy_and_not_failed() {
        let result = CheckResult::skipped("agent:ready label");
        assert_eq!(result.status, CheckStatus::Skipped);
        assert!(result.remedy.is_none());
        assert!(!result.is_failed());
    }
}
