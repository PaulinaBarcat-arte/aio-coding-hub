//! Usage: `Stream` adaptor that gunzips an upstream `bytes_stream()`.

use axum::body::Bytes;
use flate2::write::GzDecoder;
use futures_core::Stream;
use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};

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

pub(in crate::gateway) struct GunzipStream<S>
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
    pub(in crate::gateway) fn new(upstream: S) -> Self {
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

#[cfg(test)]
mod tests;
