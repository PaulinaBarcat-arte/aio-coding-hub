//! Usage: Finalize responses for failover loop terminal states.

use super::super::super::abort_guard::RequestAbortGuard;
use super::super::super::caches::CachedGatewayError;
use super::super::super::errors::{error_response, error_response_with_retry_after};
use super::super::super::logging::enqueue_request_log_with_backpressure;
use super::super::super::RequestLogEnqueueArgs;
use crate::gateway::events::{emit_request_event, FailoverAttempt};
use crate::gateway::manager::GatewayAppState;
use crate::gateway::response_fixer;
use crate::gateway::util::now_unix_seconds;
use axum::http::StatusCode;
use axum::response::Response;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub(super) struct AllUnavailableInput<'a> {
    pub(super) state: &'a GatewayAppState,
    pub(super) abort_guard: &'a mut RequestAbortGuard,
    pub(super) cli_key: String,
    pub(super) method_hint: String,
    pub(super) forwarded_path: String,
    pub(super) query: Option<String>,
    pub(super) trace_id: String,
    pub(super) started: Instant,
    pub(super) created_at_ms: i64,
    pub(super) created_at: i64,
    pub(super) session_id: Option<String>,
    pub(super) requested_model: Option<String>,
    pub(super) special_settings: Arc<Mutex<Vec<serde_json::Value>>>,
    pub(super) earliest_available_unix: Option<i64>,
    pub(super) skipped_open: usize,
    pub(super) skipped_cooldown: usize,
    pub(super) fingerprint_key: u64,
    pub(super) fingerprint_debug: String,
    pub(super) unavailable_fingerprint_key: u64,
    pub(super) unavailable_fingerprint_debug: String,
}

pub(super) async fn all_providers_unavailable(input: AllUnavailableInput<'_>) -> Response {
    let AllUnavailableInput {
        state,
        abort_guard,
        cli_key,
        method_hint,
        forwarded_path,
        query,
        trace_id,
        started,
        created_at_ms,
        created_at,
        session_id,
        requested_model,
        special_settings,
        earliest_available_unix,
        skipped_open,
        skipped_cooldown,
        fingerprint_key,
        fingerprint_debug,
        unavailable_fingerprint_key,
        unavailable_fingerprint_debug,
    } = input;

    let now_unix = now_unix_seconds() as i64;
    let retry_after_seconds = earliest_available_unix
        .and_then(|t| t.checked_sub(now_unix))
        .filter(|v| *v > 0)
        .map(|v| v as u64);

    let message = format!(
        "no provider available (skipped: open={skipped_open}, cooldown={skipped_cooldown}) for cli_key={cli_key}",
    );

    let resp = error_response_with_retry_after(
        StatusCode::SERVICE_UNAVAILABLE,
        trace_id.clone(),
        "GW_ALL_PROVIDERS_UNAVAILABLE",
        message.clone(),
        vec![],
        retry_after_seconds,
    );

    emit_request_event(
        &state.app,
        trace_id.clone(),
        cli_key.clone(),
        method_hint.clone(),
        forwarded_path.clone(),
        query.clone(),
        Some(StatusCode::SERVICE_UNAVAILABLE.as_u16()),
        None,
        Some("GW_ALL_PROVIDERS_UNAVAILABLE"),
        started.elapsed().as_millis(),
        None,
        vec![],
        None,
    );

    enqueue_request_log_with_backpressure(
        &state.app,
        &state.db,
        &state.log_tx,
        RequestLogEnqueueArgs {
            trace_id: trace_id.clone(),
            cli_key,
            session_id: session_id.clone(),
            method: method_hint,
            path: forwarded_path,
            query,
            excluded_from_stats: false,
            special_settings_json: response_fixer::special_settings_json(&special_settings),
            status: Some(StatusCode::SERVICE_UNAVAILABLE.as_u16()),
            error_code: Some("GW_ALL_PROVIDERS_UNAVAILABLE"),
            duration_ms: started.elapsed().as_millis(),
            ttfb_ms: None,
            attempts_json: "[]".to_string(),
            requested_model: requested_model.clone(),
            created_at_ms,
            created_at,
            usage: None,
        },
    )
    .await;

    if let Some(retry_after_seconds) = retry_after_seconds.filter(|v| *v > 0) {
        if let Ok(mut cache) = state.recent_errors.lock() {
            cache.insert_error(
                now_unix,
                unavailable_fingerprint_key,
                CachedGatewayError {
                    trace_id: trace_id.clone(),
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    error_code: "GW_ALL_PROVIDERS_UNAVAILABLE",
                    message: message.clone(),
                    retry_after_seconds: Some(retry_after_seconds),
                    expires_at_unix: now_unix.saturating_add(retry_after_seconds as i64),
                    fingerprint_debug: unavailable_fingerprint_debug.clone(),
                },
            );
            cache.insert_error(
                now_unix,
                fingerprint_key,
                CachedGatewayError {
                    trace_id: trace_id.clone(),
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    error_code: "GW_ALL_PROVIDERS_UNAVAILABLE",
                    message,
                    retry_after_seconds: Some(retry_after_seconds),
                    expires_at_unix: now_unix.saturating_add(retry_after_seconds as i64),
                    fingerprint_debug: fingerprint_debug.clone(),
                },
            );
        }
    }

    abort_guard.disarm();
    resp
}

pub(super) struct AllFailedInput<'a> {
    pub(super) state: &'a GatewayAppState,
    pub(super) abort_guard: &'a mut RequestAbortGuard,
    pub(super) attempts: Vec<FailoverAttempt>,
    pub(super) last_error_category: Option<&'static str>,
    pub(super) last_error_code: Option<&'static str>,
    pub(super) cli_key: String,
    pub(super) method_hint: String,
    pub(super) forwarded_path: String,
    pub(super) query: Option<String>,
    pub(super) trace_id: String,
    pub(super) started: Instant,
    pub(super) created_at_ms: i64,
    pub(super) created_at: i64,
    pub(super) session_id: Option<String>,
    pub(super) requested_model: Option<String>,
    pub(super) special_settings: Arc<Mutex<Vec<serde_json::Value>>>,
}

pub(super) async fn all_providers_failed(input: AllFailedInput<'_>) -> Response {
    let AllFailedInput {
        state,
        abort_guard,
        attempts,
        last_error_category,
        last_error_code,
        cli_key,
        method_hint,
        forwarded_path,
        query,
        trace_id,
        started,
        created_at_ms,
        created_at,
        session_id,
        requested_model,
        special_settings,
    } = input;

    let final_error_code = last_error_code.unwrap_or("GW_UPSTREAM_ALL_FAILED");

    let resp = error_response(
        StatusCode::BAD_GATEWAY,
        trace_id.clone(),
        final_error_code,
        format!("all providers failed for cli_key={cli_key}"),
        attempts.clone(),
    );

    emit_request_event(
        &state.app,
        trace_id.clone(),
        cli_key.clone(),
        method_hint.clone(),
        forwarded_path.clone(),
        query.clone(),
        Some(StatusCode::BAD_GATEWAY.as_u16()),
        last_error_category,
        Some(final_error_code),
        started.elapsed().as_millis(),
        None,
        attempts.clone(),
        None,
    );

    enqueue_request_log_with_backpressure(
        &state.app,
        &state.db,
        &state.log_tx,
        RequestLogEnqueueArgs {
            trace_id,
            cli_key,
            session_id: session_id.clone(),
            method: method_hint,
            path: forwarded_path,
            query,
            excluded_from_stats: false,
            special_settings_json: response_fixer::special_settings_json(&special_settings),
            status: Some(StatusCode::BAD_GATEWAY.as_u16()),
            error_code: Some(final_error_code),
            duration_ms: started.elapsed().as_millis(),
            ttfb_ms: None,
            attempts_json: serde_json::to_string(&attempts).unwrap_or_else(|_| "[]".to_string()),
            requested_model,
            created_at_ms,
            created_at,
            usage: None,
        },
    )
    .await;

    abort_guard.disarm();
    resp
}
