#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixingProblemKind {
    ReviewComments,
    CiFailure,
    Conflict,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_ci_failure() {
        let json = serde_json::to_string(&FixingProblemKind::CiFailure).unwrap();
        assert_eq!(json, "\"ci_failure\"");
    }

    #[test]
    fn serialize_conflict() {
        let json = serde_json::to_string(&FixingProblemKind::Conflict).unwrap();
        assert_eq!(json, "\"conflict\"");
    }

    #[test]
    fn serialize_review_comments() {
        let json = serde_json::to_string(&FixingProblemKind::ReviewComments).unwrap();
        assert_eq!(json, "\"review_comments\"");
    }

    #[test]
    fn serialize_empty_vec() {
        let v: Vec<FixingProblemKind> = vec![];
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn deserialize_roundtrip() {
        let kinds = vec![
            FixingProblemKind::ReviewComments,
            FixingProblemKind::CiFailure,
            FixingProblemKind::Conflict,
        ];
        let json = serde_json::to_string(&kinds).unwrap();
        let deserialized: Vec<FixingProblemKind> = serde_json::from_str(&json).unwrap();
        assert_eq!(kinds, deserialized);
    }
}
