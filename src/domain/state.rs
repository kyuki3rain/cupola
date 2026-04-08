use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    InitializeRunning,
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
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    pub fn is_review_waiting(&self) -> bool {
        matches!(
            self,
            Self::DesignReviewWaiting | Self::ImplementationReviewWaiting
        )
    }

    /// All variants for enumeration in tests
    pub fn all() -> [State; 10] {
        [
            State::Idle,
            State::InitializeRunning,
            State::DesignRunning,
            State::DesignReviewWaiting,
            State::DesignFixing,
            State::ImplementationRunning,
            State::ImplementationReviewWaiting,
            State::ImplementationFixing,
            State::Completed,
            State::Cancelled,
        ]
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            State::Idle => "idle",
            State::InitializeRunning => "initialize_running",
            State::DesignRunning => "design_running",
            State::DesignReviewWaiting => "design_review_waiting",
            State::DesignFixing => "design_fixing",
            State::ImplementationRunning => "implementation_running",
            State::ImplementationReviewWaiting => "implementation_review_waiting",
            State::ImplementationFixing => "implementation_fixing",
            State::Completed => "completed",
            State::Cancelled => "cancelled",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown state: {0}")]
pub struct UnknownState(String);

impl FromStr for State {
    type Err = UnknownState;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "idle" => Ok(State::Idle),
            "initialize_running" => Ok(State::InitializeRunning),
            "design_running" => Ok(State::DesignRunning),
            "design_review_waiting" => Ok(State::DesignReviewWaiting),
            "design_fixing" => Ok(State::DesignFixing),
            "implementation_running" => Ok(State::ImplementationRunning),
            "implementation_review_waiting" => Ok(State::ImplementationReviewWaiting),
            "implementation_fixing" => Ok(State::ImplementationFixing),
            "completed" => Ok(State::Completed),
            "cancelled" => Ok(State::Cancelled),
            _ => Err(UnknownState(s.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-1.S.1
    #[test]
    fn initialize_running_variant_exists() {
        let s = State::InitializeRunning;
        assert_eq!(s.to_string(), "initialize_running");
    }

    /// T-1.S.2
    #[test]
    fn is_terminal_returns_true_for_completed_and_cancelled() {
        assert!(State::Completed.is_terminal());
        assert!(State::Cancelled.is_terminal());
        let non_terminal = [
            State::Idle,
            State::InitializeRunning,
            State::DesignRunning,
            State::DesignReviewWaiting,
            State::DesignFixing,
            State::ImplementationRunning,
            State::ImplementationReviewWaiting,
            State::ImplementationFixing,
        ];
        for s in non_terminal {
            assert!(!s.is_terminal(), "{s:?} should not be terminal");
        }
    }

    /// T-1.S.3
    #[test]
    fn is_review_waiting_returns_true_only_for_waiting_states() {
        assert!(State::DesignReviewWaiting.is_review_waiting());
        assert!(State::ImplementationReviewWaiting.is_review_waiting());
        let others = [
            State::Idle,
            State::InitializeRunning,
            State::DesignRunning,
            State::DesignFixing,
            State::ImplementationRunning,
            State::ImplementationFixing,
            State::Completed,
            State::Cancelled,
        ];
        for s in others {
            assert!(!s.is_review_waiting(), "{s:?} should not be review_waiting");
        }
    }

    /// T-1.S.4
    #[test]
    fn state_roundtrips_through_display_and_fromstr() {
        for s in State::all() {
            let displayed = s.to_string();
            let parsed: State = displayed.parse().expect("should parse back");
            assert_eq!(parsed, s, "roundtrip failed for {s:?}");
        }
    }

    /// T-1.S.5
    #[test]
    fn state_has_exactly_10_variants_with_canonical_snake_case() {
        let variants = State::all();
        assert_eq!(variants.len(), 10);
        let expected = [
            ("idle", State::Idle),
            ("initialize_running", State::InitializeRunning),
            ("design_running", State::DesignRunning),
            ("design_review_waiting", State::DesignReviewWaiting),
            ("design_fixing", State::DesignFixing),
            ("implementation_running", State::ImplementationRunning),
            (
                "implementation_review_waiting",
                State::ImplementationReviewWaiting,
            ),
            ("implementation_fixing", State::ImplementationFixing),
            ("completed", State::Completed),
            ("cancelled", State::Cancelled),
        ];
        for (key, variant) in expected {
            assert_eq!(variant.to_string(), key);
        }
    }
}
