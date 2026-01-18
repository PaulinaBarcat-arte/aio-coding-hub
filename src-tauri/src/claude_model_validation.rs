use crate::{blocking, claude_model_validation_history, db, usage};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashSet;
use std::time::{Duration, Instant};

const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_EXCERPT_BYTES: usize = 16 * 1024;
const MAX_TEXT_PREVIEW_CHARS: usize = 4000;
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize)]
pub struct ClaudeModelValidationResult {
    pub ok: bool,
    pub provider_id: i64,
    pub provider_name: String,
    pub base_url: String,
    pub target_url: String,
    pub status: Option<u16>,
    pub duration_ms: i64,
    pub requested_model: Option<String>,
    pub responded_model: Option<String>,
    pub stream: bool,
    pub output_text_chars: i64,
    pub output_text_preview: String,
    pub checks: serde_json::Value,
    pub signals: serde_json::Value,
    pub response_headers: serde_json::Value,
    pub usage: Option<serde_json::Value>,
    pub error: Option<String>,
    pub raw_excerpt: String,
    pub request: serde_json::Value,
}

#[derive(Debug, Clone)]
struct ProviderForValidation {
    id: i64,
    cli_key: String,
    name: String,
    base_urls: Vec<String>,
    api_key_plaintext: String,
}

#[derive(Debug, Clone)]
struct ParsedRequest {
    request_value: serde_json::Value,
    headers: serde_json::Map<String, serde_json::Value>,
    body: serde_json::Value,
    expect_max_output_chars: Option<usize>,
    expect_exact_output_chars: Option<usize>,
    forwarded_path: String,
    forwarded_query: Option<String>,
}

fn base_urls_from_row(base_url_fallback: &str, base_urls_json: &str) -> Vec<String> {
    let mut parsed: Vec<String> = serde_json::from_str::<Vec<String>>(base_urls_json)
        .ok()
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();

    let mut seen: HashSet<String> = HashSet::with_capacity(parsed.len());
    parsed.retain(|v| seen.insert(v.clone()));

    if parsed.is_empty() {
        let fallback = base_url_fallback.trim();
        if fallback.is_empty() {
            return vec![String::new()];
        }
        return vec![fallback.to_string()];
    }

    parsed
}

fn mask_header_value(name: &str, value: &str) -> serde_json::Value {
    let name_lc = name.trim().to_lowercase();
    if name_lc == "x-api-key" || name_lc == "authorization" {
        return serde_json::Value::String("***".to_string());
    }
    serde_json::Value::String(value.to_string())
}

fn mask_response_header_value(name: &str, value: &str) -> serde_json::Value {
    let name_lc = name.trim().to_lowercase();
    if name_lc == "set-cookie"
        || name_lc == "cookie"
        || name_lc == "authorization"
        || name_lc == "proxy-authorization"
        || name_lc == "x-api-key"
    {
        return serde_json::Value::String("***".to_string());
    }
    serde_json::Value::String(value.to_string())
}

fn json_map_push_value(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: serde_json::Value,
) {
    match map.get_mut(key) {
        None => {
            map.insert(key.to_string(), value);
        }
        Some(existing) => {
            if let Some(arr) = existing.as_array_mut() {
                arr.push(value);
            } else {
                let prev = existing.take();
                *existing = serde_json::Value::Array(vec![prev, value]);
            }
        }
    }
}

fn response_headers_to_json(headers: &HeaderMap) -> serde_json::Value {
    let mut out = serde_json::Map::<String, serde_json::Value>::new();
    for (name, value) in headers.iter() {
        let name_str = name.as_str();
        if name_str.trim().is_empty() {
            continue;
        }
        let Ok(value_str) = value.to_str() else {
            continue;
        };
        json_map_push_value(
            &mut out,
            name_str,
            mask_response_header_value(name_str, value_str),
        );
    }
    serde_json::Value::Object(out)
}

fn parse_request_json(request_json: &str) -> Result<ParsedRequest, String> {
    let value: serde_json::Value = serde_json::from_str(request_json)
        .map_err(|e| format!("SEC_INVALID_INPUT: invalid JSON: {e}"))?;

    let Some(obj) = value.as_object() else {
        return Err("SEC_INVALID_INPUT: request_json must be a JSON object".to_string());
    };

    // Accept either:
    // - Wrapper: { headers, body, expect }
    // - Raw body: { model, messages, ... }
    let (headers_value, body_value, expect_value, path_value, query_value) =
        if obj.contains_key("body") {
            (
                obj.get("headers").cloned(),
                obj.get("body").cloned(),
                obj.get("expect").cloned(),
                obj.get("path").cloned(),
                obj.get("query").cloned(),
            )
        } else {
            (None, Some(value.clone()), None, None, None)
        };

    let headers_map = headers_value
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let Some(body) = body_value else {
        return Err("SEC_INVALID_INPUT: request_json.body is required".to_string());
    };
    if !body.is_object() {
        return Err("SEC_INVALID_INPUT: request_json.body must be an object".to_string());
    }

    let expect_max_output_chars = expect_value
        .as_ref()
        .and_then(|v| v.as_object())
        .and_then(|m| m.get("max_output_chars"))
        .and_then(|v| v.as_u64())
        .and_then(|v| usize::try_from(v).ok())
        .filter(|v| *v > 0);

    let expect_exact_output_chars = expect_value
        .as_ref()
        .and_then(|v| v.as_object())
        .and_then(|m| m.get("exact_output_chars"))
        .and_then(|v| v.as_u64())
        .and_then(|v| usize::try_from(v).ok())
        .filter(|v| *v > 0);

    let (forwarded_path, forwarded_query_from_path) = path_value
        .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
        .and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }

            let (path_part, query_part) = match trimmed.split_once('?') {
                Some((p, q)) => (p, Some(q)),
                None => (trimmed, None),
            };

            let mut path = path_part.trim().to_string();
            if path.is_empty() {
                return None;
            }
            if !path.starts_with('/') {
                path.insert(0, '/');
            }

            let query = query_part
                .map(|q| q.trim().trim_start_matches('?').to_string())
                .filter(|q| !q.is_empty());

            Some((path, query))
        })
        .unwrap_or_else(|| ("/v1/messages".to_string(), None));

    let forwarded_query = query_value
        .and_then(|v| {
            v.as_str()
                .map(|s| s.trim().trim_start_matches('?').to_string())
        })
        .filter(|s| !s.is_empty())
        .or(forwarded_query_from_path);

    Ok(ParsedRequest {
        request_value: value,
        headers: headers_map,
        body,
        expect_max_output_chars,
        expect_exact_output_chars,
        forwarded_path,
        forwarded_query,
    })
}

fn build_target_url(
    base_url: &str,
    forwarded_path: &str,
    forwarded_query: Option<&str>,
) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|e| format!("SEC_INVALID_INPUT: invalid base_url: {e}"))?;

    let base_path = url.path().trim_end_matches('/');
    let forwarded_path = if base_path.ends_with("/v1")
        && (forwarded_path == "/v1" || forwarded_path.starts_with("/v1/"))
    {
        forwarded_path.strip_prefix("/v1").unwrap_or(forwarded_path)
    } else {
        forwarded_path
    };

    let mut combined_path = String::new();
    combined_path.push_str(base_path);
    combined_path.push_str(forwarded_path);

    if combined_path.is_empty() {
        combined_path.push('/');
    }
    if !combined_path.starts_with('/') {
        combined_path.insert(0, '/');
    }

    url.set_path(&combined_path);
    let forwarded_query = forwarded_query
        .map(str::trim)
        .map(|v| v.trim_start_matches('?'))
        .filter(|v| !v.is_empty());
    url.set_query(forwarded_query);
    Ok(url)
}

fn header_map_from_json(
    headers_json: &serde_json::Map<String, serde_json::Value>,
    provider_api_key: &str,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let wants_authorization = headers_json
        .keys()
        .any(|k| k.trim().eq_ignore_ascii_case("authorization"));

    for (k, v) in headers_json {
        let Some(value_str) = v.as_str() else {
            continue;
        };
        let name_lc = k.trim().to_lowercase();
        if name_lc.is_empty() {
            continue;
        }
        if name_lc == "x-api-key" || name_lc == "authorization" || name_lc == "host" {
            // Never accept caller-provided auth.
            continue;
        }

        let Ok(name) = HeaderName::from_bytes(name_lc.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(value_str) else {
            continue;
        };
        headers.insert(name, value);
    }

    headers.insert(
        HeaderName::from_static("x-api-key"),
        HeaderValue::from_str(provider_api_key).unwrap_or_else(|_| HeaderValue::from_static("")),
    );

    if wants_authorization {
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&format!("Bearer {provider_api_key}"))
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
    }

    if !headers.contains_key("anthropic-version") {
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static(DEFAULT_ANTHROPIC_VERSION),
        );
    }

    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/json"),
    );

    headers
}

fn signals_from_text(text: &str) -> serde_json::Value {
    let lower = text.to_lowercase();
    let mentions_bedrock = lower.contains("amazon-bedrock")
        || lower.contains("bedrock-")
        || lower.contains("model group=bedrock");

    let mentions_max_tokens = lower.contains("max_tokens");
    let mentions_tokens_greater = lower.contains("must be greater") && lower.contains("_tokens");

    serde_json::json!({
        "mentions_amazon_bedrock": mentions_bedrock,
        "mentions_max_tokens": mentions_max_tokens,
        "mentions_max_tokens_must_be_greater_than_tokens": mentions_tokens_greater,
    })
}

fn has_cache_creation_detail(usage: Option<&serde_json::Value>) -> bool {
    let Some(obj) = usage.and_then(|v| v.as_object()) else {
        return false;
    };
    obj.contains_key("cache_creation_5m_input_tokens")
        || obj.contains_key("cache_creation_1h_input_tokens")
}

fn take_first_n_chars(s: &str, n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    s.chars().take(n).collect()
}

#[derive(Default)]
struct SseTextAccumulator {
    buffer: Vec<u8>,
    current_event: Vec<u8>,
    current_data: Vec<u8>,
    total_chars: usize,
    preview: String,
    thinking_chars: usize,
    thinking_preview: String,
    thinking_block_seen: bool,
    signature_chars: usize,
    message_delta_seen: bool,
    message_delta_stop_reason: Option<String>,
    message_delta_stop_reason_is_max_tokens: bool,
    response_id: String,
    service_tier: String,
}

impl SseTextAccumulator {
    fn ingest_chunk(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);

        let buf = std::mem::take(&mut self.buffer);
        let mut start = 0usize;
        for (idx, b) in buf.iter().enumerate() {
            if *b != b'\n' {
                continue;
            }

            let mut line = &buf[start..idx];
            if line.last() == Some(&b'\r') {
                line = &line[..line.len().saturating_sub(1)];
            }
            self.ingest_line(line);
            start = idx + 1;
        }

        if start < buf.len() {
            self.buffer.extend_from_slice(&buf[start..]);
        }
    }

    fn finalize(&mut self) {
        if !self.buffer.is_empty() {
            let mut tail = std::mem::take(&mut self.buffer);
            if tail.last() == Some(&b'\r') {
                tail.pop();
            }
            self.ingest_line(&tail);
        }
        self.flush_event();
    }

    fn ingest_line(&mut self, line: &[u8]) {
        if line.is_empty() {
            self.flush_event();
            return;
        }
        if line[0] == b':' {
            return;
        }
        if let Some(rest) = line.strip_prefix(b"event:") {
            let rest = trim_ascii(rest);
            self.current_event.clear();
            self.current_event.extend_from_slice(rest);
            return;
        }
        if let Some(rest) = line.strip_prefix(b"data:") {
            let mut rest = rest;
            if rest.first() == Some(&b' ') {
                rest = &rest[1..];
            }
            if rest == b"[DONE]" {
                return;
            }
            if !self.current_data.is_empty() {
                self.current_data.push(b'\n');
            }
            self.current_data.extend_from_slice(rest);
        }
    }

    fn flush_event(&mut self) {
        if self.current_data.is_empty() {
            self.current_event.clear();
            return;
        }

        let event_name = if self.current_event.is_empty() {
            b"message".to_vec()
        } else {
            self.current_event.clone()
        };
        let data_json: serde_json::Value = match serde_json::from_slice(&self.current_data) {
            Ok(v) => v,
            Err(_) => {
                self.current_event.clear();
                self.current_data.clear();
                return;
            }
        };

        self.ingest_event(&event_name, &data_json);
        self.current_event.clear();
        self.current_data.clear();
    }

    fn append_text(&mut self, text: &str) {
        self.total_chars = self.total_chars.saturating_add(text.chars().count());
        if self.preview.chars().count() >= MAX_TEXT_PREVIEW_CHARS {
            return;
        }
        let remaining = MAX_TEXT_PREVIEW_CHARS.saturating_sub(self.preview.chars().count());
        self.preview.push_str(&take_first_n_chars(text, remaining));
    }

    fn append_thinking(&mut self, text: &str) {
        self.thinking_block_seen = true;
        self.thinking_chars = self.thinking_chars.saturating_add(text.chars().count());
        if self.thinking_preview.chars().count() >= MAX_TEXT_PREVIEW_CHARS {
            return;
        }
        let remaining =
            MAX_TEXT_PREVIEW_CHARS.saturating_sub(self.thinking_preview.chars().count());
        self.thinking_preview
            .push_str(&take_first_n_chars(text, remaining));
    }

    fn ingest_signature(&mut self, signature: &str) {
        let trimmed = signature.trim();
        if trimmed.is_empty() {
            return;
        }
        self.thinking_block_seen = true;
        let chars = trimmed.chars().count();
        if chars > self.signature_chars {
            self.signature_chars = chars;
        }
    }

    fn ingest_response_meta(&mut self, value: &serde_json::Value) {
        let (id, service_tier) = extract_response_meta_from_message_json(value);
        if self.response_id.is_empty() {
            if let Some(v) = id {
                self.response_id = v;
            }
        }
        if self.service_tier.is_empty() {
            if let Some(v) = service_tier {
                self.service_tier = v;
            }
        }
    }

    fn ingest_event(&mut self, event: &[u8], data: &serde_json::Value) {
        let event_name = std::str::from_utf8(event).unwrap_or("").trim();
        let data_type = data
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if event_name == "message_delta" || data_type == "message_delta" {
            self.message_delta_seen = true;
            if let Some(stop_reason) = data
                .get("delta")
                .and_then(|v| v.get("stop_reason"))
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
            {
                let is_max_tokens = stop_reason == "max_tokens";
                self.message_delta_stop_reason_is_max_tokens =
                    self.message_delta_stop_reason_is_max_tokens || is_max_tokens;
                if is_max_tokens || self.message_delta_stop_reason.is_none() {
                    self.message_delta_stop_reason = Some(stop_reason);
                }
            }
        }

        // 先尽可能从 message 或根对象里提取结构字段（不会影响 text/thinking 的计数口径）。
        if let Some(message) = data.get("message") {
            self.ingest_response_meta(message);
        }
        self.ingest_response_meta(data);

        // Prefer deltas: { delta: { type: "text_delta", text: "..." } }
        if let Some(delta) = data.get("delta").and_then(|v| v.as_object()) {
            let delta_type = delta
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if delta_type == "text_delta" || delta_type == "text" {
                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                    self.append_text(text);
                    return;
                }
            }

            // thinking_delta: { delta: { type:"thinking_delta", thinking:"..." } }
            if delta_type == "thinking_delta" || delta_type == "thinking" {
                if let Some(text) = delta.get("thinking").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        self.append_thinking(text);
                        return;
                    }
                }
                // Best-effort fallback: some variants might use `text` for thinking.
                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        self.append_thinking(text);
                        return;
                    }
                }
            }

            // Best-effort: signature might appear on delta/message shapes.
            if let Some(signature) = delta.get("signature").and_then(|v| v.as_str()) {
                self.ingest_signature(signature);
            }
            if let Some(thinking) = delta.get("thinking").and_then(|v| v.as_str()) {
                if !thinking.is_empty() {
                    self.append_thinking(thinking);
                    return;
                }
            }

            // Best-effort fallback: some variants might include "text" without explicit type.
            if delta_type.is_empty() {
                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                    self.append_text(text);
                    return;
                }
            }

            // Best-effort fallback: some proxies might embed message content in delta.content.
            if let Some(content) = delta.get("content") {
                if let Some(text) = content.as_str() {
                    self.append_text(text);
                    return;
                }
                if let Some(arr) = content.as_array() {
                    for block in arr {
                        let Some(obj) = block.as_object() else {
                            continue;
                        };
                        let block_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        if block_type == "text" {
                            let Some(text) = obj.get("text").and_then(|v| v.as_str()) else {
                                continue;
                            };
                            if !text.is_empty() {
                                self.append_text(text);
                            }
                            continue;
                        }
                        if block_type == "thinking" || block_type == "redacted_thinking" {
                            self.thinking_block_seen = true;
                            if let Some(thinking) = obj
                                .get("thinking")
                                .and_then(|v| v.as_str())
                                .or_else(|| obj.get("text").and_then(|v| v.as_str()))
                            {
                                if !thinking.is_empty() {
                                    self.append_thinking(thinking);
                                }
                            }
                            if let Some(signature) = obj.get("signature").and_then(|v| v.as_str()) {
                                self.ingest_signature(signature);
                            }
                        }
                    }
                    if self.total_chars > 0 {
                        return;
                    }
                }
            }
        }

        // content_block_start: { content_block: { type:"text", text:"..." } }
        if let Some(block) = data.get("content_block").and_then(|v| v.as_object()) {
            if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        self.append_text(text);
                        return;
                    }
                }
            }
            if let Some(block_type) = block.get("type").and_then(|v| v.as_str()) {
                if block_type == "thinking" || block_type == "redacted_thinking" {
                    self.thinking_block_seen = true;
                    if let Some(thinking) = block
                        .get("thinking")
                        .and_then(|v| v.as_str())
                        .or_else(|| block.get("text").and_then(|v| v.as_str()))
                    {
                        if !thinking.is_empty() {
                            self.append_thinking(thinking);
                        }
                    }
                    if let Some(signature) = block.get("signature").and_then(|v| v.as_str()) {
                        self.ingest_signature(signature);
                    }
                }
            }
        }

        // Last resort: if no delta was captured, try to extract from a full message/content shape
        // (some proxies may not preserve Anthropic SSE event structure).
        if self.total_chars == 0 {
            if let Some(message) = data.get("message") {
                let (chars, preview) = extract_text_from_message_json(message);
                if chars > 0 {
                    self.total_chars = chars;
                    self.preview = preview;
                    return;
                }
            }

            let (chars, preview) = extract_text_from_message_json(data);
            if chars > 0 {
                self.total_chars = chars;
                self.preview = preview;
            }
        }

        // Thinking/signature：同样做 best-effort 兜底提取（只在尚未拿到时尝试，避免重复计数）。
        if !self.thinking_block_seen && self.thinking_chars == 0 && self.signature_chars == 0 {
            if let Some(message) = data.get("message") {
                let (has_block, chars, preview, signature_chars) =
                    extract_thinking_from_message_json(message);
                if has_block {
                    self.thinking_block_seen = true;
                }
                if chars > 0 {
                    self.thinking_chars = chars;
                    self.thinking_preview = preview;
                }
                if signature_chars > self.signature_chars {
                    self.signature_chars = signature_chars;
                }
                if self.thinking_block_seen || self.thinking_chars > 0 || self.signature_chars > 0 {
                    return;
                }
            }

            let (has_block, chars, preview, signature_chars) =
                extract_thinking_from_message_json(data);
            if has_block {
                self.thinking_block_seen = true;
            }
            if chars > 0 {
                self.thinking_chars = chars;
                self.thinking_preview = preview;
            }
            if signature_chars > self.signature_chars {
                self.signature_chars = signature_chars;
            }
        }
    }
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();

    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    &bytes[start..end]
}

fn extract_text_from_message_json(value: &serde_json::Value) -> (usize, String) {
    let mut total = 0usize;
    let mut preview = String::new();

    let Some(content) = value.get("content") else {
        return (0, String::new());
    };

    if let Some(s) = content.as_str() {
        total = s.chars().count();
        preview = take_first_n_chars(s, MAX_TEXT_PREVIEW_CHARS);
        return (total, preview);
    }

    let Some(arr) = content.as_array() else {
        return (0, String::new());
    };

    for block in arr {
        let Some(obj) = block.as_object() else {
            continue;
        };
        if obj.get("type").and_then(|v| v.as_str()) != Some("text") {
            continue;
        }
        let Some(text) = obj.get("text").and_then(|v| v.as_str()) else {
            continue;
        };
        total = total.saturating_add(text.chars().count());
        if preview.chars().count() < MAX_TEXT_PREVIEW_CHARS {
            let remaining = MAX_TEXT_PREVIEW_CHARS.saturating_sub(preview.chars().count());
            preview.push_str(&take_first_n_chars(text, remaining));
        }
    }

    (total, preview)
}

fn extract_response_meta_from_message_json(
    value: &serde_json::Value,
) -> (Option<String>, Option<String>) {
    // Support both:
    // - message JSON: { id, service_tier, usage, content, ... }
    // - SSE wrapper: { message: { ... } }
    if let Some(message) = value.get("message") {
        return extract_response_meta_from_message_json(message);
    }

    let Some(obj) = value.as_object() else {
        return (None, None);
    };

    let id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Prefer top-level service_tier; best-effort fallback to usage.service_tier.
    let service_tier = obj
        .get("service_tier")
        .and_then(|v| v.as_str())
        .or_else(|| {
            obj.get("usage")
                .and_then(|u| u.get("service_tier"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    (id, service_tier)
}

fn extract_thinking_from_message_json(value: &serde_json::Value) -> (bool, usize, String, usize) {
    // Returns:
    // - has_thinking_block: whether "thinking"/"redacted_thinking" blocks were observed
    // - thinking_chars: accumulated char count of thinking text (best-effort)
    // - thinking_preview: first N chars of thinking (best-effort)
    // - signature_chars: max signature length found (best-effort)

    // Support SSE wrapper: { message: { ... } }
    if let Some(message) = value.get("message") {
        return extract_thinking_from_message_json(message);
    }

    let mut has_block = false;
    let mut total = 0usize;
    let mut preview = String::new();
    let mut signature_chars = 0usize;

    // Some variants may flatten thinking to a top-level field.
    if let Some(t) = value.get("thinking").and_then(|v| v.as_str()) {
        let text = t.trim();
        if !text.is_empty() {
            has_block = true;
            total = total.saturating_add(text.chars().count());
            if preview.chars().count() < MAX_TEXT_PREVIEW_CHARS {
                let remaining = MAX_TEXT_PREVIEW_CHARS.saturating_sub(preview.chars().count());
                preview.push_str(&take_first_n_chars(text, remaining));
            }
        }
    }

    let Some(content) = value.get("content") else {
        return (has_block, total, preview, signature_chars);
    };

    let Some(arr) = content.as_array() else {
        return (has_block, total, preview, signature_chars);
    };

    for block in arr {
        let Some(obj) = block.as_object() else {
            continue;
        };
        let block_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if block_type == "thinking" || block_type == "redacted_thinking" {
            has_block = true;

            if let Some(sig) = obj.get("signature").and_then(|v| v.as_str()) {
                let trimmed = sig.trim();
                if !trimmed.is_empty() {
                    signature_chars = signature_chars.max(trimmed.chars().count());
                }
            }

            // `thinking` blocks usually provide the text under `thinking`.
            // Some proxies might store it under `text`; treat that as best-effort.
            if let Some(t) = obj
                .get("thinking")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("text").and_then(|v| v.as_str()))
            {
                let text = t.trim();
                if !text.is_empty() {
                    total = total.saturating_add(text.chars().count());
                    if preview.chars().count() < MAX_TEXT_PREVIEW_CHARS {
                        let remaining =
                            MAX_TEXT_PREVIEW_CHARS.saturating_sub(preview.chars().count());
                        preview.push_str(&take_first_n_chars(text, remaining));
                    }
                }
            }
        }
    }

    (has_block, total, preview, signature_chars)
}

async fn load_provider(
    app: tauri::AppHandle,
    provider_id: i64,
) -> Result<ProviderForValidation, String> {
    blocking::run("claude_provider_validate_model_load_provider", move || {
        if provider_id <= 0 {
            return Err(format!(
                "SEC_INVALID_INPUT: invalid provider_id={provider_id}"
            ));
        }

        let conn = db::open_connection(&app)?;
        let row: Option<(i64, String, String, String, String, String)> = conn
            .query_row(
                r#"
SELECT
  id,
  cli_key,
  name,
  base_url,
  base_urls_json,
  api_key_plaintext
FROM providers
WHERE id = ?1
"#,
                params![provider_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("DB_ERROR: failed to query provider: {e}"))?;

        let Some((id, cli_key, name, base_url_fallback, base_urls_json, api_key_plaintext)) = row
        else {
            return Err("DB_NOT_FOUND: provider not found".to_string());
        };

        let base_urls = base_urls_from_row(&base_url_fallback, &base_urls_json);

        Ok(ProviderForValidation {
            id,
            cli_key,
            name,
            base_urls,
            api_key_plaintext,
        })
    })
    .await
}

pub async fn validate_provider_model(
    app: &tauri::AppHandle,
    provider_id: i64,
    base_url: &str,
    request_json: &str,
) -> Result<ClaudeModelValidationResult, String> {
    let started = Instant::now();

    let provider = load_provider(app.clone(), provider_id).await?;
    if provider.cli_key != "claude" {
        return Err("SEC_INVALID_INPUT: only cli_key=claude is supported".to_string());
    }

    let base_url = base_url.trim();
    if base_url.is_empty() {
        return Err("SEC_INVALID_INPUT: base_url is required".to_string());
    }

    if !provider.base_urls.iter().any(|u| u == base_url) {
        return Err("SEC_INVALID_INPUT: base_url must be one of provider.base_urls".to_string());
    }

    let parsed = parse_request_json(request_json)?;

    let requested_model = parsed
        .body
        .get("model")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let stream = parsed
        .body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let target_url = build_target_url(
        base_url,
        &parsed.forwarded_path,
        parsed.forwarded_query.as_deref(),
    )?;

    let mut sanitized_request = parsed.request_value.clone();
    if let Some(obj) = sanitized_request.as_object_mut() {
        if let Some(headers) = obj.get_mut("headers").and_then(|v| v.as_object_mut()) {
            let mut next = serde_json::Map::new();
            for (k, v) in headers.iter() {
                if let Some(s) = v.as_str() {
                    next.insert(k.clone(), mask_header_value(k, s));
                }
            }
            // Ensure x-api-key is always masked (even if user did not include it).
            next.insert(
                "x-api-key".to_string(),
                serde_json::Value::String("***".to_string()),
            );
            *headers = next;
        }
    }

    let headers = header_map_from_json(&parsed.headers, &provider.api_key_plaintext);
    let body_bytes = serde_json::to_vec(&parsed.body)
        .map_err(|e| format!("SYSTEM_ERROR: failed to encode body JSON: {e}"))?;

    let client = reqwest::Client::builder()
        .user_agent(format!(
            "aio-coding-hub-validate/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP_CLIENT_INIT: {e}"))?;

    let mut raw_excerpt = Vec::<u8>::new();

    let mut err_out: Option<String> = None;

    let send_result = client
        .post(target_url.clone())
        .headers(headers)
        .body(body_bytes)
        .send()
        .await;

    let resp = match send_result {
        Ok(v) => Some(v),
        Err(e) => {
            err_out = Some(format!("HTTP_ERROR: {e}"));
            None
        }
    };

    if resp.is_none() {
        let result = ClaudeModelValidationResult {
            ok: false,
            provider_id: provider.id,
            provider_name: provider.name,
            base_url: base_url.to_string(),
            target_url: target_url.to_string(),
            status: None,
            duration_ms: started.elapsed().as_millis().min(i64::MAX as u128) as i64,
            requested_model,
            responded_model: None,
            stream,
            output_text_chars: 0,
            output_text_preview: String::new(),
            checks: serde_json::json!({}),
            signals: serde_json::json!({}),
            response_headers: serde_json::json!({}),
            usage: None,
            error: err_out,
            raw_excerpt: String::new(),
            request: sanitized_request,
        };

        return Ok(result);
    }

    let mut resp = resp.unwrap();
    let response_headers = response_headers_to_json(resp.headers());

    let status = resp.status().as_u16();
    let mut total_read = 0usize;

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let content_type_lc = content_type.to_lowercase();
    let is_sse_by_header = content_type_lc.contains("text/event-stream");

    let mut stream_read_error: Option<String> = None;
    let mut response_parse_mode = if is_sse_by_header { "sse" } else { "json" };

    let (
        responded_model,
        usage_json_value,
        output_text_chars,
        output_text_preview,
        thinking_block_seen,
        thinking_chars,
        thinking_preview,
        signature_chars,
        sse_message_delta_seen,
        sse_message_delta_stop_reason,
        sse_message_delta_stop_reason_is_max_tokens,
        response_id,
        service_tier,
    ) = if is_sse_by_header {
        let mut usage_tracker = usage::SseUsageTracker::new("claude");
        let mut text_tracker = SseTextAccumulator::default();

        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    total_read = total_read.saturating_add(chunk.len());

                    if raw_excerpt.len() < MAX_EXCERPT_BYTES {
                        let remaining = MAX_EXCERPT_BYTES.saturating_sub(raw_excerpt.len());
                        raw_excerpt.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                    }

                    usage_tracker.ingest_chunk(chunk.as_ref());
                    text_tracker.ingest_chunk(chunk.as_ref());

                    if total_read >= MAX_RESPONSE_BYTES {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    stream_read_error = Some(format!("STREAM_READ_ERROR: {e}"));
                    break;
                }
            }
        }

        text_tracker.finalize();
        let usage_extract = usage_tracker.finalize();
        let responded_model = usage_tracker.best_effort_model();
        let usage_json_value = usage_extract
            .as_ref()
            .and_then(|u| serde_json::from_str::<serde_json::Value>(&u.usage_json).ok());

        (
            responded_model,
            usage_json_value,
            text_tracker.total_chars,
            text_tracker.preview,
            text_tracker.thinking_block_seen,
            text_tracker.thinking_chars,
            text_tracker.thinking_preview,
            text_tracker.signature_chars,
            text_tracker.message_delta_seen,
            text_tracker.message_delta_stop_reason.clone(),
            text_tracker.message_delta_stop_reason_is_max_tokens,
            if text_tracker.response_id.trim().is_empty() {
                None
            } else {
                Some(text_tracker.response_id)
            },
            if text_tracker.service_tier.trim().is_empty() {
                None
            } else {
                Some(text_tracker.service_tier)
            },
        )
    } else {
        // Non-SSE by header: read up to MAX_RESPONSE_BYTES and parse as JSON; if parse fails and
        // caller requested stream=true, fall back to best-effort SSE parsing.
        let mut buf = Vec::<u8>::new();
        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    total_read = total_read.saturating_add(chunk.len());

                    if buf.len() < MAX_RESPONSE_BYTES {
                        let remaining = MAX_RESPONSE_BYTES.saturating_sub(buf.len());
                        buf.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                    }
                    if raw_excerpt.len() < MAX_EXCERPT_BYTES {
                        let remaining = MAX_EXCERPT_BYTES.saturating_sub(raw_excerpt.len());
                        raw_excerpt.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                    }

                    if total_read >= MAX_RESPONSE_BYTES {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    stream_read_error = Some(format!("STREAM_READ_ERROR: {e}"));
                    break;
                }
            }
        }

        let responded_model = usage::parse_model_from_json_bytes(&buf);
        let usage_json_value = usage::parse_usage_from_json_bytes(&buf)
            .and_then(|u| serde_json::from_str::<serde_json::Value>(&u.usage_json).ok());

        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&buf) {
            let (chars, preview) = extract_text_from_message_json(&value);
            let (thinking_block, thinking_chars, thinking_preview, signature_chars) =
                extract_thinking_from_message_json(&value);
            let (resp_id, service_tier) = extract_response_meta_from_message_json(&value);
            (
                responded_model,
                usage_json_value,
                chars,
                preview,
                thinking_block,
                thinking_chars,
                thinking_preview,
                signature_chars,
                false,
                None,
                false,
                resp_id,
                service_tier,
            )
        } else if stream {
            response_parse_mode = "sse_fallback";
            let mut usage_tracker = usage::SseUsageTracker::new("claude");
            let mut text_tracker = SseTextAccumulator::default();
            usage_tracker.ingest_chunk(&buf);
            text_tracker.ingest_chunk(&buf);
            text_tracker.finalize();
            let usage_extract = usage_tracker.finalize();
            let responded_model = usage_tracker.best_effort_model().or(responded_model);
            let usage_json_value = usage_extract
                .as_ref()
                .and_then(|u| serde_json::from_str::<serde_json::Value>(&u.usage_json).ok())
                .or(usage_json_value);
            (
                responded_model,
                usage_json_value,
                text_tracker.total_chars,
                text_tracker.preview,
                text_tracker.thinking_block_seen,
                text_tracker.thinking_chars,
                text_tracker.thinking_preview,
                text_tracker.signature_chars,
                text_tracker.message_delta_seen,
                text_tracker.message_delta_stop_reason.clone(),
                text_tracker.message_delta_stop_reason_is_max_tokens,
                if text_tracker.response_id.trim().is_empty() {
                    None
                } else {
                    Some(text_tracker.response_id)
                },
                if text_tracker.service_tier.trim().is_empty() {
                    None
                } else {
                    Some(text_tracker.service_tier)
                },
            )
        } else {
            (
                responded_model,
                usage_json_value,
                0usize,
                String::new(),
                false,
                0usize,
                String::new(),
                0usize,
                false,
                None,
                false,
                None,
                None,
            )
        }
    };

    let raw_excerpt_text = String::from_utf8_lossy(&raw_excerpt).to_string();
    let mut signals = signals_from_text(&raw_excerpt_text);
    if let Some(obj) = signals.as_object_mut() {
        obj.insert(
            "has_cache_creation_detail".to_string(),
            serde_json::Value::Bool(has_cache_creation_detail(usage_json_value.as_ref())),
        );
        obj.insert(
            "thinking_block_seen".to_string(),
            serde_json::Value::Bool(thinking_block_seen),
        );
        obj.insert(
            "thinking_chars".to_string(),
            serde_json::Value::Number((thinking_chars as i64).into()),
        );
        if !thinking_preview.trim().is_empty() {
            obj.insert(
                "thinking_preview".to_string(),
                serde_json::Value::String(thinking_preview.clone()),
            );
        }
        obj.insert(
            "signature_chars".to_string(),
            serde_json::Value::Number((signature_chars as i64).into()),
        );
        if let Some(v) = response_id.as_ref() {
            obj.insert(
                "response_id".to_string(),
                serde_json::Value::String(v.clone()),
            );
        }
        if let Some(v) = service_tier.as_ref() {
            obj.insert(
                "service_tier".to_string(),
                serde_json::Value::String(v.clone()),
            );
        }
        obj.insert(
            "response_bytes_truncated".to_string(),
            serde_json::Value::Bool(total_read >= MAX_RESPONSE_BYTES),
        );
        obj.insert(
            "response_content_type".to_string(),
            serde_json::Value::String(content_type),
        );
        obj.insert(
            "response_parse_mode".to_string(),
            serde_json::Value::String(response_parse_mode.to_string()),
        );
        obj.insert(
            "stream_read_error".to_string(),
            serde_json::Value::Bool(stream_read_error.is_some()),
        );
        if let Some(err) = &stream_read_error {
            obj.insert(
                "stream_read_error_message".to_string(),
                serde_json::Value::String(err.clone()),
            );
        }
    }

    let mut checks = serde_json::json!({
        "output_text_chars": output_text_chars as i64,
        "thinking_chars": thinking_chars as i64,
        "signature_chars": signature_chars as i64,
        "has_response_id": response_id.is_some(),
        "has_service_tier": service_tier.is_some(),
        "sse_message_delta_seen": sse_message_delta_seen,
        "sse_message_delta_stop_reason": sse_message_delta_stop_reason,
        "sse_message_delta_stop_reason_is_max_tokens": sse_message_delta_stop_reason_is_max_tokens,
    });
    if let Some(max_chars) = parsed.expect_max_output_chars {
        if let Some(obj) = checks.as_object_mut() {
            obj.insert(
                "expect_max_output_chars".to_string(),
                serde_json::Value::Number((max_chars as i64).into()),
            );
            obj.insert(
                "output_text_chars_le_max".to_string(),
                serde_json::Value::Bool(output_text_chars <= max_chars),
            );
        }
    }
    if let Some(exact_chars) = parsed.expect_exact_output_chars {
        if let Some(obj) = checks.as_object_mut() {
            obj.insert(
                "expect_exact_output_chars".to_string(),
                serde_json::Value::Number((exact_chars as i64).into()),
            );
            obj.insert(
                "output_text_chars_eq_expected".to_string(),
                serde_json::Value::Bool(output_text_chars == exact_chars),
            );
        }
    }

    // “请求成功”口径（用于 ok 与落库）：HTTP 2xx + 有响应数据 + 无 stream 读取错误。
    //
    // 背景：曾出现 HTTP 2xx 但无响应数据（total_read==0）的场景，前端会误判为成功并写入历史。
    let http_ok = (200..300).contains(&status);
    let has_body_bytes = total_read > 0;
    let no_stream_read_error = stream_read_error.is_none();
    let ok = http_ok && has_body_bytes && no_stream_read_error;

    if err_out.is_none() {
        err_out = stream_read_error.clone();
    }
    if http_ok && !has_body_bytes && err_out.is_none() {
        err_out = Some("EMPTY_RESPONSE_BODY".to_string());
    }
    if !http_ok && err_out.is_none() {
        err_out = Some(format!("UPSTREAM_ERROR: status={status}"));
    }

    let result = ClaudeModelValidationResult {
        ok,
        provider_id: provider.id,
        provider_name: provider.name,
        base_url: base_url.to_string(),
        target_url: target_url.to_string(),
        status: Some(status),
        duration_ms: started.elapsed().as_millis().min(i64::MAX as u128) as i64,
        requested_model,
        responded_model,
        stream,
        output_text_chars: output_text_chars.min(i64::MAX as usize) as i64,
        output_text_preview,
        checks,
        signals,
        response_headers,
        usage: usage_json_value,
        error: err_out,
        raw_excerpt: raw_excerpt_text,
        request: sanitized_request,
    };

    // 仅记录“请求成功”的验证：HTTP 2xx + 有响应数据 + 无 stream 读取错误。
    // 例如 HTTP=503、空响应、stream read error 均不写入历史。
    if result.ok {
        let app_handle = app.clone();
        let request_json_text = request_json.to_string();
        let result_json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());
        let _ = blocking::run("claude_validation_history_insert", move || {
            claude_model_validation_history::insert_run_and_prune(
                &app_handle,
                provider.id,
                &request_json_text,
                &result_json,
                Some(50),
            )?;
            Ok(())
        })
        .await;
    }

    Ok(result)
}

pub async fn get_provider_api_key_plaintext(
    app: &tauri::AppHandle,
    provider_id: i64,
) -> Result<String, String> {
    let provider = load_provider(app.clone(), provider_id).await?;
    if provider.cli_key != "claude" {
        return Err("SEC_INVALID_INPUT: only cli_key=claude is supported".to_string());
    }
    Ok(provider.api_key_plaintext)
}
