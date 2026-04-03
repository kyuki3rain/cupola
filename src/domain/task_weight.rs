use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskWeight {
    Light,
    #[default]
    Medium,
    Heavy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_medium() {
        assert_eq!(TaskWeight::default(), TaskWeight::Medium);
    }

    #[test]
    fn serde_roundtrip_light() {
        let json = serde_json::to_string(&TaskWeight::Light).unwrap();
        assert_eq!(json, "\"light\"");
        let back: TaskWeight = serde_json::from_str(&json).unwrap();
        assert_eq!(back, TaskWeight::Light);
    }

    #[test]
    fn serde_roundtrip_medium() {
        let json = serde_json::to_string(&TaskWeight::Medium).unwrap();
        assert_eq!(json, "\"medium\"");
        let back: TaskWeight = serde_json::from_str(&json).unwrap();
        assert_eq!(back, TaskWeight::Medium);
    }

    #[test]
    fn serde_roundtrip_heavy() {
        let json = serde_json::to_string(&TaskWeight::Heavy).unwrap();
        assert_eq!(json, "\"heavy\"");
        let back: TaskWeight = serde_json::from_str(&json).unwrap();
        assert_eq!(back, TaskWeight::Heavy);
    }

    #[test]
    fn deserialize_all_variants() {
        assert_eq!(
            serde_json::from_str::<TaskWeight>("\"light\"").unwrap(),
            TaskWeight::Light
        );
        assert_eq!(
            serde_json::from_str::<TaskWeight>("\"medium\"").unwrap(),
            TaskWeight::Medium
        );
        assert_eq!(
            serde_json::from_str::<TaskWeight>("\"heavy\"").unwrap(),
            TaskWeight::Heavy
        );
    }
}
