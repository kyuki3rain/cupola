use crate::domain::process_run::ProcessRunState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubIssueState {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubIssueSnapshot {
    pub state: GithubIssueState,
    pub has_ready_label: bool,
    pub ready_label_trusted: bool,
    pub weight: Option<crate::domain::task_weight::TaskWeight>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiStatus {
    Ok,
    Failure,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PrSnapshot {
    pub state: PrState,
    pub has_review_comments: bool,
    pub ci_status: CiStatus,
    pub has_conflict: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub state: ProcessRunState,
    pub index: u32,
    pub consecutive_failures: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessesSnapshot {
    pub init: Option<ProcessSnapshot>,
    pub design: Option<ProcessSnapshot>,
    pub design_fix: Option<ProcessSnapshot>,
    pub impl_: Option<ProcessSnapshot>,
    pub impl_fix: Option<ProcessSnapshot>,
}

#[derive(Debug, Clone)]
pub struct WorldSnapshot {
    pub github_issue: GithubIssueSnapshot,
    pub design_pr: Option<PrSnapshot>,
    pub impl_pr: Option<PrSnapshot>,
    pub processes: ProcessesSnapshot,
    pub ci_fix_exhausted: bool,
}

/// Convenience builders for tests and fixtures
pub mod fixtures {
    use super::*;

    pub fn open_issue() -> GithubIssueSnapshot {
        GithubIssueSnapshot {
            state: GithubIssueState::Open,
            has_ready_label: false,
            ready_label_trusted: false,
            weight: None,
        }
    }

    pub fn open_pr() -> PrSnapshot {
        PrSnapshot {
            state: PrState::Open,
            has_review_comments: false,
            ci_status: CiStatus::Ok,
            has_conflict: false,
        }
    }

    pub fn merged_pr() -> PrSnapshot {
        PrSnapshot {
            state: PrState::Merged,
            has_review_comments: false,
            ci_status: CiStatus::Ok,
            has_conflict: false,
        }
    }

    pub fn closed_pr() -> PrSnapshot {
        PrSnapshot {
            state: PrState::Closed,
            has_review_comments: false,
            ci_status: CiStatus::Ok,
            has_conflict: false,
        }
    }

    pub fn running_process(index: u32) -> ProcessSnapshot {
        ProcessSnapshot {
            state: ProcessRunState::Running,
            index,
            consecutive_failures: 0,
        }
    }

    pub fn succeeded_process(index: u32) -> ProcessSnapshot {
        ProcessSnapshot {
            state: ProcessRunState::Succeeded,
            index,
            consecutive_failures: 0,
        }
    }

    pub fn failed_process(index: u32, consecutive_failures: u32) -> ProcessSnapshot {
        ProcessSnapshot {
            state: ProcessRunState::Failed,
            index,
            consecutive_failures,
        }
    }

    pub fn stale_process(index: u32) -> ProcessSnapshot {
        ProcessSnapshot {
            state: ProcessRunState::Stale,
            index,
            consecutive_failures: 0,
        }
    }

    /// A sane default WorldSnapshot for idle issues (for use in decide tests)
    pub fn idle() -> WorldSnapshot {
        WorldSnapshot {
            github_issue: open_issue(),
            design_pr: None,
            impl_pr: None,
            processes: ProcessesSnapshot::default(),
            ci_fix_exhausted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-1.W.1: WorldSnapshot holds the correct fields
    #[test]
    fn world_snapshot_has_correct_fields() {
        let snap = fixtures::idle();
        // Fields exist: github_issue, design_pr, impl_pr, processes, ci_fix_exhausted
        let _ = &snap.github_issue;
        let _ = &snap.design_pr;
        let _ = &snap.impl_pr;
        let _ = &snap.processes;
        let _ = snap.ci_fix_exhausted;
    }

    /// T-1.W.2: GithubIssueSnapshot holds correct fields
    #[test]
    fn github_issue_snapshot_has_correct_fields() {
        let snap = GithubIssueSnapshot {
            state: GithubIssueState::Open,
            has_ready_label: true,
            ready_label_trusted: false,
            weight: None,
        };
        assert_eq!(snap.state, GithubIssueState::Open);
        assert!(snap.has_ready_label);
        assert!(!snap.ready_label_trusted);
        assert!(snap.weight.is_none());
    }

    /// T-1.W.3: PrSnapshot holds correct fields
    #[test]
    fn pr_snapshot_has_correct_fields() {
        let pr = PrSnapshot {
            state: PrState::Open,
            has_review_comments: false,
            ci_status: CiStatus::Ok,
            has_conflict: false,
        };
        assert_eq!(pr.state, PrState::Open);
        assert!(!pr.has_review_comments);
        assert_eq!(pr.ci_status, CiStatus::Ok);
        assert!(!pr.has_conflict);

        // verify all PrState variants
        let _ = PrState::Open;
        let _ = PrState::Merged;
        let _ = PrState::Closed;

        // verify all CiStatus variants
        let _ = CiStatus::Ok;
        let _ = CiStatus::Failure;
        let _ = CiStatus::Unknown;
    }

    /// T-1.W.4: ProcessesSnapshot holds correct fields
    #[test]
    fn processes_snapshot_has_correct_fields() {
        let ps = ProcessesSnapshot {
            init: None,
            design: Some(fixtures::running_process(0)),
            design_fix: None,
            impl_: None,
            impl_fix: None,
        };
        assert!(ps.init.is_none());
        assert!(ps.design.is_some());
        assert!(ps.design_fix.is_none());
        assert!(ps.impl_.is_none());
        assert!(ps.impl_fix.is_none());
    }

    /// T-1.W.5: ProcessSnapshot holds correct fields
    #[test]
    fn process_snapshot_has_correct_fields() {
        let ps = ProcessSnapshot {
            state: ProcessRunState::Failed,
            index: 3,
            consecutive_failures: 2,
        };
        assert_eq!(ps.state, ProcessRunState::Failed);
        assert_eq!(ps.index, 3);
        assert_eq!(ps.consecutive_failures, 2);
    }

    /// T-1.W.6: idle() fixture compiles and returns a sane default
    #[test]
    fn idle_fixture_returns_sane_default() {
        let snap = fixtures::idle();
        assert_eq!(snap.github_issue.state, GithubIssueState::Open);
        assert!(!snap.github_issue.has_ready_label);
        assert!(snap.design_pr.is_none());
        assert!(snap.impl_pr.is_none());
        assert!(!snap.ci_fix_exhausted);
    }
}
