use std::time::Duration;

use reqwest::StatusCode;

/// GitHub HTTP API エラーをバリアント別に型付けし、リトライ可否を判断できるようにする。
#[derive(Debug, thiserror::Error)]
pub enum GitHubApiError {
    #[error("rate limit exceeded (retry after {retry_after:?})")]
    RateLimit { retry_after: Option<Duration> },
    #[error("unauthorized: check token")]
    Unauthorized,
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("server error (5xx): {status}")]
    ServerError { status: StatusCode },
    #[error("not found: {resource}")]
    NotFound { resource: String },
    #[error("other: {0}")]
    Other(#[from] anyhow::Error),
}

/// HTTP ステータスコード・ボディ・ヘッダーから `GitHubApiError` に変換する。
///
/// - 429 → `RateLimit { retry_after }`
/// - 401 → `Unauthorized`
/// - 403 → `Forbidden(body)`
/// - 404 → `NotFound { resource }`
/// - 5xx → `ServerError { status }`
/// - その他 → `Other` (anyhow でラップ)
///
/// # Preconditions
/// `status.is_success()` が false であること。
pub fn classify_http_error(
    status: StatusCode,
    body: String,
    retry_after: Option<Duration>,
    resource: &str,
) -> GitHubApiError {
    match status.as_u16() {
        429 => GitHubApiError::RateLimit { retry_after },
        401 => GitHubApiError::Unauthorized,
        403 => GitHubApiError::Forbidden(body),
        404 => GitHubApiError::NotFound {
            resource: resource.to_string(),
        },
        500..=599 => GitHubApiError::ServerError { status },
        _ => GitHubApiError::Other(anyhow::anyhow!(
            "unexpected HTTP status {}: {}",
            status,
            body
        )),
    }
}

/// `Retry-After` ヘッダーを秒数の `Duration` として解析する。
/// ヘッダーが存在しない、または数値でない場合は `None` を返す。
pub fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use reqwest::StatusCode;

    // ============================================================
    // classify_http_error ユニットテスト (Task 4.1)
    // ============================================================

    #[test]
    fn classify_429_with_retry_after_returns_rate_limit_with_duration() {
        let err = classify_http_error(
            StatusCode::TOO_MANY_REQUESTS,
            String::new(),
            Some(Duration::from_secs(30)),
            "resource",
        );
        assert!(
            matches!(err, GitHubApiError::RateLimit { retry_after: Some(d) } if d == Duration::from_secs(30)),
            "expected RateLimit with 30s retry_after"
        );
    }

    #[test]
    fn classify_429_without_retry_after_returns_rate_limit_none() {
        let err = classify_http_error(
            StatusCode::TOO_MANY_REQUESTS,
            String::new(),
            None,
            "resource",
        );
        assert!(
            matches!(err, GitHubApiError::RateLimit { retry_after: None }),
            "expected RateLimit with None retry_after"
        );
    }

    #[test]
    fn classify_401_returns_unauthorized() {
        let err = classify_http_error(StatusCode::UNAUTHORIZED, String::new(), None, "");
        assert!(
            matches!(err, GitHubApiError::Unauthorized),
            "expected Unauthorized"
        );
    }

    #[test]
    fn classify_403_returns_forbidden_with_body() {
        let body = "forbidden message".to_string();
        let err = classify_http_error(StatusCode::FORBIDDEN, body.clone(), None, "");
        assert!(
            matches!(err, GitHubApiError::Forbidden(ref msg) if msg == &body),
            "expected Forbidden with message"
        );
    }

    #[test]
    fn classify_500_returns_server_error() {
        let err = classify_http_error(StatusCode::INTERNAL_SERVER_ERROR, String::new(), None, "");
        assert!(
            matches!(err, GitHubApiError::ServerError { status } if status == StatusCode::INTERNAL_SERVER_ERROR),
            "expected ServerError with 500"
        );
    }

    #[test]
    fn classify_503_returns_server_error() {
        let err = classify_http_error(StatusCode::SERVICE_UNAVAILABLE, String::new(), None, "");
        assert!(
            matches!(err, GitHubApiError::ServerError { status } if status == StatusCode::SERVICE_UNAVAILABLE),
            "expected ServerError with 503"
        );
    }

    #[test]
    fn classify_404_returns_not_found_with_resource() {
        let err = classify_http_error(StatusCode::NOT_FOUND, String::new(), None, "my-resource");
        assert!(
            matches!(err, GitHubApiError::NotFound { ref resource } if resource == "my-resource"),
            "expected NotFound with resource name"
        );
    }

    #[test]
    fn classify_other_status_returns_other() {
        let err = classify_http_error(StatusCode::BAD_REQUEST, "bad req".to_string(), None, "");
        assert!(
            matches!(err, GitHubApiError::Other(_)),
            "expected Other for 400"
        );
    }

    #[test]
    fn github_api_error_converts_to_anyhow_and_back() {
        let err = classify_http_error(StatusCode::UNAUTHORIZED, String::new(), None, "");
        let anyhow_err: anyhow::Error = err.into();
        assert!(
            anyhow_err.downcast_ref::<GitHubApiError>().is_some(),
            "should downcast to GitHubApiError"
        );
    }

    // ============================================================
    // parse_retry_after ユニットテスト (Task 4.1)
    // ============================================================

    #[test]
    fn parse_retry_after_returns_duration_when_header_is_number() {
        let mut map = reqwest::header::HeaderMap::new();
        map.insert("retry-after", "120".parse().expect("valid header"));
        let result = parse_retry_after(&map);
        assert_eq!(result, Some(Duration::from_secs(120)));
    }

    #[test]
    fn parse_retry_after_returns_none_when_header_absent() {
        let map = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after(&map), None);
    }

    #[test]
    fn parse_retry_after_returns_none_when_header_is_non_numeric() {
        let mut map = reqwest::header::HeaderMap::new();
        map.insert(
            "retry-after",
            "Fri, 01 Jan 2027 00:00:00 GMT"
                .parse()
                .expect("valid header"),
        );
        assert_eq!(parse_retry_after(&map), None);
    }
}
