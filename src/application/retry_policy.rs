#[derive(Debug, PartialEq, Eq)]
pub enum RetryDecision {
    Retry,
    Exhausted,
}

pub struct RetryPolicy {
    max_retries: u32,
}

impl RetryPolicy {
    pub fn new(max_retries: u32) -> Self {
        Self { max_retries }
    }

    pub fn evaluate(&self, current_retry_count: u32) -> RetryDecision {
        if current_retry_count < self.max_retries {
            RetryDecision::Retry
        } else {
            RetryDecision::Exhausted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_when_under_limit() {
        let policy = RetryPolicy::new(3);
        assert_eq!(policy.evaluate(0), RetryDecision::Retry);
        assert_eq!(policy.evaluate(1), RetryDecision::Retry);
        assert_eq!(policy.evaluate(2), RetryDecision::Retry);
    }

    #[test]
    fn exhausted_at_limit() {
        let policy = RetryPolicy::new(3);
        assert_eq!(policy.evaluate(3), RetryDecision::Exhausted);
        assert_eq!(policy.evaluate(4), RetryDecision::Exhausted);
    }

    #[test]
    fn zero_retries_always_exhausted() {
        let policy = RetryPolicy::new(0);
        assert_eq!(policy.evaluate(0), RetryDecision::Exhausted);
    }
}
