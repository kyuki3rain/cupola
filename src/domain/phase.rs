use crate::domain::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Design,
    DesignFix,
    Implementation,
    ImplementationFix,
}

impl Phase {
    /// State から対応する Phase を返す。spawn 不要な State は None を返す。
    pub fn from_state(state: State) -> Option<Self> {
        match state {
            State::DesignRunning => Some(Self::Design),
            State::DesignFixing => Some(Self::DesignFix),
            State::ImplementationRunning => Some(Self::Implementation),
            State::ImplementationFixing => Some(Self::ImplementationFix),
            _ => None,
        }
    }

    /// DesignFix → Some(Design)、ImplementationFix → Some(Implementation)、
    /// それ以外 → None
    pub fn base(&self) -> Option<Self> {
        match self {
            Self::DesignFix => Some(Self::Design),
            Self::ImplementationFix => Some(Self::Implementation),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_state_design_running() {
        assert_eq!(Phase::from_state(State::DesignRunning), Some(Phase::Design));
    }

    #[test]
    fn from_state_design_fixing() {
        assert_eq!(
            Phase::from_state(State::DesignFixing),
            Some(Phase::DesignFix)
        );
    }

    #[test]
    fn from_state_implementation_running() {
        assert_eq!(
            Phase::from_state(State::ImplementationRunning),
            Some(Phase::Implementation)
        );
    }

    #[test]
    fn from_state_implementation_fixing() {
        assert_eq!(
            Phase::from_state(State::ImplementationFixing),
            Some(Phase::ImplementationFix)
        );
    }

    #[test]
    fn from_state_non_spawn_states_return_none() {
        let non_spawn = [
            State::Idle,
            State::Initialized,
            State::DesignReviewWaiting,
            State::ImplementationReviewWaiting,
            State::Completed,
            State::Cancelled,
        ];
        for state in non_spawn {
            assert_eq!(
                Phase::from_state(state),
                None,
                "{state:?} should return None"
            );
        }
    }

    #[test]
    fn base_design_fix_returns_design() {
        assert_eq!(Phase::DesignFix.base(), Some(Phase::Design));
    }

    #[test]
    fn base_implementation_fix_returns_implementation() {
        assert_eq!(Phase::ImplementationFix.base(), Some(Phase::Implementation));
    }

    #[test]
    fn base_design_returns_none() {
        assert_eq!(Phase::Design.base(), None);
    }

    #[test]
    fn base_implementation_returns_none() {
        assert_eq!(Phase::Implementation.base(), None);
    }
}
