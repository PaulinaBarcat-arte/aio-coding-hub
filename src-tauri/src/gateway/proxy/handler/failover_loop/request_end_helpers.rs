//! Usage: Small helpers to emit request-end events and enqueue request logs consistently.

use super::super::super::logging::enqueue_request_log_with_backpressure;
use super::super::super::RequestLogEnqueueArgs;
use crate::gateway::events::{emit_request_event, FailoverAttempt};
use crate::gateway::manager::GatewayAppState;

pub(super) struct RequestEndArgs<'a> {
    pub(super) state: &'a GatewayAppState,
    pub(super) trace_id: &'a str,
    pub(super) cli_key: &'a str,
    pub(super) method: &'a str,
    pub(super) path: &'a str,
    pub(super) query: Option<&'a str>,
    pub(super) excluded_from_stats: bool,
    pub(super) status: Option<u16>,
    pub(super) error_category: Option<&'static str>,
    pub(super) error_code: Option<&'static str>,
    pub(super) duration_ms: u128,
    pub(super) event_ttfb_ms: Option<u128>,
    pub(super) log_ttfb_ms: Option<u128>,
    pub(super) attempts: &'a [FailoverAttempt],
    pub(super) special_settings_json: Option<String>,
    pub(super) session_id: Option<String>,
    pub(super) requested_model: Option<String>,
    pub(super) created_at_ms: i64,
    pub(super) created_at: i64,
    pub(super) usage_metrics: Option<crate::usage::UsageMetrics>,
    pub(super) usage: Option<crate::usage::UsageExtract>,
}

pub(super) async fn emit_request_event_and_enqueue_request_log(args: RequestEndArgs<'_>) {
    let query = args.query.map(str::to_string);
    let (attempts, attempts_json) = if args.attempts.is_empty() {
        (Vec::new(), "[]".to_string())
    } else {
        let attempts = args.attempts.to_vec();
        let attempts_json = serde_json::to_string(&attempts).unwrap_or_else(|_| "[]".to_string());
        (attempts, attempts_json)
    };

    let trace_id = args.trace_id.to_string();
    let cli_key = args.cli_key.to_string();
    let method = args.method.to_string();
    let path = args.path.to_string();

    emit_request_event(
        &args.state.app,
        trace_id.clone(),
        cli_key.clone(),
        method.clone(),
        path.clone(),
        query.clone(),
        args.status,
        args.error_category,
        args.error_code,
        args.duration_ms,
        args.event_ttfb_ms,
        attempts,
        args.usage_metrics,
    );

    enqueue_request_log_with_backpressure(
        &args.state.app,
        &args.state.db,
        &args.state.log_tx,
        RequestLogEnqueueArgs {
            trace_id,
            cli_key,
            session_id: args.session_id,
            method,
            path,
            query,
            excluded_from_stats: args.excluded_from_stats,
            special_settings_json: args.special_settings_json,
            status: args.status,
            error_code: args.error_code,
            duration_ms: args.duration_ms,
            ttfb_ms: args.log_ttfb_ms,
            attempts_json,
            requested_model: args.requested_model,
            created_at_ms: args.created_at_ms,
            created_at: args.created_at,
            usage: args.usage,
        },
    )
    .await;
}
