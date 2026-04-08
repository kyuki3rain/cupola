use crate::domain::fixing_problem_kind::FixingProblemKind;
use crate::domain::process_run::ProcessRunType;

/// Effects produced by `decide`. Execute phase runs them in priority order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    // Transition effects (priority 1-2)
    PostCompletedComment,
    PostCancelComment,
    PostRetryExhaustedComment,
    // Event effects
    RejectUntrustedReadyIssue,
    PostCiFixLimitComment,
    // Persistent effects
    SpawnInit,
    SwitchToImplBranch,
    SpawnProcess {
        type_: ProcessRunType,
        causes: Vec<FixingProblemKind>,
    },
    CleanupWorktree,
    CloseIssue,
}

impl Effect {
    /// Global priority for execution ordering (lower = higher priority)
    pub fn priority(&self) -> u8 {
        match self {
            // Transition effects
            Effect::PostCompletedComment
            | Effect::PostCancelComment
            | Effect::PostRetryExhaustedComment => 1,
            // Event effects
            Effect::RejectUntrustedReadyIssue | Effect::PostCiFixLimitComment => 2,
            // Persistent effects
            Effect::SpawnInit => 3,
            Effect::SwitchToImplBranch => 4,
            Effect::SpawnProcess { .. } => 5,
            Effect::CleanupWorktree => 6,
            Effect::CloseIssue => 7,
        }
    }

    /// Best-effort effects log failures but do not abort the execution chain
    pub fn is_best_effort(&self) -> bool {
        match self {
            Effect::PostCompletedComment
            | Effect::PostCancelComment
            | Effect::PostRetryExhaustedComment
            | Effect::RejectUntrustedReadyIssue
            | Effect::PostCiFixLimitComment
            | Effect::CleanupWorktree
            | Effect::CloseIssue => true,
            Effect::SpawnInit | Effect::SwitchToImplBranch | Effect::SpawnProcess { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-1.E.1: Effect enum has exactly 10 variants
    #[test]
    fn effect_enum_has_correct_variants() {
        // Just verify all variants can be constructed
        let _ = Effect::PostCompletedComment;
        let _ = Effect::PostCancelComment;
        let _ = Effect::PostRetryExhaustedComment;
        let _ = Effect::RejectUntrustedReadyIssue;
        let _ = Effect::PostCiFixLimitComment;
        let _ = Effect::SpawnInit;
        let _ = Effect::SwitchToImplBranch;
        let _ = Effect::SpawnProcess {
            type_: ProcessRunType::Design,
            causes: vec![],
        };
        let _ = Effect::CleanupWorktree;
        let _ = Effect::CloseIssue;
    }

    /// T-1.E.2: priority ordering
    #[test]
    fn effect_priority_ordering() {
        // transition effects (1)
        assert_eq!(Effect::PostCompletedComment.priority(), 1);
        assert_eq!(Effect::PostCancelComment.priority(), 1);
        assert_eq!(Effect::PostRetryExhaustedComment.priority(), 1);
        // event effects (2)
        assert_eq!(Effect::RejectUntrustedReadyIssue.priority(), 2);
        assert_eq!(Effect::PostCiFixLimitComment.priority(), 2);
        // persistent effects
        assert_eq!(Effect::SpawnInit.priority(), 3);
        assert_eq!(Effect::SwitchToImplBranch.priority(), 4);
        assert_eq!(
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![]
            }
            .priority(),
            5
        );
        assert_eq!(Effect::CleanupWorktree.priority(), 6);
        assert_eq!(Effect::CloseIssue.priority(), 7);
    }

    /// T-1.E.2: sorting by priority produces correct order
    #[test]
    fn sorting_by_priority_produces_correct_order() {
        let mut effects = [
            Effect::CloseIssue,
            Effect::PostCompletedComment,
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![],
            },
            Effect::SpawnInit,
        ];
        effects.sort_by_key(|e| e.priority());
        assert_eq!(effects[0], Effect::PostCompletedComment);
        assert_eq!(effects[1], Effect::SpawnInit);
        assert_eq!(
            effects[2],
            Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![]
            }
        );
        assert_eq!(effects[3], Effect::CloseIssue);
    }

    /// T-1.E.3: is_best_effort
    #[test]
    fn is_best_effort_returns_correct_values() {
        assert!(Effect::PostCompletedComment.is_best_effort());
        assert!(Effect::PostCancelComment.is_best_effort());
        assert!(Effect::PostRetryExhaustedComment.is_best_effort());
        assert!(Effect::RejectUntrustedReadyIssue.is_best_effort());
        assert!(Effect::PostCiFixLimitComment.is_best_effort());
        assert!(Effect::CleanupWorktree.is_best_effort());
        assert!(Effect::CloseIssue.is_best_effort());

        assert!(!Effect::SpawnInit.is_best_effort());
        assert!(!Effect::SwitchToImplBranch.is_best_effort());
        assert!(
            !Effect::SpawnProcess {
                type_: ProcessRunType::Design,
                causes: vec![]
            }
            .is_best_effort()
        );
    }
}
