//! Usage: Shared context types for `failover_loop` internal submodules.

use super::super::super::abort_guard::RequestAbortGuard;
use crate::circuit_breaker;
use crate::gateway::events::FailoverAttempt;
use crate::gateway::manager::GatewayAppState;
use crate::gateway::response_fixer;
use axum::response::Response;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(super) const MAX_NON_SSE_BODY_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) struct CommonCtx<'a> {
    pub(super) state: &'a GatewayAppState,
    pub(super) cli_key: &'a String,
    pub(super) forwarded_path: &'a String,
    pub(super) method_hint: &'a String,
    pub(super) query: &'a Option<String>,
    pub(super) trace_id: &'a String,
    pub(super) started: Instant,
    pub(super) created_at_ms: i64,
    pub(super) created_at: i64,
    pub(super) session_id: &'a Option<String>,
    pub(super) requested_model: &'a Option<String>,
    pub(super) effective_sort_mode_id: Option<i64>,
    pub(super) special_settings: &'a Arc<Mutex<Vec<serde_json::Value>>>,
    pub(super) provider_cooldown_secs: i64,
    pub(super) upstream_first_byte_timeout_secs: u32,
    pub(super) upstream_first_byte_timeout: Option<Duration>,
    pub(super) upstream_stream_idle_timeout: Option<Duration>,
    pub(super) upstream_request_timeout_non_streaming: Option<Duration>,
    pub(super) max_attempts_per_provider: u32,
    pub(super) enable_response_fixer: bool,
    pub(super) response_fixer_stream_config: response_fixer::ResponseFixerConfig,
    pub(super) response_fixer_non_stream_config: response_fixer::ResponseFixerConfig,
    pub(super) introspection_body: &'a [u8],
}

#[derive(Clone, Copy)]
pub(super) struct ProviderCtx<'a> {
    pub(super) provider_id: i64,
    pub(super) provider_name_base: &'a String,
    pub(super) provider_base_url_base: &'a String,
    pub(super) provider_index: u32,
    pub(super) session_reuse: Option<bool>,
}

#[derive(Clone, Copy)]
pub(super) struct AttemptCtx<'a> {
    pub(super) attempt_index: u32,
    pub(super) retry_index: u32,
    pub(super) attempt_started_ms: u128,
    pub(super) attempt_started: Instant,
    pub(super) circuit_before: &'a circuit_breaker::CircuitSnapshot,
}

pub(super) struct LoopState<'a> {
    pub(super) attempts: &'a mut Vec<FailoverAttempt>,
    pub(super) failed_provider_ids: &'a mut HashSet<i64>,
    pub(super) last_error_category: &'a mut Option<&'static str>,
    pub(super) last_error_code: &'a mut Option<&'static str>,
    pub(super) circuit_snapshot: &'a mut circuit_breaker::CircuitSnapshot,
    pub(super) abort_guard: &'a mut RequestAbortGuard,
}

pub(super) enum LoopControl {
    ContinueRetry,
    BreakRetry,
    Return(Response),
}
