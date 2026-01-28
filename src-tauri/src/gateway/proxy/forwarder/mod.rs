//! Usage: Gateway proxy forwarding layer (aligns with cc-switch's Forwarder separation).

use super::request_context::RequestContext;
use axum::response::Response;

#[path = "../handler/failover_loop/mod.rs"]
mod failover_loop;

pub(super) async fn forward(ctx: RequestContext) -> Response {
    failover_loop::run(ctx).await
}
