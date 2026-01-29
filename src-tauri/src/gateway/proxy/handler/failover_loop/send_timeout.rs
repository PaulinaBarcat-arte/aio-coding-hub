//! Usage: Handle upstream send timeout inside `failover_loop::run`.

use super::*;

pub(super) async fn handle_timeout(
    ctx: CommonCtx<'_>,
    provider_ctx: ProviderCtx<'_>,
    attempt_ctx: AttemptCtx<'_>,
    loop_state: LoopState<'_>,
) -> LoopControl {
    let upstream_first_byte_timeout_secs = ctx.upstream_first_byte_timeout_secs;
    let max_attempts_per_provider = ctx.max_attempts_per_provider;

    let error_code = "GW_UPSTREAM_TIMEOUT";
    let retry_index = attempt_ctx.retry_index;
    let decision = if retry_index < max_attempts_per_provider {
        FailoverDecision::RetrySameProvider
    } else {
        FailoverDecision::SwitchProvider
    };

    let outcome = format!(
        "request_timeout: category={} code={} decision={} timeout_secs={}",
        ErrorCategory::SystemError.as_str(),
        error_code,
        decision.as_str(),
        upstream_first_byte_timeout_secs,
    );

    record_system_failure_and_decide(RecordSystemFailureArgs {
        ctx,
        provider_ctx,
        attempt_ctx,
        loop_state,
        status: None,
        error_code,
        decision,
        outcome,
        reason: "request timeout".to_string(),
    })
    .await
}
