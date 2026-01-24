//! Usage: Shared context types for `failover_loop` internal submodules.

use super::super::super::abort_guard::RequestAbortGuard;
use crate::circuit_breaker;
use crate::gateway::events::FailoverAttempt;
use crate::gateway::manager::GatewayAppState;
use crate::gateway::response_fixer;
use crate::gateway::util::RequestedModelLocation;
use crate::providers;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method};
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

pub(in super::super) struct FailoverLoopInput {
    pub(in super::super) state: GatewayAppState,
    pub(in super::super) cli_key: String,
    pub(in super::super) forwarded_path: String,
    pub(in super::super) req_method: Method,
    pub(in super::super) method_hint: String,
    pub(in super::super) query: Option<String>,
    pub(in super::super) trace_id: String,
    pub(in super::super) started: Instant,
    pub(in super::super) created_at_ms: i64,
    pub(in super::super) created_at: i64,
    pub(in super::super) session_id: Option<String>,
    pub(in super::super) requested_model: Option<String>,
    pub(in super::super) requested_model_location: Option<RequestedModelLocation>,
    pub(in super::super) effective_sort_mode_id: Option<i64>,
    pub(in super::super) providers: Vec<providers::ProviderForGateway>,
    pub(in super::super) session_bound_provider_id: Option<i64>,
    pub(in super::super) base_headers: HeaderMap,
    pub(in super::super) body_bytes: Bytes,
    pub(in super::super) introspection_json: Option<serde_json::Value>,
    pub(in super::super) strip_request_content_encoding_seed: bool,
    pub(in super::super) special_settings: Arc<Mutex<Vec<serde_json::Value>>>,
    pub(in super::super) provider_base_url_ping_cache_ttl_seconds: u32,
    pub(in super::super) max_attempts_per_provider: u32,
    pub(in super::super) max_providers_to_try: u32,
    pub(in super::super) provider_cooldown_secs: i64,
    pub(in super::super) upstream_first_byte_timeout_secs: u32,
    pub(in super::super) upstream_first_byte_timeout: Option<Duration>,
    pub(in super::super) upstream_stream_idle_timeout: Option<Duration>,
    pub(in super::super) upstream_request_timeout_non_streaming: Option<Duration>,
    pub(in super::super) fingerprint_key: u64,
    pub(in super::super) fingerprint_debug: String,
    pub(in super::super) unavailable_fingerprint_key: u64,
    pub(in super::super) unavailable_fingerprint_debug: String,
    pub(in super::super) abort_guard: RequestAbortGuard,
    pub(in super::super) enable_thinking_signature_rectifier: bool,
    pub(in super::super) enable_response_fixer: bool,
    pub(in super::super) response_fixer_stream_config: response_fixer::ResponseFixerConfig,
    pub(in super::super) response_fixer_non_stream_config: response_fixer::ResponseFixerConfig,
}
