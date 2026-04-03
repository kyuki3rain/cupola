#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    // GitHub-originated
    IssueDetected { issue_number: u64 },
    IssueClosed,
    DesignPrMerged,
    ImplementationPrMerged,
    FixingRequired,

    // Internal
    InitializationCompleted,
    ProcessCompletedWithPr,
    ProcessCompleted,
    ProcessFailed,
    RetryExhausted,
}
