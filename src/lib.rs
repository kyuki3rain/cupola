rust_i18n::i18n!("locales", fallback = "en");

pub mod adapter;
pub mod application;
pub mod bootstrap;
pub mod domain;

#[cfg(test)]
mod i18n_tests {
    #[test]
    fn en_design_starting() {
        let result = rust_i18n::t!("issue_comment.design_starting", locale = "en").to_string();
        assert_eq!(result, "Starting design");
    }

    #[test]
    fn ja_design_starting() {
        let result = rust_i18n::t!("issue_comment.design_starting", locale = "ja").to_string();
        assert_eq!(result, "設計を開始します");
    }

    #[test]
    fn en_retry_exhausted_interpolation() {
        let result = rust_i18n::t!(
            "issue_comment.retry_exhausted",
            locale = "en",
            count = 3,
            error = "timeout"
        )
        .to_string();
        assert!(result.contains("3"), "count should be interpolated");
        assert!(result.contains("timeout"), "error should be interpolated");
        assert!(result.contains("Retry limit"));
    }

    #[test]
    fn unknown_locale_falls_back_to_en() {
        let result =
            rust_i18n::t!("issue_comment.unknown_error", locale = "unknown_lang").to_string();
        assert_eq!(result, "unknown");
    }
}
