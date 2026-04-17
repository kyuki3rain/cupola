use chrono::{DateTime, Utc};

use crate::domain::state::State;

pub(super) fn str_to_state(col_idx: usize, s: &str) -> rusqlite::Result<State> {
    s.parse::<State>().map_err(|_| {
        rusqlite::Error::InvalidColumnType(col_idx, s.to_owned(), rusqlite::types::Type::Text)
    })
}

pub(super) fn parse_sqlite_datetime(col_idx: usize, s: &str) -> rusqlite::Result<DateTime<Utc>> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|naive| naive.and_utc())
        .map_err(|_| {
            rusqlite::Error::InvalidColumnType(col_idx, s.to_owned(), rusqlite::types::Type::Text)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_to_state_valid_roundtrip() {
        for s in crate::domain::state::State::all() {
            let displayed = s.to_string();
            assert_eq!(str_to_state(0, &displayed).unwrap(), s);
        }
    }

    #[test]
    fn str_to_state_invalid_returns_error() {
        let result = str_to_state(1, "not_a_valid_state");
        assert!(result.is_err());
        if let Err(rusqlite::Error::InvalidColumnType(idx, val, _)) = result {
            assert_eq!(idx, 1);
            assert_eq!(val, "not_a_valid_state");
        } else {
            panic!("expected InvalidColumnType error");
        }
    }

    #[test]
    fn parse_sqlite_datetime_valid() {
        let result = parse_sqlite_datetime(0, "2026-04-17 10:30:00");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-04-17 10:30:00"
        );
    }

    #[test]
    fn parse_sqlite_datetime_invalid_returns_error() {
        let result = parse_sqlite_datetime(2, "not-a-datetime");
        assert!(result.is_err());
        if let Err(rusqlite::Error::InvalidColumnType(idx, val, _)) = result {
            assert_eq!(idx, 2);
            assert_eq!(val, "not-a-datetime");
        } else {
            panic!("expected InvalidColumnType error");
        }
    }
}
