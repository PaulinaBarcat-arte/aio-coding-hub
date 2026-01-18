use crate::{circuit_breaker, request_logs, session_manager, usage};
use axum::body::{Body, Bytes};
use flate2::write::GzDecoder;
use futures_core::Stream;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use super::events::{emit_circuit_transition, emit_request_event, FailoverAttempt};
use super::proxy::{
    spawn_enqueue_request_log_with_backpressure, ErrorCategory, RequestLogEnqueueArgs,
};
use super::response_fixer;
use super::util::now_unix_seconds;

pub(super) struct StreamFinalizeCtx {
    pub(super) app: tauri::AppHandle,
    pub(super) log_tx: tokio::sync::mpsc::Sender<request_logs::RequestLogInsert>,
    pub(super) circuit: Arc<circuit_breaker::CircuitBreaker>,
    pub(super) session: Arc<session_manager::SessionManager>,
    pub(super) session_id: Option<String>,
    pub(super) sort_mode_id: Option<i64>,
    pub(super) trace_id: String,
    pub(super) cli_key: String,
    pub(super) method: String,
    pub(super) path: String,
    pub(super) query: Option<String>,
    pub(super) excluded_from_stats: bool,
    pub(super) special_settings: Arc<Mutex<Vec<serde_json::Value>>>,
    pub(super) status: u16,
    pub(super) error_category: Option<&'static str>,
    pub(super) error_code: Option<&'static str>,
    pub(super) started: Instant,
    pub(super) attempts: Vec<FailoverAttempt>,
    pub(super) attempts_json: String,
    pub(super) requested_model: Option<String>,
    pub(super) created_at_ms: i64,
    pub(super) created_at: i64,
    pub(super) provider_cooldown_secs: i64,
    pub(super) provider_id: i64,
    pub(super) provider_name: String,
    pub(super) base_url: String,
}

pub(super) struct RelayBodyStream {
    rx: tokio::sync::mpsc::Receiver<Result<Bytes, reqwest::Error>>,
}

impl RelayBodyStream {
    pub(super) fn new(rx: tokio::sync::mpsc::Receiver<Result<Bytes, reqwest::Error>>) -> Self {
        Self { rx }
    }
}

impl Stream for RelayBodyStream {
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        Pin::new(&mut this.rx).poll_recv(cx)
    }
}

pub(super) struct FirstChunkStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    first: Option<Bytes>,
    rest: S,
}

impl<S> FirstChunkStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    pub(super) fn new(first: Option<Bytes>, rest: S) -> Self {
        Self { first, rest }
    }
}

impl<S> Stream for FirstChunkStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        if let Some(first) = this.first.take() {
            return Poll::Ready(Some(Ok(first)));
        }
        Pin::new(&mut this.rest).poll_next(cx)
    }
}

#[derive(Default)]
struct VecWriteBuffer {
    buf: Vec<u8>,
}

impl Write for VecWriteBuffer {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl VecWriteBuffer {
    fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }
}

struct NextFuture<'a, S: Stream + Unpin>(&'a mut S);

impl<'a, S: Stream + Unpin> Future for NextFuture<'a, S> {
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.0).poll_next(cx)
    }
}

async fn next_item<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    NextFuture(stream).await
}

pub(super) struct GunzipStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    upstream: S,
    decoder: GzDecoder<VecWriteBuffer>,
    queued: Option<Bytes>,
    pending_error: Option<reqwest::Error>,
    upstream_done: bool,
}

impl<S> GunzipStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    pub(super) fn new(upstream: S) -> Self {
        Self {
            upstream,
            decoder: GzDecoder::new(VecWriteBuffer::default()),
            queued: None,
            pending_error: None,
            upstream_done: false,
        }
    }

    fn drain_output_if_any(&mut self) {
        if self.queued.is_some() {
            return;
        }
        let out = self.decoder.get_mut().take();
        if out.is_empty() {
            return;
        }
        self.queued = Some(Bytes::from(out));
    }

    fn flush_and_drain(&mut self) {
        let _ = self.decoder.flush();
        self.drain_output_if_any();
    }
}

impl<S> Stream for GunzipStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        loop {
            if let Some(bytes) = this.queued.take() {
                return Poll::Ready(Some(Ok(bytes)));
            }

            if this.upstream_done {
                if let Some(err) = this.pending_error.take() {
                    return Poll::Ready(Some(Err(err)));
                }
                return Poll::Ready(None);
            }

            match Pin::new(&mut this.upstream).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    this.upstream_done = true;
                    this.flush_and_drain();
                    continue;
                }
                Poll::Ready(Some(Err(err))) => {
                    this.upstream_done = true;
                    this.pending_error = Some(err);
                    this.flush_and_drain();
                    continue;
                }
                Poll::Ready(Some(Ok(chunk))) => {
                    let mut had_error = false;
                    if this.decoder.write_all(chunk.as_ref()).is_err() {
                        had_error = true;
                    }
                    if this.decoder.flush().is_err() {
                        had_error = true;
                    }
                    this.drain_output_if_any();

                    if had_error {
                        // 容错：解压失败（常见于 gzip 流被提前截断）。尽可能输出已解压内容，然后直接结束流。
                        this.upstream_done = true;
                    }
                    continue;
                }
            }
        }
    }
}

pub(super) struct UsageSseTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    upstream: S,
    tracker: usage::SseUsageTracker,
    ctx: StreamFinalizeCtx,
    first_byte_ms: Option<u128>,
    idle_timeout: Option<Duration>,
    idle_sleep: Option<Pin<Box<tokio::time::Sleep>>>,
    finalized: bool,
}

impl<S, B> UsageSseTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    pub(super) fn new(
        upstream: S,
        ctx: StreamFinalizeCtx,
        idle_timeout: Option<Duration>,
        initial_first_byte_ms: Option<u128>,
    ) -> Self {
        Self {
            upstream,
            tracker: usage::SseUsageTracker::new(&ctx.cli_key),
            ctx,
            first_byte_ms: initial_first_byte_ms,
            idle_timeout,
            idle_sleep: idle_timeout.map(|d| Box::pin(tokio::time::sleep(d))),
            finalized: false,
        }
    }

    fn finalize(&mut self, error_code: Option<&'static str>) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        let duration_ms = self.ctx.started.elapsed().as_millis();
        let usage = self.tracker.finalize();
        let usage_metrics = usage.as_ref().map(|u| u.metrics.clone());
        let requested_model = self
            .ctx
            .requested_model
            .clone()
            .or_else(|| self.tracker.best_effort_model());
        let effective_error_category = if error_code == Some("GW_STREAM_ABORTED") {
            Some(ErrorCategory::ClientAbort.as_str())
        } else {
            self.ctx.error_category
        };

        let now_unix = now_unix_seconds() as i64;
        if error_code.is_some()
            && effective_error_category != Some(ErrorCategory::ClientAbort.as_str())
            && self.ctx.provider_cooldown_secs > 0
        {
            self.ctx.circuit.trigger_cooldown(
                self.ctx.provider_id,
                now_unix,
                self.ctx.provider_cooldown_secs,
            );
        }
        if error_code.is_none() && (200..300).contains(&self.ctx.status) {
            let change = self
                .ctx
                .circuit
                .record_success(self.ctx.provider_id, now_unix);
            if let Some(t) = change.transition {
                emit_circuit_transition(
                    &self.ctx.app,
                    &self.ctx.trace_id,
                    &self.ctx.cli_key,
                    self.ctx.provider_id,
                    &self.ctx.provider_name,
                    &self.ctx.base_url,
                    &t,
                    now_unix,
                );
            }
            if let Some(session_id) = self.ctx.session_id.as_deref() {
                self.ctx.session.bind_success(
                    &self.ctx.cli_key,
                    session_id,
                    self.ctx.provider_id,
                    self.ctx.sort_mode_id,
                    now_unix,
                );
            }
        } else if effective_error_category == Some(ErrorCategory::ProviderError.as_str()) {
            let change = self
                .ctx
                .circuit
                .record_failure(self.ctx.provider_id, now_unix);
            if let Some(t) = change.transition {
                emit_circuit_transition(
                    &self.ctx.app,
                    &self.ctx.trace_id,
                    &self.ctx.cli_key,
                    self.ctx.provider_id,
                    &self.ctx.provider_name,
                    &self.ctx.base_url,
                    &t,
                    now_unix,
                );
            }
        }

        emit_request_event(
            &self.ctx.app,
            self.ctx.trace_id.clone(),
            self.ctx.cli_key.clone(),
            self.ctx.method.clone(),
            self.ctx.path.clone(),
            self.ctx.query.clone(),
            Some(self.ctx.status),
            effective_error_category,
            error_code,
            duration_ms,
            self.first_byte_ms,
            self.ctx.attempts.clone(),
            usage_metrics,
        );

        spawn_enqueue_request_log_with_backpressure(
            self.ctx.app.clone(),
            self.ctx.log_tx.clone(),
            RequestLogEnqueueArgs {
                trace_id: self.ctx.trace_id.clone(),
                cli_key: self.ctx.cli_key.clone(),
                session_id: self.ctx.session_id.clone(),
                method: self.ctx.method.clone(),
                path: self.ctx.path.clone(),
                query: self.ctx.query.clone(),
                excluded_from_stats: self.ctx.excluded_from_stats,
                special_settings_json: response_fixer::special_settings_json(
                    &self.ctx.special_settings,
                ),
                status: Some(self.ctx.status),
                error_code,
                duration_ms,
                ttfb_ms: self.first_byte_ms,
                attempts_json: self.ctx.attempts_json.clone(),
                requested_model,
                created_at_ms: self.ctx.created_at_ms,
                created_at: self.ctx.created_at,
                usage,
            },
        );
    }
}

impl<S, B> Stream for UsageSseTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    type Item = Result<B, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        let next = Pin::new(&mut this.upstream).poll_next(cx);

        match next {
            Poll::Pending => {
                if let Some(timer) = this.idle_sleep.as_mut() {
                    if timer.as_mut().poll(cx).is_ready() {
                        this.finalize(Some("GW_STREAM_IDLE_TIMEOUT"));
                        return Poll::Ready(None);
                    }
                }
                Poll::Pending
            }
            Poll::Ready(None) => {
                this.finalize(this.ctx.error_code);
                Poll::Ready(None)
            }
            Poll::Ready(Some(Ok(chunk))) => {
                if this.first_byte_ms.is_none() {
                    this.first_byte_ms = Some(this.ctx.started.elapsed().as_millis());
                }
                if let Some(d) = this.idle_timeout {
                    this.idle_sleep = Some(Box::pin(tokio::time::sleep(d)));
                }
                this.tracker.ingest_chunk(chunk.as_ref());
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.finalize(Some("GW_STREAM_ERROR"));
                Poll::Ready(Some(Err(err)))
            }
        }
    }
}

impl<S, B> Drop for UsageSseTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    fn drop(&mut self) {
        if !self.finalized {
            self.finalize(Some("GW_STREAM_ABORTED"));
        }
    }
}

const SSE_RELAY_BUFFER_CAPACITY: usize = 32;

pub(super) fn spawn_usage_sse_relay_body<S>(
    upstream: S,
    ctx: StreamFinalizeCtx,
    idle_timeout: Option<Duration>,
    initial_first_byte_ms: Option<u128>,
) -> Body
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin + Send + 'static,
{
    let (tx, rx) =
        tokio::sync::mpsc::channel::<Result<Bytes, reqwest::Error>>(SSE_RELAY_BUFFER_CAPACITY);

    let mut tee = UsageSseTeeStream::new(upstream, ctx, idle_timeout, initial_first_byte_ms);

    tokio::spawn(async move {
        let mut forwarded_chunks: i64 = 0;
        let mut forwarded_bytes: i64 = 0;
        let mut client_abort_detected_by: Option<&'static str> = None;

        loop {
            tokio::select! {
                // 如果客户端提前断开，但上游短时间没有新 chunk，就会卡在 next_item().await。
                // 这里通过监听 rx 端被 drop 来更早感知断开，避免误记 GW_STREAM_ABORTED。
                _ = tx.closed() => {
                    client_abort_detected_by = Some("rx_closed");
                    break;
                }
                item = next_item(&mut tee) => {
                    let Some(item) = item else {
                        break;
                    };

                    match item {
                        Ok(chunk) => {
                            let chunk_len = chunk.len().min(i64::MAX as usize) as i64;

                            if tx.send(Ok(chunk)).await.is_err() {
                                client_abort_detected_by = Some("send_failed");
                                break;
                            }

                            forwarded_chunks = forwarded_chunks.saturating_add(1);
                            forwarded_bytes = forwarded_bytes.saturating_add(chunk_len);
                        }
                        Err(err) => {
                            // 尽力把流错误透传给客户端
                            let _ = tx.send(Err(err)).await;
                            break;
                        }
                    }
                }
            }
        }

        if let Some(detected_by) = client_abort_detected_by {
            let duration_ms = tee.ctx.started.elapsed().as_millis().min(i64::MAX as u128) as i64;
            let ttfb_ms = tee.first_byte_ms.and_then(|v| {
                if v >= duration_ms as u128 {
                    return None;
                }
                Some(v.min(i64::MAX as u128) as i64)
            });

            if let Ok(mut guard) = tee.ctx.special_settings.lock() {
                guard.push(serde_json::json!({
                    "type": "client_abort",
                    "scope": "stream",
                    "reason": "client_disconnected",
                    "detected_by": detected_by,
                    "duration_ms": duration_ms,
                    "ttfb_ms": ttfb_ms,
                    "forwarded_chunks": forwarded_chunks,
                    "forwarded_bytes": forwarded_bytes,
                    "ts": now_unix_seconds() as i64,
                }));
            }

            // 对齐 claude-code-hub：client abort 不等价于 request failed。
            // 这里按“已开始处理但客户端提前断开”收敛，不写入 GW_STREAM_ABORTED。
            tee.finalize(None);
        }
    });

    Body::from_stream(RelayBodyStream::new(rx))
}

pub(super) struct UsageBodyBufferTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    upstream: S,
    ctx: StreamFinalizeCtx,
    first_byte_ms: Option<u128>,
    buffer: Vec<u8>,
    max_bytes: usize,
    truncated: bool,
    total_timeout: Option<Duration>,
    total_sleep: Option<Pin<Box<tokio::time::Sleep>>>,
    finalized: bool,
}

impl<S, B> UsageBodyBufferTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    pub(super) fn new(
        upstream: S,
        ctx: StreamFinalizeCtx,
        max_bytes: usize,
        total_timeout: Option<Duration>,
    ) -> Self {
        let remaining = total_timeout.and_then(|d| d.checked_sub(ctx.started.elapsed()));
        Self {
            upstream,
            ctx,
            first_byte_ms: None,
            buffer: Vec::new(),
            max_bytes,
            truncated: false,
            total_timeout,
            total_sleep: remaining.map(|d| Box::pin(tokio::time::sleep(d))),
            finalized: false,
        }
    }

    fn finalize(&mut self, error_code: Option<&'static str>) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        let duration_ms = self.ctx.started.elapsed().as_millis();
        let usage = if self.truncated || self.buffer.is_empty() {
            None
        } else {
            usage::parse_usage_from_json_bytes(&self.buffer)
        };
        let usage_metrics = usage.as_ref().map(|u| u.metrics.clone());
        let requested_model = self.ctx.requested_model.clone().or_else(|| {
            if self.truncated || self.buffer.is_empty() {
                None
            } else {
                usage::parse_model_from_json_bytes(&self.buffer)
            }
        });
        let effective_error_category = if error_code == Some("GW_STREAM_ABORTED") {
            Some(ErrorCategory::ClientAbort.as_str())
        } else {
            self.ctx.error_category
        };

        let now_unix = now_unix_seconds() as i64;
        if error_code.is_some()
            && effective_error_category != Some(ErrorCategory::ClientAbort.as_str())
            && self.ctx.provider_cooldown_secs > 0
        {
            self.ctx.circuit.trigger_cooldown(
                self.ctx.provider_id,
                now_unix,
                self.ctx.provider_cooldown_secs,
            );
        }
        if error_code.is_none() && (200..300).contains(&self.ctx.status) {
            let change = self
                .ctx
                .circuit
                .record_success(self.ctx.provider_id, now_unix);
            if let Some(t) = change.transition {
                emit_circuit_transition(
                    &self.ctx.app,
                    &self.ctx.trace_id,
                    &self.ctx.cli_key,
                    self.ctx.provider_id,
                    &self.ctx.provider_name,
                    &self.ctx.base_url,
                    &t,
                    now_unix,
                );
            }
            if let Some(session_id) = self.ctx.session_id.as_deref() {
                self.ctx.session.bind_success(
                    &self.ctx.cli_key,
                    session_id,
                    self.ctx.provider_id,
                    self.ctx.sort_mode_id,
                    now_unix,
                );
            }
        } else if effective_error_category == Some(ErrorCategory::ProviderError.as_str()) {
            let change = self
                .ctx
                .circuit
                .record_failure(self.ctx.provider_id, now_unix);
            if let Some(t) = change.transition {
                emit_circuit_transition(
                    &self.ctx.app,
                    &self.ctx.trace_id,
                    &self.ctx.cli_key,
                    self.ctx.provider_id,
                    &self.ctx.provider_name,
                    &self.ctx.base_url,
                    &t,
                    now_unix,
                );
            }
        }

        emit_request_event(
            &self.ctx.app,
            self.ctx.trace_id.clone(),
            self.ctx.cli_key.clone(),
            self.ctx.method.clone(),
            self.ctx.path.clone(),
            self.ctx.query.clone(),
            Some(self.ctx.status),
            effective_error_category,
            error_code,
            duration_ms,
            self.first_byte_ms,
            self.ctx.attempts.clone(),
            usage_metrics,
        );

        spawn_enqueue_request_log_with_backpressure(
            self.ctx.app.clone(),
            self.ctx.log_tx.clone(),
            RequestLogEnqueueArgs {
                trace_id: self.ctx.trace_id.clone(),
                cli_key: self.ctx.cli_key.clone(),
                session_id: self.ctx.session_id.clone(),
                method: self.ctx.method.clone(),
                path: self.ctx.path.clone(),
                query: self.ctx.query.clone(),
                excluded_from_stats: self.ctx.excluded_from_stats,
                special_settings_json: response_fixer::special_settings_json(
                    &self.ctx.special_settings,
                ),
                status: Some(self.ctx.status),
                error_code,
                duration_ms,
                ttfb_ms: self.first_byte_ms,
                attempts_json: self.ctx.attempts_json.clone(),
                requested_model,
                created_at_ms: self.ctx.created_at_ms,
                created_at: self.ctx.created_at,
                usage,
            },
        );
    }
}

impl<S, B> Stream for UsageBodyBufferTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    type Item = Result<B, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        if let Some(total) = this.total_timeout {
            if this.ctx.started.elapsed() >= total {
                this.finalize(Some("GW_UPSTREAM_TIMEOUT"));
                return Poll::Ready(None);
            }
        }

        let next = Pin::new(&mut this.upstream).poll_next(cx);

        match next {
            Poll::Pending => {
                if let Some(timer) = this.total_sleep.as_mut() {
                    if timer.as_mut().poll(cx).is_ready() {
                        this.finalize(Some("GW_UPSTREAM_TIMEOUT"));
                        return Poll::Ready(None);
                    }
                }
                Poll::Pending
            }
            Poll::Ready(None) => {
                this.finalize(this.ctx.error_code);
                Poll::Ready(None)
            }
            Poll::Ready(Some(Ok(chunk))) => {
                if this.first_byte_ms.is_none() {
                    this.first_byte_ms = Some(this.ctx.started.elapsed().as_millis());
                }
                if !this.truncated {
                    let bytes = chunk.as_ref();
                    if this.buffer.len().saturating_add(bytes.len()) <= this.max_bytes {
                        this.buffer.extend_from_slice(bytes);
                    } else {
                        this.truncated = true;
                        this.buffer.clear();
                    }
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.finalize(Some("GW_STREAM_ERROR"));
                Poll::Ready(Some(Err(err)))
            }
        }
    }
}

impl<S, B> Drop for UsageBodyBufferTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    fn drop(&mut self) {
        if !self.finalized {
            self.finalize(Some("GW_STREAM_ABORTED"));
        }
    }
}

pub(super) struct TimingOnlyTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    upstream: S,
    ctx: StreamFinalizeCtx,
    first_byte_ms: Option<u128>,
    total_timeout: Option<Duration>,
    total_sleep: Option<Pin<Box<tokio::time::Sleep>>>,
    finalized: bool,
}

impl<S, B> TimingOnlyTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    pub(super) fn new(
        upstream: S,
        ctx: StreamFinalizeCtx,
        total_timeout: Option<Duration>,
    ) -> Self {
        let remaining = total_timeout.and_then(|d| d.checked_sub(ctx.started.elapsed()));
        Self {
            upstream,
            ctx,
            first_byte_ms: None,
            total_timeout,
            total_sleep: remaining.map(|d| Box::pin(tokio::time::sleep(d))),
            finalized: false,
        }
    }

    fn finalize(&mut self, error_code: Option<&'static str>) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        let duration_ms = self.ctx.started.elapsed().as_millis();
        let effective_error_category = if error_code == Some("GW_STREAM_ABORTED") {
            Some(ErrorCategory::ClientAbort.as_str())
        } else {
            self.ctx.error_category
        };

        let now_unix = now_unix_seconds() as i64;
        if error_code.is_some()
            && effective_error_category != Some(ErrorCategory::ClientAbort.as_str())
            && self.ctx.provider_cooldown_secs > 0
        {
            self.ctx.circuit.trigger_cooldown(
                self.ctx.provider_id,
                now_unix,
                self.ctx.provider_cooldown_secs,
            );
        }
        if error_code.is_none() && (200..300).contains(&self.ctx.status) {
            let change = self
                .ctx
                .circuit
                .record_success(self.ctx.provider_id, now_unix);
            if let Some(t) = change.transition {
                emit_circuit_transition(
                    &self.ctx.app,
                    &self.ctx.trace_id,
                    &self.ctx.cli_key,
                    self.ctx.provider_id,
                    &self.ctx.provider_name,
                    &self.ctx.base_url,
                    &t,
                    now_unix,
                );
            }
            if let Some(session_id) = self.ctx.session_id.as_deref() {
                self.ctx.session.bind_success(
                    &self.ctx.cli_key,
                    session_id,
                    self.ctx.provider_id,
                    self.ctx.sort_mode_id,
                    now_unix,
                );
            }
        } else if effective_error_category == Some(ErrorCategory::ProviderError.as_str()) {
            let change = self
                .ctx
                .circuit
                .record_failure(self.ctx.provider_id, now_unix);
            if let Some(t) = change.transition {
                emit_circuit_transition(
                    &self.ctx.app,
                    &self.ctx.trace_id,
                    &self.ctx.cli_key,
                    self.ctx.provider_id,
                    &self.ctx.provider_name,
                    &self.ctx.base_url,
                    &t,
                    now_unix,
                );
            }
        }

        emit_request_event(
            &self.ctx.app,
            self.ctx.trace_id.clone(),
            self.ctx.cli_key.clone(),
            self.ctx.method.clone(),
            self.ctx.path.clone(),
            self.ctx.query.clone(),
            Some(self.ctx.status),
            effective_error_category,
            error_code,
            duration_ms,
            self.first_byte_ms,
            self.ctx.attempts.clone(),
            None,
        );

        spawn_enqueue_request_log_with_backpressure(
            self.ctx.app.clone(),
            self.ctx.log_tx.clone(),
            RequestLogEnqueueArgs {
                trace_id: self.ctx.trace_id.clone(),
                cli_key: self.ctx.cli_key.clone(),
                session_id: self.ctx.session_id.clone(),
                method: self.ctx.method.clone(),
                path: self.ctx.path.clone(),
                query: self.ctx.query.clone(),
                excluded_from_stats: self.ctx.excluded_from_stats,
                special_settings_json: response_fixer::special_settings_json(
                    &self.ctx.special_settings,
                ),
                status: Some(self.ctx.status),
                error_code,
                duration_ms,
                ttfb_ms: self.first_byte_ms,
                attempts_json: self.ctx.attempts_json.clone(),
                requested_model: self.ctx.requested_model.clone(),
                created_at_ms: self.ctx.created_at_ms,
                created_at: self.ctx.created_at,
                usage: None,
            },
        );
    }
}

impl<S, B> Stream for TimingOnlyTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    type Item = Result<B, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        if let Some(total) = this.total_timeout {
            if this.ctx.started.elapsed() >= total {
                this.finalize(Some("GW_UPSTREAM_TIMEOUT"));
                return Poll::Ready(None);
            }
        }

        let next = Pin::new(&mut this.upstream).poll_next(cx);

        match next {
            Poll::Pending => {
                if let Some(timer) = this.total_sleep.as_mut() {
                    if timer.as_mut().poll(cx).is_ready() {
                        this.finalize(Some("GW_UPSTREAM_TIMEOUT"));
                        return Poll::Ready(None);
                    }
                }
                Poll::Pending
            }
            Poll::Ready(None) => {
                this.finalize(this.ctx.error_code);
                Poll::Ready(None)
            }
            Poll::Ready(Some(Ok(chunk))) => {
                if this.first_byte_ms.is_none() {
                    this.first_byte_ms = Some(this.ctx.started.elapsed().as_millis());
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(err))) => {
                this.finalize(Some("GW_STREAM_ERROR"));
                Poll::Ready(Some(Err(err)))
            }
        }
    }
}

impl<S, B> Drop for TimingOnlyTeeStream<S, B>
where
    S: Stream<Item = Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    fn drop(&mut self) {
        if !self.finalized {
            self.finalize(Some("GW_STREAM_ABORTED"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct VecBytesStream {
        items: VecDeque<Result<Bytes, reqwest::Error>>,
    }

    impl VecBytesStream {
        fn new(items: Vec<Result<Bytes, reqwest::Error>>) -> Self {
            Self {
                items: items.into_iter().collect(),
            }
        }
    }

    impl Stream for VecBytesStream {
        type Item = Result<Bytes, reqwest::Error>;

        fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.items.pop_front())
        }
    }

    struct NextFuture<'a, S: Stream + Unpin>(&'a mut S);

    impl<'a, S: Stream + Unpin> Future for NextFuture<'a, S> {
        type Output = Option<S::Item>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            Pin::new(&mut *self.0).poll_next(cx)
        }
    }

    async fn next_item<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
        NextFuture(stream).await
    }

    async fn collect_ok_bytes<S>(mut stream: S) -> Vec<u8>
    where
        S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
    {
        let mut out: Vec<u8> = Vec::new();
        while let Some(item) = next_item(&mut stream).await {
            let bytes = item.expect("stream should not error in test");
            out.extend_from_slice(bytes.as_ref());
        }
        out
    }

    fn gzip_bytes(input: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(input).expect("gzip write");
        encoder.finish().expect("gzip finish")
    }

    #[tokio::test]
    async fn gunzip_stream_decompresses_gzip_body() {
        let original = b"hello\nworld\n";
        let gz = gzip_bytes(original);

        let mid = gz.len() / 2;
        let upstream = VecBytesStream::new(vec![
            Ok(Bytes::copy_from_slice(&gz[..mid])),
            Ok(Bytes::copy_from_slice(&gz[mid..])),
        ]);

        let out = collect_ok_bytes(GunzipStream::new(upstream)).await;
        assert_eq!(out, original);
    }

    #[tokio::test]
    async fn gunzip_stream_ignores_truncated_gzip_and_returns_partial_output() {
        let original = b"{\"ok\":true}\n";
        let mut gz = gzip_bytes(original);
        // gzip footer is 8 bytes (CRC32 + ISIZE). Truncating it should trigger an error, but the
        // decompressor should still output the full payload in most cases.
        if gz.len() > 8 {
            gz.truncate(gz.len() - 8);
        }

        let upstream = VecBytesStream::new(vec![Ok(Bytes::from(gz))]);
        let out = collect_ok_bytes(GunzipStream::new(upstream)).await;
        assert_eq!(out, original);
    }
}
