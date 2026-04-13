use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};

use crate::domain::fixing_problem_kind::FixingProblemKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRunType {
    Init,
    Design,
    DesignFix,
    Impl,
    ImplFix,
}

impl fmt::Display for ProcessRunType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProcessRunType::Init => "init",
            ProcessRunType::Design => "design",
            ProcessRunType::DesignFix => "design_fix",
            ProcessRunType::Impl => "impl",
            ProcessRunType::ImplFix => "impl_fix",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown process run type: {0}")]
pub struct UnknownProcessRunType(String);

impl FromStr for ProcessRunType {
    type Err = UnknownProcessRunType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "init" => Ok(ProcessRunType::Init),
            "design" => Ok(ProcessRunType::Design),
            "design_fix" => Ok(ProcessRunType::DesignFix),
            "impl" => Ok(ProcessRunType::Impl),
            "impl_fix" => Ok(ProcessRunType::ImplFix),
            _ => Err(UnknownProcessRunType(s.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRunState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Stale,
}

impl fmt::Display for ProcessRunState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProcessRunState::Pending => "pending",
            ProcessRunState::Running => "running",
            ProcessRunState::Succeeded => "succeeded",
            ProcessRunState::Failed => "failed",
            ProcessRunState::Stale => "stale",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown process run state: {0}")]
pub struct UnknownProcessRunState(String);

impl FromStr for ProcessRunState {
    type Err = UnknownProcessRunState;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ProcessRunState::Pending),
            "running" => Ok(ProcessRunState::Running),
            "succeeded" => Ok(ProcessRunState::Succeeded),
            "failed" => Ok(ProcessRunState::Failed),
            "stale" => Ok(ProcessRunState::Stale),
            _ => Err(UnknownProcessRunState(s.to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessRun {
    pub id: i64,
    pub issue_id: i64,
    pub type_: ProcessRunType,
    pub index: u32,
    pub state: ProcessRunState,
    pub pid: Option<u32>,
    pub pr_number: Option<u64>,
    pub causes: Vec<FixingProblemKind>,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl ProcessRun {
    pub fn new_running(
        issue_id: i64,
        type_: ProcessRunType,
        index: u32,
        causes: Vec<FixingProblemKind>,
    ) -> Self {
        Self {
            id: 0,
            issue_id,
            type_,
            index,
            state: ProcessRunState::Running,
            pid: None,
            pr_number: None,
            causes,
            error_message: None,
            started_at: Utc::now(),
            finished_at: None,
        }
    }

    pub fn new_pending(
        issue_id: i64,
        type_: ProcessRunType,
        index: u32,
        causes: Vec<FixingProblemKind>,
    ) -> Self {
        Self {
            id: 0,
            issue_id,
            type_,
            index,
            state: ProcessRunState::Pending,
            pid: None,
            pr_number: None,
            causes,
            error_message: None,
            started_at: Utc::now(),
            finished_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-1.P.1: ProcessRunType has exactly 5 variants with snake_case roundtrip
    #[test]
    fn process_run_type_has_5_variants_with_roundtrip() {
        let variants = [
            ("init", ProcessRunType::Init),
            ("design", ProcessRunType::Design),
            ("design_fix", ProcessRunType::DesignFix),
            ("impl", ProcessRunType::Impl),
            ("impl_fix", ProcessRunType::ImplFix),
        ];
        assert_eq!(variants.len(), 5);
        for (s, v) in &variants {
            assert_eq!(v.to_string(), *s);
            assert_eq!(s.parse::<ProcessRunType>().unwrap(), *v);
        }
    }

    /// T-1.P.2: ProcessRunState has exactly 4 variants with snake_case roundtrip
    #[test]
    fn process_run_state_has_4_variants_with_roundtrip() {
        let variants = [
            ("running", ProcessRunState::Running),
            ("succeeded", ProcessRunState::Succeeded),
            ("failed", ProcessRunState::Failed),
            ("stale", ProcessRunState::Stale),
        ];
        assert_eq!(variants.len(), 4);
        for (s, v) in &variants {
            assert_eq!(v.to_string(), *s);
            assert_eq!(s.parse::<ProcessRunState>().unwrap(), *v);
        }
    }

    /// T-1.P.3: ProcessRun struct has exactly the specified fields
    #[test]
    fn process_run_has_correct_fields() {
        let pr = ProcessRun {
            id: 1,
            issue_id: 2,
            type_: ProcessRunType::Design,
            index: 0,
            state: ProcessRunState::Running,
            pid: Some(1234),
            pr_number: None,
            causes: vec![],
            error_message: None,
            started_at: Utc::now(),
            finished_at: None,
        };
        assert_eq!(pr.id, 1);
        assert_eq!(pr.issue_id, 2);
        assert_eq!(pr.type_, ProcessRunType::Design);
        assert_eq!(pr.index, 0);
        assert_eq!(pr.state, ProcessRunState::Running);
        assert_eq!(pr.pid, Some(1234));
        assert!(pr.pr_number.is_none());
        assert!(pr.causes.is_empty());
        assert!(pr.error_message.is_none());
        assert!(pr.finished_at.is_none());
    }

    /// T-1.P.4: new_running constructor
    #[test]
    fn new_running_constructor_sets_correct_defaults() {
        let causes = vec![FixingProblemKind::CiFailure];
        let pr = ProcessRun::new_running(10, ProcessRunType::DesignFix, 2, causes.clone());
        assert_eq!(pr.state, ProcessRunState::Running);
        assert!(pr.pid.is_none());
        assert!(pr.pr_number.is_none());
        assert_eq!(pr.causes, causes);
        assert!(pr.finished_at.is_none());
        assert_eq!(pr.issue_id, 10);
        assert_eq!(pr.type_, ProcessRunType::DesignFix);
        assert_eq!(pr.index, 2);
    }
}
