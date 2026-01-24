//! Usage: Provider gating helpers (circuit allow/skip + event emission).

use super::context::CommonCtx;
use crate::circuit_breaker;
use crate::gateway::events::{emit_circuit_event, emit_circuit_transition, GatewayCircuitEvent};
use crate::gateway::util::now_unix_seconds;

pub(super) struct ProviderGateInput<'a> {
    pub(super) ctx: CommonCtx<'a>,
    pub(super) provider_id: i64,
    pub(super) provider_name_base: &'a String,
    pub(super) provider_base_url_display: &'a String,
    pub(super) earliest_available_unix: &'a mut Option<i64>,
    pub(super) skipped_open: &'a mut usize,
    pub(super) skipped_cooldown: &'a mut usize,
}

pub(super) struct ProviderGateAllow {
    pub(super) circuit_after: circuit_breaker::CircuitSnapshot,
}

pub(super) fn gate_provider(input: ProviderGateInput<'_>) -> Option<ProviderGateAllow> {
    let ProviderGateInput {
        ctx,
        provider_id,
        provider_name_base,
        provider_base_url_display,
        earliest_available_unix,
        skipped_open,
        skipped_cooldown,
    } = input;

    let state = ctx.state;
    let now_unix = now_unix_seconds() as i64;
    let allow = state.circuit.should_allow(provider_id, now_unix);
    if let Some(t) = allow.transition.as_ref() {
        emit_circuit_transition(
            &state.app,
            ctx.trace_id,
            ctx.cli_key,
            provider_id,
            provider_name_base,
            provider_base_url_display,
            t,
            now_unix,
        );
    }
    if allow.allow {
        return Some(ProviderGateAllow {
            circuit_after: allow.after.clone(),
        });
    }

    let snap = allow.after;
    let reason = if snap.state == circuit_breaker::CircuitState::Open {
        *skipped_open = skipped_open.saturating_add(1);
        "SKIP_OPEN"
    } else {
        *skipped_cooldown = skipped_cooldown.saturating_add(1);
        "SKIP_COOLDOWN"
    };

    if let Some(until) = snap.cooldown_until.or(snap.open_until) {
        if until > now_unix {
            *earliest_available_unix = Some(match *earliest_available_unix {
                Some(cur) => cur.min(until),
                None => until,
            });
        }
    }

    emit_circuit_event(
        &state.app,
        GatewayCircuitEvent {
            trace_id: ctx.trace_id.clone(),
            cli_key: ctx.cli_key.clone(),
            provider_id,
            provider_name: provider_name_base.clone(),
            base_url: provider_base_url_display.clone(),
            prev_state: snap.state.as_str(),
            next_state: snap.state.as_str(),
            failure_count: snap.failure_count,
            failure_threshold: snap.failure_threshold,
            open_until: snap.open_until,
            cooldown_until: snap.cooldown_until,
            reason,
            ts: now_unix,
        },
    );

    None
}
