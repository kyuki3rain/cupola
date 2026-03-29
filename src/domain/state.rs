#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Initialized,
    DesignRunning,
    DesignReviewWaiting,
    DesignFixing,
    ImplementationRunning,
    ImplementationReviewWaiting,
    ImplementationFixing,
    Completed,
    Cancelled,
}

impl State {
    pub fn needs_process(&self) -> bool {
        matches!(
            self,
            Self::DesignRunning
                | Self::DesignFixing
                | Self::ImplementationRunning
                | Self::ImplementationFixing
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    pub fn is_review_waiting(&self) -> bool {
        matches!(
            self,
            Self::DesignReviewWaiting | Self::ImplementationReviewWaiting
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_process_returns_true_for_running_and_fixing_states() {
        assert!(State::DesignRunning.needs_process());
        assert!(State::DesignFixing.needs_process());
        assert!(State::ImplementationRunning.needs_process());
        assert!(State::ImplementationFixing.needs_process());
    }

    #[test]
    fn needs_process_returns_false_for_other_states() {
        assert!(!State::Idle.needs_process());
        assert!(!State::Initialized.needs_process());
        assert!(!State::DesignReviewWaiting.needs_process());
        assert!(!State::ImplementationReviewWaiting.needs_process());
        assert!(!State::Completed.needs_process());
        assert!(!State::Cancelled.needs_process());
    }

    #[test]
    fn is_terminal_returns_true_for_completed_and_cancelled() {
        assert!(State::Completed.is_terminal());
        assert!(State::Cancelled.is_terminal());
    }

    #[test]
    fn is_terminal_returns_false_for_non_terminal_states() {
        let non_terminal = [
            State::Idle,
            State::Initialized,
            State::DesignRunning,
            State::DesignReviewWaiting,
            State::DesignFixing,
            State::ImplementationRunning,
            State::ImplementationReviewWaiting,
            State::ImplementationFixing,
        ];
        for state in non_terminal {
            assert!(!state.is_terminal(), "{state:?} should not be terminal");
        }
    }

    #[test]
    fn is_review_waiting_returns_true_for_waiting_states() {
        assert!(State::DesignReviewWaiting.is_review_waiting());
        assert!(State::ImplementationReviewWaiting.is_review_waiting());
    }

    #[test]
    fn is_review_waiting_returns_false_for_other_states() {
        let non_waiting = [
            State::Idle,
            State::Initialized,
            State::DesignRunning,
            State::DesignFixing,
            State::ImplementationRunning,
            State::ImplementationFixing,
            State::Completed,
            State::Cancelled,
        ];
        for state in non_waiting {
            assert!(
                !state.is_review_waiting(),
                "{state:?} should not be review_waiting"
            );
        }
    }
}
