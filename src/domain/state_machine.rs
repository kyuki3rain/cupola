use crate::domain::event::Event;
use crate::domain::state::State;

#[derive(Debug, thiserror::Error)]
#[error("invalid transition from {from:?} with event {event:?}")]
pub struct TransitionError {
    pub from: State,
    pub event: Event,
}

pub struct StateMachine;

impl StateMachine {
    pub fn transition(current: &State, event: &Event) -> Result<State, TransitionError> {
        match (current, event) {
            // idle
            (State::Idle, Event::IssueDetected { .. }) => Ok(State::Initialized),

            // initialized
            (State::Initialized, Event::InitializationCompleted) => Ok(State::DesignRunning),
            (State::Initialized, Event::RetryExhausted) => Ok(State::Cancelled),

            // design_running
            (State::DesignRunning, Event::ProcessCompletedWithPr) => Ok(State::DesignReviewWaiting),
            (State::DesignRunning, Event::ProcessFailed) => Ok(State::DesignRunning),
            (State::DesignRunning, Event::RetryExhausted) => Ok(State::Cancelled),

            // design_review_waiting (merge takes priority)
            (State::DesignReviewWaiting, Event::DesignPrMerged) => Ok(State::ImplementationRunning),
            (State::DesignReviewWaiting, Event::FixingRequired) => Ok(State::DesignFixing),

            // design_fixing
            (State::DesignFixing, Event::ProcessCompleted) => Ok(State::DesignReviewWaiting),
            (State::DesignFixing, Event::ProcessFailed) => Ok(State::DesignFixing),
            (State::DesignFixing, Event::RetryExhausted) => Ok(State::Cancelled),

            // implementation_running
            (State::ImplementationRunning, Event::ProcessCompletedWithPr) => {
                Ok(State::ImplementationReviewWaiting)
            }
            (State::ImplementationRunning, Event::ProcessFailed) => {
                Ok(State::ImplementationRunning)
            }
            (State::ImplementationRunning, Event::RetryExhausted) => Ok(State::Cancelled),

            // implementation_review_waiting (merge takes priority)
            (State::ImplementationReviewWaiting, Event::ImplementationPrMerged) => {
                Ok(State::Completed)
            }
            (State::ImplementationReviewWaiting, Event::FixingRequired) => {
                Ok(State::ImplementationFixing)
            }

            // implementation_fixing
            (State::ImplementationFixing, Event::ProcessCompleted) => {
                Ok(State::ImplementationReviewWaiting)
            }
            (State::ImplementationFixing, Event::ProcessFailed) => Ok(State::ImplementationFixing),
            (State::ImplementationFixing, Event::RetryExhausted) => Ok(State::Cancelled),

            // Issue close from any non-terminal state
            (s, Event::IssueClosed) if !s.is_terminal() => Ok(State::Cancelled),

            _ => Err(TransitionError {
                from: *current,
                event: event.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Valid transitions ===

    #[test]
    fn idle_to_initialized_on_issue_detected() {
        let result =
            StateMachine::transition(&State::Idle, &Event::IssueDetected { issue_number: 42 });
        assert_eq!(result.unwrap(), State::Initialized);
    }

    #[test]
    fn initialized_to_design_running_on_init_completed() {
        let result = StateMachine::transition(&State::Initialized, &Event::InitializationCompleted);
        assert_eq!(result.unwrap(), State::DesignRunning);
    }

    #[test]
    fn initialized_to_cancelled_on_retry_exhausted() {
        let result = StateMachine::transition(&State::Initialized, &Event::RetryExhausted);
        assert_eq!(result.unwrap(), State::Cancelled);
    }

    #[test]
    fn design_running_to_review_waiting_on_process_completed_with_pr() {
        let result =
            StateMachine::transition(&State::DesignRunning, &Event::ProcessCompletedWithPr);
        assert_eq!(result.unwrap(), State::DesignReviewWaiting);
    }

    #[test]
    fn design_running_stays_on_process_failed() {
        let result = StateMachine::transition(&State::DesignRunning, &Event::ProcessFailed);
        assert_eq!(result.unwrap(), State::DesignRunning);
    }

    #[test]
    fn design_running_to_cancelled_on_retry_exhausted() {
        let result = StateMachine::transition(&State::DesignRunning, &Event::RetryExhausted);
        assert_eq!(result.unwrap(), State::Cancelled);
    }

    #[test]
    fn design_review_waiting_to_impl_running_on_merge() {
        let result = StateMachine::transition(&State::DesignReviewWaiting, &Event::DesignPrMerged);
        assert_eq!(result.unwrap(), State::ImplementationRunning);
    }

    #[test]
    fn design_review_waiting_to_fixing_on_unresolved() {
        let result = StateMachine::transition(&State::DesignReviewWaiting, &Event::FixingRequired);
        assert_eq!(result.unwrap(), State::DesignFixing);
    }

    #[test]
    fn design_fixing_to_review_waiting_on_process_completed() {
        let result = StateMachine::transition(&State::DesignFixing, &Event::ProcessCompleted);
        assert_eq!(result.unwrap(), State::DesignReviewWaiting);
    }

    #[test]
    fn design_fixing_stays_on_process_failed() {
        let result = StateMachine::transition(&State::DesignFixing, &Event::ProcessFailed);
        assert_eq!(result.unwrap(), State::DesignFixing);
    }

    #[test]
    fn design_fixing_to_cancelled_on_retry_exhausted() {
        let result = StateMachine::transition(&State::DesignFixing, &Event::RetryExhausted);
        assert_eq!(result.unwrap(), State::Cancelled);
    }

    #[test]
    fn impl_running_to_review_waiting_on_process_completed_with_pr() {
        let result = StateMachine::transition(
            &State::ImplementationRunning,
            &Event::ProcessCompletedWithPr,
        );
        assert_eq!(result.unwrap(), State::ImplementationReviewWaiting);
    }

    #[test]
    fn impl_running_stays_on_process_failed() {
        let result = StateMachine::transition(&State::ImplementationRunning, &Event::ProcessFailed);
        assert_eq!(result.unwrap(), State::ImplementationRunning);
    }

    #[test]
    fn impl_running_to_cancelled_on_retry_exhausted() {
        let result =
            StateMachine::transition(&State::ImplementationRunning, &Event::RetryExhausted);
        assert_eq!(result.unwrap(), State::Cancelled);
    }

    #[test]
    fn impl_review_waiting_to_completed_on_merge() {
        let result = StateMachine::transition(
            &State::ImplementationReviewWaiting,
            &Event::ImplementationPrMerged,
        );
        assert_eq!(result.unwrap(), State::Completed);
    }

    #[test]
    fn impl_review_waiting_to_fixing_on_unresolved() {
        let result =
            StateMachine::transition(&State::ImplementationReviewWaiting, &Event::FixingRequired);
        assert_eq!(result.unwrap(), State::ImplementationFixing);
    }

    #[test]
    fn impl_fixing_to_review_waiting_on_process_completed() {
        let result =
            StateMachine::transition(&State::ImplementationFixing, &Event::ProcessCompleted);
        assert_eq!(result.unwrap(), State::ImplementationReviewWaiting);
    }

    #[test]
    fn impl_fixing_stays_on_process_failed() {
        let result = StateMachine::transition(&State::ImplementationFixing, &Event::ProcessFailed);
        assert_eq!(result.unwrap(), State::ImplementationFixing);
    }

    #[test]
    fn impl_fixing_to_cancelled_on_retry_exhausted() {
        let result = StateMachine::transition(&State::ImplementationFixing, &Event::RetryExhausted);
        assert_eq!(result.unwrap(), State::Cancelled);
    }

    // === IssueClosed from all non-terminal states ===

    #[test]
    fn issue_closed_cancels_all_non_terminal_states() {
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
            let result = StateMachine::transition(&state, &Event::IssueClosed);
            assert_eq!(
                result.unwrap(),
                State::Cancelled,
                "IssueClosed from {state:?} should go to Cancelled"
            );
        }
    }

    // === Invalid transitions ===

    #[test]
    fn issue_closed_from_terminal_is_invalid() {
        assert!(StateMachine::transition(&State::Completed, &Event::IssueClosed).is_err());
        assert!(StateMachine::transition(&State::Cancelled, &Event::IssueClosed).is_err());
    }

    #[test]
    fn idle_rejects_invalid_events() {
        let invalid_events = [
            Event::InitializationCompleted,
            Event::ProcessCompletedWithPr,
            Event::ProcessCompleted,
            Event::ProcessFailed,
            Event::RetryExhausted,
            Event::DesignPrMerged,
            Event::ImplementationPrMerged,
            Event::FixingRequired,
        ];
        for event in &invalid_events {
            assert!(
                StateMachine::transition(&State::Idle, event).is_err(),
                "Idle + {event:?} should be invalid"
            );
        }
    }

    #[test]
    fn completed_rejects_all_events() {
        let events = [
            Event::IssueDetected { issue_number: 1 },
            Event::InitializationCompleted,
            Event::ProcessCompletedWithPr,
            Event::ProcessCompleted,
            Event::ProcessFailed,
            Event::RetryExhausted,
            Event::DesignPrMerged,
            Event::ImplementationPrMerged,
            Event::FixingRequired,
        ];
        for event in &events {
            assert!(
                StateMachine::transition(&State::Completed, event).is_err(),
                "Completed + {event:?} should be invalid"
            );
        }
    }

    #[test]
    fn cancelled_rejects_all_events() {
        let events = [
            Event::IssueDetected { issue_number: 1 },
            Event::InitializationCompleted,
            Event::ProcessCompletedWithPr,
            Event::ProcessCompleted,
            Event::ProcessFailed,
            Event::RetryExhausted,
            Event::DesignPrMerged,
            Event::ImplementationPrMerged,
            Event::FixingRequired,
        ];
        for event in &events {
            assert!(
                StateMachine::transition(&State::Cancelled, event).is_err(),
                "Cancelled + {event:?} should be invalid"
            );
        }
    }

    #[test]
    fn transition_error_contains_from_and_event() {
        let err = StateMachine::transition(&State::Idle, &Event::ProcessFailed).unwrap_err();
        assert_eq!(err.from, State::Idle);
        assert_eq!(err.event, Event::ProcessFailed);
    }
}
