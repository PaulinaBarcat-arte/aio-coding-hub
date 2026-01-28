//! Usage: Error classification + standardized gateway error responses.

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use super::failover::FailoverDecision;
use super::ErrorCategory;
use crate::gateway::events::FailoverAttempt;

#[derive(Debug, Serialize)]
struct GatewayErrorResponse {
    trace_id: String,
    error_code: &'static str,
    message: String,
    attempts: Vec<FailoverAttempt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after_seconds: Option<u64>,
}

pub(super) fn classify_reqwest_error(err: &reqwest::Error) -> (ErrorCategory, &'static str) {
    if err.is_timeout() {
        return (ErrorCategory::SystemError, "GW_UPSTREAM_TIMEOUT");
    }
    if err.is_connect() {
        return (ErrorCategory::SystemError, "GW_UPSTREAM_CONNECT_FAILED");
    }
    (ErrorCategory::SystemError, "GW_INTERNAL_ERROR")
}

pub(super) fn classify_upstream_status(
    status: reqwest::StatusCode,
) -> (ErrorCategory, &'static str, FailoverDecision) {
    if status.is_server_error() {
        return (
            ErrorCategory::ProviderError,
            "GW_UPSTREAM_5XX",
            FailoverDecision::RetrySameProvider,
        );
    }

    match status.as_u16() {
        401 | 403 => (
            ErrorCategory::ProviderError,
            "GW_UPSTREAM_4XX",
            FailoverDecision::SwitchProvider,
        ),
        402 => (
            // Payment Required / insufficient balance / subscription required.
            // Align with claude-code-hub: treat as provider-side limitation and allow failover.
            ErrorCategory::ProviderError,
            "GW_UPSTREAM_4XX",
            FailoverDecision::SwitchProvider,
        ),
        404 => (
            // Resource not found is often provider-specific (path/model support mismatch).
            // Align with claude-code-hub: switch provider (do not abort the whole request).
            ErrorCategory::ResourceNotFound,
            "GW_UPSTREAM_4XX",
            FailoverDecision::SwitchProvider,
        ),
        408 | 429 => (
            ErrorCategory::ProviderError,
            "GW_UPSTREAM_4XX",
            FailoverDecision::RetrySameProvider,
        ),
        _ if status.is_client_error() => (
            // Default: allow retry + failover for upstream 4xx.
            // Non-retryable client input errors are detected separately by scanning upstream error bodies.
            ErrorCategory::ProviderError,
            "GW_UPSTREAM_4XX",
            FailoverDecision::RetrySameProvider,
        ),
        _ => (
            ErrorCategory::ProviderError,
            "GW_INTERNAL_ERROR",
            FailoverDecision::Abort,
        ),
    }
}

pub(super) fn error_response(
    status: StatusCode,
    trace_id: String,
    error_code: &'static str,
    message: String,
    attempts: Vec<FailoverAttempt>,
) -> Response {
    error_response_with_retry_after(status, trace_id, error_code, message, attempts, None)
}

pub(super) fn error_response_with_retry_after(
    status: StatusCode,
    trace_id: String,
    error_code: &'static str,
    message: String,
    attempts: Vec<FailoverAttempt>,
    retry_after_seconds: Option<u64>,
) -> Response {
    let payload = GatewayErrorResponse {
        trace_id: trace_id.clone(),
        error_code,
        message,
        attempts,
        retry_after_seconds,
    };

    let mut resp = (status, Json(payload)).into_response();

    if let Ok(v) = HeaderValue::from_str(&trace_id) {
        resp.headers_mut().insert("x-trace-id", v);
    }

    if let Some(seconds) = retry_after_seconds.filter(|v| *v > 0) {
        let value = seconds.to_string();
        if let Ok(v) = HeaderValue::from_str(&value) {
            resp.headers_mut().insert(header::RETRY_AFTER, v);
        }
    }

    resp
}

#[cfg(test)]
mod tests {
    use super::{classify_upstream_status, FailoverDecision};
    use crate::gateway::proxy::ErrorCategory;

    #[test]
    fn upstream_402_switches_provider() {
        let (category, code, decision) =
            classify_upstream_status(reqwest::StatusCode::PAYMENT_REQUIRED);
        assert!(matches!(category, ErrorCategory::ProviderError));
        assert_eq!(code, "GW_UPSTREAM_4XX");
        assert!(matches!(decision, FailoverDecision::SwitchProvider));
    }

    #[test]
    fn upstream_404_switches_provider() {
        let (category, code, decision) = classify_upstream_status(reqwest::StatusCode::NOT_FOUND);
        assert!(matches!(category, ErrorCategory::ResourceNotFound));
        assert_eq!(code, "GW_UPSTREAM_4XX");
        assert!(matches!(decision, FailoverDecision::SwitchProvider));
    }

    #[test]
    fn upstream_other_4xx_retries_then_failover() {
        let (category, code, decision) =
            classify_upstream_status(reqwest::StatusCode::UNPROCESSABLE_ENTITY);
        assert!(matches!(category, ErrorCategory::ProviderError));
        assert_eq!(code, "GW_UPSTREAM_4XX");
        assert!(matches!(decision, FailoverDecision::RetrySameProvider));
    }
}
