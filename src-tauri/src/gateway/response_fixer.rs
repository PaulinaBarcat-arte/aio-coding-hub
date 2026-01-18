use axum::body::Bytes;
use futures_core::Stream;
use serde_json::Value;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

pub(super) const DEFAULT_MAX_JSON_DEPTH: usize = 200;
pub(super) const DEFAULT_MAX_FIX_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) struct ResponseFixerConfig {
    pub(super) fix_encoding: bool,
    pub(super) fix_sse_format: bool,
    pub(super) fix_truncated_json: bool,
    pub(super) max_json_depth: usize,
    pub(super) max_fix_size: usize,
}

#[derive(Debug, Default, Clone)]
struct ResponseFixerApplied {
    encoding_applied: bool,
    encoding_details: Option<&'static str>,
    sse_applied: bool,
    sse_details: Option<&'static str>,
    json_applied: bool,
    json_details: Option<&'static str>,
}

#[derive(Debug)]
pub(super) struct NonStreamFixOutcome {
    pub(super) body: Bytes,
    pub(super) header_value: &'static str,
    pub(super) special_setting: Option<Value>,
}

pub(super) fn special_settings_json(shared: &Arc<Mutex<Vec<Value>>>) -> Option<String> {
    let guard = shared.lock().ok()?;
    if guard.is_empty() {
        return None;
    }
    Some(serde_json::to_string(&*guard).unwrap_or_else(|_| "[]".to_string()))
}

fn build_fixers_applied(
    applied: &ResponseFixerApplied,
    include_sse: bool,
) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::with_capacity(if include_sse { 3 } else { 2 });
    out.push(serde_json::json!({
        "fixer": "encoding",
        "applied": applied.encoding_applied,
        "details": applied.encoding_details,
    }));
    if include_sse {
        out.push(serde_json::json!({
            "fixer": "sse",
            "applied": applied.sse_applied,
            "details": applied.sse_details,
        }));
    }
    out.push(serde_json::json!({
        "fixer": "json",
        "applied": applied.json_applied,
        "details": applied.json_details,
    }));
    out
}

fn build_special_setting(
    hit: bool,
    applied: &ResponseFixerApplied,
    include_sse: bool,
    total_bytes_processed: usize,
    processing_time_ms: u64,
) -> Value {
    serde_json::json!({
        "type": "response_fixer",
        "scope": "response",
        "hit": hit,
        "fixersApplied": build_fixers_applied(applied, include_sse),
        "totalBytesProcessed": total_bytes_processed as u64,
        "processingTimeMs": processing_time_ms,
    })
}

pub(super) fn process_non_stream(body: Bytes, config: ResponseFixerConfig) -> NonStreamFixOutcome {
    let started = Instant::now();
    let mut applied = ResponseFixerApplied::default();

    let mut data = body;
    let total_bytes_processed = data.len();

    if config.fix_encoding {
        let res = EncodingFixer::fix_bytes(data);
        if res.applied {
            applied.encoding_applied = true;
            applied.encoding_details = res.details;
        }
        data = res.data;
    }

    if config.fix_truncated_json {
        let fixer = JsonFixer::new(config.max_json_depth, config.max_fix_size);
        let res = fixer.fix_bytes(data);
        if res.applied {
            applied.json_applied = true;
            applied.json_details = res.details;
        }
        data = res.data;
    }

    let audit_hit = applied.encoding_applied || applied.json_applied;
    let processing_time_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;

    let special_setting = if audit_hit {
        Some(build_special_setting(
            true,
            &applied,
            false,
            total_bytes_processed,
            processing_time_ms,
        ))
    } else {
        None
    };

    NonStreamFixOutcome {
        body: data,
        header_value: if audit_hit { "applied" } else { "not-applied" },
        special_setting,
    }
}

#[derive(Debug)]
struct FixBytesOutcome {
    data: Bytes,
    applied: bool,
    details: Option<&'static str>,
}

struct EncodingFixer;

impl EncodingFixer {
    fn has_utf8_bom(data: &[u8]) -> bool {
        data.len() >= 3 && data[0] == 0xef && data[1] == 0xbb && data[2] == 0xbf
    }

    fn has_utf16_bom(data: &[u8]) -> bool {
        data.len() >= 2
            && ((data[0] == 0xfe && data[1] == 0xff) || (data[0] == 0xff && data[1] == 0xfe))
    }

    fn is_valid_utf8(data: &[u8]) -> bool {
        std::str::from_utf8(data).is_ok()
    }

    fn strip_null_bytes(data: &[u8]) -> Option<Vec<u8>> {
        let first_null = data.iter().position(|b| *b == 0)?;
        let null_count = data[first_null..].iter().filter(|b| **b == 0).count();
        let mut out = Vec::with_capacity(data.len().saturating_sub(null_count));
        out.extend_from_slice(&data[..first_null]);
        for b in &data[first_null..] {
            if *b != 0 {
                out.push(*b);
            }
        }
        Some(out)
    }

    fn can_fix(data: &[u8]) -> bool {
        if Self::has_utf8_bom(data) || Self::has_utf16_bom(data) {
            return true;
        }
        if data.contains(&0) {
            return true;
        }
        !Self::is_valid_utf8(data)
    }

    fn fix_bytes(input: Bytes) -> FixBytesOutcome {
        if !Self::can_fix(input.as_ref()) {
            return FixBytesOutcome {
                data: input,
                applied: false,
                details: None,
            };
        }

        let mut details: Option<&'static str> = None;
        let mut changed_by_strip = false;

        // 先去 BOM（可零拷贝 slice）。
        let mut data = if Self::has_utf8_bom(input.as_ref()) {
            changed_by_strip = true;
            details = Some("removed_utf8_bom");
            input.slice(3..)
        } else if Self::has_utf16_bom(input.as_ref()) {
            changed_by_strip = true;
            details = Some("removed_utf16_bom");
            input.slice(2..)
        } else {
            input
        };

        // 去空字节（需要重建 buffer）。
        if let Some(stripped) = Self::strip_null_bytes(data.as_ref()) {
            changed_by_strip = true;
            if details.is_none() {
                details = Some("removed_null_bytes");
            }
            data = Bytes::from(stripped);
        }

        if Self::is_valid_utf8(data.as_ref()) {
            return FixBytesOutcome {
                data,
                applied: changed_by_strip,
                details,
            };
        }

        // 有损修复：用 replacement char 替换无效序列，再重新编码，保证输出一定是合法 UTF-8。
        let lossy = String::from_utf8_lossy(data.as_ref());
        FixBytesOutcome {
            data: Bytes::from(lossy.into_owned().into_bytes()),
            applied: true,
            details: Some("lossy_utf8_decode_encode"),
        }
    }
}

struct SseFixer;

impl SseFixer {
    fn is_ascii_whitespace(byte: u8) -> bool {
        byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r'
    }

    fn to_lower_ascii(byte: u8) -> u8 {
        byte.to_ascii_lowercase()
    }

    fn starts_with_bytes(data: &[u8], prefix: &[u8]) -> bool {
        data.len() >= prefix.len() && data[..prefix.len()] == *prefix
    }

    fn includes_data_colon(data: &[u8]) -> bool {
        const DATA_COLON: &[u8] = b"data:";
        if data.len() < DATA_COLON.len() {
            return false;
        }
        data.windows(DATA_COLON.len()).any(|w| w == DATA_COLON)
    }

    fn looks_like_json_line(line: &[u8]) -> bool {
        let mut i = 0usize;
        while i < line.len() && Self::is_ascii_whitespace(line[i]) {
            i += 1;
        }
        if i >= line.len() {
            return false;
        }

        match line[i] {
            b'{' | b'[' => return true,
            _ => {}
        }

        // [DONE]
        const DONE: &[u8] = b"[DONE]";
        line[i..].starts_with(DONE)
    }

    fn can_fix(data: &[u8]) -> bool {
        if Self::starts_with_bytes(data, b"data:")
            || Self::starts_with_bytes(data, b"event:")
            || Self::starts_with_bytes(data, b"id:")
            || Self::starts_with_bytes(data, b"retry:")
            || Self::starts_with_bytes(data, b":")
        {
            return true;
        }

        // data: 字段常见畸形写法（Data:/DATA:/data : ...）
        if data.len() >= 4 {
            let lower = [
                Self::to_lower_ascii(data[0]),
                Self::to_lower_ascii(data[1]),
                Self::to_lower_ascii(data[2]),
                Self::to_lower_ascii(data[3]),
            ];
            if lower == *b"data" {
                return true;
            }
        }

        if Self::looks_like_json_line(data) {
            return true;
        }

        Self::includes_data_colon(data)
    }

    fn fix_field_space(prefix: &[u8], line: &[u8]) -> Option<Vec<u8>> {
        if !Self::starts_with_bytes(line, prefix) {
            return None;
        }
        let after = &line[prefix.len()..];
        if !after.is_empty() && after[0] == b' ' {
            return None;
        }
        let mut out = Vec::with_capacity(prefix.len() + 1 + after.len());
        out.extend_from_slice(prefix);
        out.push(b' ');
        out.extend_from_slice(after);
        Some(out)
    }

    fn try_fix_malformed(line: &[u8]) -> Option<Vec<u8>> {
        // 模式 1: "data :xxx"（data 与冒号之间只有空白）
        if Self::starts_with_bytes(line, b"data") {
            let rest = &line[4..];
            let colon_pos = rest.iter().position(|b| *b == b':');
            if let Some(colon_idx) = colon_pos {
                if rest[..colon_idx]
                    .iter()
                    .all(|b| Self::is_ascii_whitespace(*b))
                {
                    let after_colon = &rest[(colon_idx + 1)..];
                    let mut j = 0usize;
                    while j < after_colon.len() && after_colon[j] == b' ' {
                        j += 1;
                    }
                    let trimmed = &after_colon[j..];
                    let mut out = Vec::with_capacity(6 + trimmed.len());
                    out.extend_from_slice(b"data: ");
                    out.extend_from_slice(trimmed);
                    return Some(out);
                }
            }
        }

        // 模式 2: Data:/DATA: 等大小写错误
        if line.len() >= 5 {
            let lower = [
                Self::to_lower_ascii(line[0]),
                Self::to_lower_ascii(line[1]),
                Self::to_lower_ascii(line[2]),
                Self::to_lower_ascii(line[3]),
                Self::to_lower_ascii(line[4]),
            ];
            if lower == *b"data:" {
                let mut normalized = Vec::with_capacity(line.len());
                normalized.extend_from_slice(b"data:");
                normalized.extend_from_slice(&line[5..]);
                if let Some(fixed) = Self::fix_field_space(b"data:", &normalized) {
                    return Some(fixed);
                }
                return Some(normalized);
            }
        }

        None
    }

    fn fix_line(line: &[u8]) -> Option<Vec<u8>> {
        // 先匹配“合法字段行”，避免把正常的 data/event/id/retry 行误判为 malformed。
        if Self::starts_with_bytes(line, b"data:") {
            return Self::fix_field_space(b"data:", line);
        }
        if Self::starts_with_bytes(line, b"event:") {
            return Self::fix_field_space(b"event:", line);
        }
        if Self::starts_with_bytes(line, b"id:") {
            return Self::fix_field_space(b"id:", line);
        }
        if Self::starts_with_bytes(line, b"retry:") {
            return Self::fix_field_space(b"retry:", line);
        }

        if Self::starts_with_bytes(line, b":") {
            return None;
        }

        if Self::looks_like_json_line(line) {
            let mut out = Vec::with_capacity(6 + line.len());
            out.extend_from_slice(b"data: ");
            out.extend_from_slice(line);
            return Some(out);
        }

        Self::try_fix_malformed(line)
    }

    fn fix_bytes(input: Bytes) -> FixBytesOutcome {
        if !Self::can_fix(input.as_ref()) {
            return FixBytesOutcome {
                data: input,
                applied: false,
                details: None,
            };
        }

        let bytes = input.as_ref();
        let mut out: Option<Vec<u8>> = None;
        let mut cursor = 0usize;
        let mut changed = false;
        let mut last_was_empty = false;

        let mut pos = 0usize;
        while pos < bytes.len() {
            let start = pos;
            let mut scan = start;
            let mut line_end = bytes.len();
            let mut next_pos = bytes.len();
            let mut newline_normalized = false;

            while scan < bytes.len() {
                match bytes[scan] {
                    b'\n' => {
                        line_end = scan;
                        next_pos = scan + 1;
                        break;
                    }
                    b'\r' => {
                        line_end = scan;
                        next_pos = scan + 1;
                        if next_pos < bytes.len() && bytes[next_pos] == b'\n' {
                            next_pos += 1;
                        }
                        newline_normalized = true;
                        break;
                    }
                    _ => scan += 1,
                }
            }

            // 末尾无换行：补一个 LF
            if next_pos == bytes.len() && line_end == bytes.len() {
                newline_normalized = true;
            }

            pos = next_pos;
            let line = &bytes[start..line_end];

            if line.is_empty() {
                if last_was_empty {
                    changed = true;
                    if out.is_none() {
                        let mut v = Vec::new();
                        if start > 0 {
                            v.extend_from_slice(&bytes[..start]);
                        }
                        out = Some(v);
                    } else if cursor < start {
                        if let Some(out_vec) = out.as_mut() {
                            out_vec.extend_from_slice(&bytes[cursor..start]);
                        }
                    }
                    // 连续空行：跳过当前行（不输出任何内容）
                    cursor = pos;
                    continue;
                }
                last_was_empty = true;
                if newline_normalized {
                    changed = true;
                    if out.is_none() {
                        let mut v = Vec::new();
                        if start > 0 {
                            v.extend_from_slice(&bytes[..start]);
                        }
                        out = Some(v);
                    } else if cursor < start {
                        if let Some(out_vec) = out.as_mut() {
                            out_vec.extend_from_slice(&bytes[cursor..start]);
                        }
                    }
                    if let Some(out_vec) = out.as_mut() {
                        out_vec.push(b'\n');
                    }
                    cursor = pos;
                } else if let Some(out) = out.as_mut() {
                    out.extend_from_slice(&bytes[cursor..pos]);
                    cursor = pos;
                }
                continue;
            }
            last_was_empty = false;

            let fixed = Self::fix_line(line);
            let segment_changed = fixed.is_some() || newline_normalized;
            if segment_changed {
                changed = true;
                if out.is_none() {
                    let mut v = Vec::new();
                    if start > 0 {
                        v.extend_from_slice(&bytes[..start]);
                    }
                    out = Some(v);
                } else if cursor < start {
                    if let Some(out_vec) = out.as_mut() {
                        out_vec.extend_from_slice(&bytes[cursor..start]);
                    }
                }

                if let Some(out_vec) = out.as_mut() {
                    if let Some(fixed_line) = fixed {
                        out_vec.extend_from_slice(&fixed_line);
                    } else {
                        out_vec.extend_from_slice(line);
                    }
                    out_vec.push(b'\n');
                }
                cursor = pos;
                continue;
            }

            if let Some(out_vec) = out.as_mut() {
                out_vec.extend_from_slice(&bytes[cursor..pos]);
                cursor = pos;
            }
        }

        let Some(mut out_vec) = out else {
            return FixBytesOutcome {
                data: input,
                applied: false,
                details: None,
            };
        };

        if cursor < bytes.len() {
            out_vec.extend_from_slice(&bytes[cursor..]);
        }

        FixBytesOutcome {
            data: Bytes::from(out_vec),
            applied: changed,
            details: None,
        }
    }
}

struct JsonFixer {
    max_depth: usize,
    max_size: usize,
}

impl JsonFixer {
    fn new(max_depth: usize, max_size: usize) -> Self {
        Self {
            max_depth,
            max_size,
        }
    }

    fn is_whitespace(byte: u8) -> bool {
        byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r'
    }

    fn looks_like_json(data: &[u8]) -> bool {
        for b in data {
            if Self::is_whitespace(*b) {
                continue;
            }
            return *b == b'{' || *b == b'[';
        }
        false
    }

    fn remove_trailing_comma(out: &mut Vec<u8>) {
        let mut idx = out.len();
        while idx > 0 && Self::is_whitespace(out[idx - 1]) {
            idx -= 1;
        }
        if idx > 0 && out[idx - 1] == b',' {
            out.truncate(idx - 1);
        }
    }

    fn needs_null_value(out: &[u8], stack: &[u8]) -> bool {
        if stack.last().copied() != Some(b'}') {
            return false;
        }
        let mut idx = out.len();
        while idx > 0 && Self::is_whitespace(out[idx - 1]) {
            idx -= 1;
        }
        idx > 0 && out[idx - 1] == b':'
    }

    fn can_fix(&self, data: &[u8]) -> bool {
        Self::looks_like_json(data)
    }

    fn fix_bytes(&self, input: Bytes) -> FixBytesOutcome {
        match self.fix_slice(input.as_ref()) {
            FixSliceOutcome::Unchanged => FixBytesOutcome {
                data: input,
                applied: false,
                details: None,
            },
            FixSliceOutcome::Applied(bytes) => FixBytesOutcome {
                data: Bytes::from(bytes),
                applied: true,
                details: None,
            },
            FixSliceOutcome::Skipped(details) => FixBytesOutcome {
                data: input,
                applied: false,
                details: Some(details),
            },
        }
    }

    fn repair(&self, data: &[u8]) -> Option<Vec<u8>> {
        let mut out: Vec<u8> = Vec::with_capacity(data.len().saturating_add(8));
        let mut stack: Vec<u8> = Vec::new();

        let mut in_string = false;
        let mut escape_next = false;
        let mut depth = 0usize;

        for &byte in data {
            if escape_next {
                escape_next = false;
                out.push(byte);
                continue;
            }

            if in_string && byte == b'\\' {
                escape_next = true;
                out.push(byte);
                continue;
            }

            if byte == b'"' {
                in_string = !in_string;
                out.push(byte);
                continue;
            }

            if !in_string {
                match byte {
                    b'{' => {
                        depth += 1;
                        if depth > self.max_depth {
                            return None;
                        }
                        stack.push(b'}');
                        out.push(byte);
                        continue;
                    }
                    b'[' => {
                        depth += 1;
                        if depth > self.max_depth {
                            return None;
                        }
                        stack.push(b']');
                        out.push(byte);
                        continue;
                    }
                    b'}' | b']' => {
                        Self::remove_trailing_comma(&mut out);
                        if stack.last().copied() == Some(byte) {
                            stack.pop();
                            depth = depth.saturating_sub(1);
                            out.push(byte);
                        }
                        continue;
                    }
                    _ => {}
                }
            }

            out.push(byte);
        }

        // 末尾不完整的转义序列：去掉最后一个反斜杠
        if escape_next {
            out.pop();
        }

        // 闭合未关闭的字符串
        if in_string {
            out.push(b'"');
        }

        Self::remove_trailing_comma(&mut out);

        // 对象末尾冒号无值：补 null
        if Self::needs_null_value(&out, &stack) {
            out.extend_from_slice(b"null");
        }

        while let Some(close) = stack.pop() {
            Self::remove_trailing_comma(&mut out);
            out.push(close);
        }

        Some(out)
    }
}

enum FixSliceOutcome {
    Unchanged,
    Applied(Vec<u8>),
    Skipped(&'static str),
}

impl JsonFixer {
    fn fix_slice(&self, input: &[u8]) -> FixSliceOutcome {
        if input.len() > self.max_size {
            return FixSliceOutcome::Skipped("exceeded_max_size");
        }

        if !self.can_fix(input) {
            return FixSliceOutcome::Unchanged;
        }

        if serde_json::from_slice::<serde_json::Value>(input).is_ok() {
            return FixSliceOutcome::Unchanged;
        }

        let repaired = match self.repair(input) {
            Some(v) => v,
            None => return FixSliceOutcome::Skipped("repair_failed"),
        };

        if serde_json::from_slice::<serde_json::Value>(&repaired).is_ok() {
            return FixSliceOutcome::Applied(repaired);
        }

        FixSliceOutcome::Skipped("validate_repaired_failed")
    }
}

fn fix_sse_json_lines(input: Bytes, json_fixer: &JsonFixer) -> FixBytesOutcome {
    const LF: u8 = b'\n';

    let bytes = input.as_ref();
    let mut out: Vec<u8> = Vec::new();
    let mut changed = false;

    let mut cursor = 0usize;
    let mut line_start = 0usize;

    for (i, b) in bytes.iter().enumerate() {
        if *b != LF {
            continue;
        }
        let line = &bytes[line_start..i];
        if let Some(fixed_line) = fix_maybe_data_json_line(line, json_fixer) {
            changed = true;
            if cursor < line_start {
                out.extend_from_slice(&bytes[cursor..line_start]);
            }
            out.extend_from_slice(&fixed_line);
            out.push(LF);
            cursor = i + 1;
        } else if changed {
            out.extend_from_slice(&bytes[cursor..(i + 1)]);
            cursor = i + 1;
        }
        line_start = i + 1;
    }

    if line_start < bytes.len() {
        let line = &bytes[line_start..];
        if let Some(fixed_line) = fix_maybe_data_json_line(line, json_fixer) {
            changed = true;
            if cursor < line_start {
                out.extend_from_slice(&bytes[cursor..line_start]);
            }
            out.extend_from_slice(&fixed_line);
        } else if changed {
            out.extend_from_slice(&bytes[cursor..]);
        }
    } else if changed && cursor < bytes.len() {
        out.extend_from_slice(&bytes[cursor..]);
    }

    if !changed {
        return FixBytesOutcome {
            data: input,
            applied: false,
            details: None,
        };
    }

    FixBytesOutcome {
        data: Bytes::from(out),
        applied: true,
        details: None,
    }
}

fn fix_maybe_data_json_line(line: &[u8], json_fixer: &JsonFixer) -> Option<Vec<u8>> {
    const DATA_PREFIX: &[u8] = b"data:";

    if line.len() < DATA_PREFIX.len() {
        return None;
    }
    if !line.starts_with(DATA_PREFIX) {
        return None;
    }

    let mut payload_start = DATA_PREFIX.len();
    if payload_start < line.len() && line[payload_start] == b' ' {
        payload_start += 1;
    }

    let payload = &line[payload_start..];
    let fixed_payload = match json_fixer.fix_slice(payload) {
        FixSliceOutcome::Applied(v) => v,
        _ => return None,
    };

    let mut out = Vec::with_capacity(6 + fixed_payload.len());
    out.extend_from_slice(b"data: ");
    out.extend_from_slice(&fixed_payload);
    Some(out)
}

struct ChunkBuffer {
    chunks: Vec<Bytes>,
    head: usize,
    head_offset: usize,
    total: usize,
    processable_end: usize,
    pending_cr: bool,
}

impl ChunkBuffer {
    fn new() -> Self {
        Self {
            chunks: Vec::new(),
            head: 0,
            head_offset: 0,
            total: 0,
            processable_end: 0,
            pending_cr: false,
        }
    }

    fn len(&self) -> usize {
        self.total
    }

    fn push(&mut self, chunk: Bytes) {
        if chunk.is_empty() {
            return;
        }

        let prev_total = self.total;
        let bytes = chunk.as_ref();
        let chunk_len = bytes.len();

        if self.pending_cr {
            self.processable_end = if bytes.first() == Some(&b'\n') {
                prev_total + 1
            } else {
                prev_total
            };
            self.pending_cr = false;
        }

        for (i, b) in bytes.iter().enumerate() {
            if *b == b'\n' {
                self.processable_end = prev_total + i + 1;
                continue;
            }
            if *b != b'\r' {
                continue;
            }
            if i + 1 < bytes.len() {
                if bytes[i + 1] != b'\n' {
                    self.processable_end = prev_total + i + 1;
                }
                continue;
            }
            self.pending_cr = true;
        }

        self.chunks.push(chunk);
        self.total += chunk_len;
    }

    fn find_processable_end(&self) -> usize {
        if self.total == 0 {
            return 0;
        }
        if self.pending_cr {
            return 0;
        }
        self.processable_end
    }

    fn take(&mut self, size: usize) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }
        if size > self.total {
            panic!("ChunkBuffer.take size exceeds buffered length");
        }

        let mut out: Vec<u8> = Vec::with_capacity(size);
        let mut remaining = size;

        while remaining > 0 {
            let chunk = &self.chunks[self.head];
            let available = chunk.len().saturating_sub(self.head_offset);
            let to_copy = available.min(remaining);
            out.extend_from_slice(&chunk.as_ref()[self.head_offset..(self.head_offset + to_copy)]);

            self.head_offset += to_copy;
            self.total -= to_copy;
            remaining -= to_copy;

            if self.head_offset >= chunk.len() {
                self.head += 1;
                self.head_offset = 0;
            }
        }

        if self.head > 64 {
            self.chunks.drain(0..self.head);
            self.head = 0;
        }

        self.processable_end = self.processable_end.saturating_sub(size);
        out
    }

    fn drain(&mut self) -> Vec<u8> {
        let size = self.total;
        let out = self.take(size);
        self.clear();
        out
    }

    fn flush_to(&mut self, queue: &mut VecDeque<Bytes>) {
        for i in self.head..self.chunks.len() {
            let chunk = &self.chunks[i];
            if i == self.head && self.head_offset > 0 {
                let view = chunk.slice(self.head_offset..);
                if !view.is_empty() {
                    queue.push_back(view);
                }
                continue;
            }
            queue.push_back(chunk.clone());
        }
        self.clear();
    }

    fn clear(&mut self) {
        self.chunks.clear();
        self.head = 0;
        self.head_offset = 0;
        self.total = 0;
        self.processable_end = 0;
        self.pending_cr = false;
    }
}

pub(super) struct ResponseFixerStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    upstream: S,
    config: ResponseFixerConfig,
    special_settings: Arc<Mutex<Vec<Value>>>,
    started: Instant,
    total_bytes_processed: usize,
    applied: ResponseFixerApplied,
    buffer: ChunkBuffer,
    passthrough: bool,
    queued: VecDeque<Bytes>,
    pending_error: Option<reqwest::Error>,
    upstream_done: bool,
    finalized: bool,
}

impl<S> ResponseFixerStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    pub(super) fn new(
        upstream: S,
        config: ResponseFixerConfig,
        special_settings: Arc<Mutex<Vec<Value>>>,
    ) -> Self {
        Self {
            upstream,
            config,
            special_settings,
            started: Instant::now(),
            total_bytes_processed: 0,
            applied: ResponseFixerApplied::default(),
            buffer: ChunkBuffer::new(),
            passthrough: false,
            queued: VecDeque::new(),
            pending_error: None,
            upstream_done: false,
            finalized: false,
        }
    }

    fn finalize_if_needed(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        let hit =
            self.applied.encoding_applied || self.applied.sse_applied || self.applied.json_applied;
        if !hit {
            return;
        }

        let processing_time_ms = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let special = build_special_setting(
            true,
            &self.applied,
            true,
            self.total_bytes_processed,
            processing_time_ms,
        );

        if let Ok(mut guard) = self.special_settings.lock() {
            guard.push(special);
        }
    }

    fn process_bytes(&mut self, input: Bytes) -> Bytes {
        let mut data = input;

        if self.config.fix_encoding {
            let res = EncodingFixer::fix_bytes(data);
            if res.applied {
                self.applied.encoding_applied = true;
                if self.applied.encoding_details.is_none() {
                    self.applied.encoding_details = res.details;
                }
            }
            data = res.data;
        }

        if self.config.fix_sse_format {
            let res = SseFixer::fix_bytes(data);
            if res.applied {
                self.applied.sse_applied = true;
                self.applied.sse_details = self.applied.sse_details.or(res.details);
            }
            data = res.data;
        }

        if self.config.fix_truncated_json {
            let json_fixer = JsonFixer::new(self.config.max_json_depth, self.config.max_fix_size);
            let res = fix_sse_json_lines(data, &json_fixer);
            if res.applied {
                self.applied.json_applied = true;
                self.applied.json_details = self.applied.json_details.or(res.details);
            }
            data = res.data;
        }

        data
    }
}

impl<S> Stream for ResponseFixerStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        loop {
            if let Some(next) = this.queued.pop_front() {
                return Poll::Ready(Some(Ok(next)));
            }

            if let Some(err) = this.pending_error.take() {
                return Poll::Ready(Some(Err(err)));
            }

            if this.upstream_done {
                this.finalize_if_needed();
                return Poll::Ready(None);
            }

            match Pin::new(&mut this.upstream).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    if this.buffer.len() > 0 && !this.passthrough {
                        let drained = Bytes::from(this.buffer.drain());
                        let fixed = this.process_bytes(drained);
                        if !fixed.is_empty() {
                            this.queued.push_back(fixed);
                        }
                    } else {
                        this.buffer.clear();
                    }

                    this.upstream_done = true;
                    this.finalize_if_needed();
                    continue;
                }
                Poll::Ready(Some(Err(err))) => {
                    if this.buffer.len() > 0 && !this.passthrough {
                        let drained = Bytes::from(this.buffer.drain());
                        let fixed = this.process_bytes(drained);
                        if !fixed.is_empty() {
                            this.queued.push_back(fixed);
                        }
                    } else {
                        this.buffer.clear();
                    }

                    this.pending_error = Some(err);
                    this.upstream_done = true;
                    this.finalize_if_needed();
                    continue;
                }
                Poll::Ready(Some(Ok(chunk))) => {
                    this.total_bytes_processed =
                        this.total_bytes_processed.saturating_add(chunk.len());

                    if this.passthrough {
                        return Poll::Ready(Some(Ok(chunk)));
                    }

                    // 安全保护：如果长时间无换行，buffer 会持续增长。达到上限后降级为透传，避免内存无界增长。
                    if this.buffer.len().saturating_add(chunk.len()) > this.config.max_fix_size {
                        this.passthrough = true;
                        this.buffer.flush_to(&mut this.queued);
                        this.queued.push_back(chunk);
                        continue;
                    }

                    this.buffer.push(chunk);

                    let end = this.buffer.find_processable_end();
                    if end == 0 {
                        continue;
                    }

                    let to_process = Bytes::from(this.buffer.take(end));
                    let fixed = this.process_bytes(to_process);
                    if !fixed.is_empty() {
                        this.queued.push_back(fixed);
                    }
                    continue;
                }
            }
        }
    }
}

impl<S> Drop for ResponseFixerStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    fn drop(&mut self) {
        self.finalize_if_needed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};

    #[test]
    fn encoding_fixer_valid_utf8_passthrough() {
        let input = Bytes::from_static("Hello 世界".as_bytes());
        let res = EncodingFixer::fix_bytes(input.clone());
        assert!(!res.applied);
        assert_eq!(res.data, input);
    }

    #[test]
    fn encoding_fixer_removes_utf8_bom() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        bytes.extend_from_slice(b"Hello");
        let input = Bytes::from(bytes);
        let res = EncodingFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(std::str::from_utf8(res.data.as_ref()).unwrap(), "Hello");
    }

    #[test]
    fn encoding_fixer_removes_utf16_bom() {
        // UTF-16LE BOM + "A"（0x41 0x00）
        let input = Bytes::from_static(&[0xff, 0xfe, 0x41, 0x00]);
        let res = EncodingFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(std::str::from_utf8(res.data.as_ref()).unwrap(), "A");
    }

    #[test]
    fn encoding_fixer_removes_null_bytes() {
        let input = Bytes::from_static(&[0x48, 0x65, 0x00, 0x6c, 0x6c, 0x6f]);
        let res = EncodingFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(std::str::from_utf8(res.data.as_ref()).unwrap(), "Hello");
    }

    #[test]
    fn encoding_fixer_lossy_fix_invalid_utf8() {
        // 0xC3 0x28 是无效 UTF-8 序列
        let input = Bytes::from_static(&[0xc3, 0x28, 0x61]);
        let res = EncodingFixer::fix_bytes(input);
        assert!(res.applied);
        assert!(std::str::from_utf8(res.data.as_ref()).is_ok());
    }

    #[test]
    fn sse_fixer_fixes_data_space() {
        let input = Bytes::from_static(b"data:{\"test\":true}\n");
        let res = SseFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: {\"test\":true}\n");
    }

    #[test]
    fn sse_fixer_valid_sse_passthrough_ptr_eq() {
        let input = Bytes::from_static(b"data: {\"test\": true}\n");
        let res = SseFixer::fix_bytes(input.clone());
        assert!(!res.applied);
        assert_eq!(res.data, input);
    }

    #[test]
    fn sse_fixer_wraps_json_line_and_done() {
        let json_line = Bytes::from_static(b"{\"content\":\"hello\"}\n");
        let res = SseFixer::fix_bytes(json_line);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: {\"content\":\"hello\"}\n");

        let done = Bytes::from_static(b"[DONE]\n");
        let res = SseFixer::fix_bytes(done);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: [DONE]\n");
    }

    #[test]
    fn sse_fixer_keeps_comment_and_fixes_fields() {
        let input = Bytes::from_static(
            b": this is a comment\nevent:message\nid:123\nretry:1000\ndata: test\n",
        );
        let res = SseFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(
            res.data.as_ref(),
            b": this is a comment\nevent: message\nid: 123\nretry: 1000\ndata: test\n"
        );
    }

    #[test]
    fn sse_fixer_normalizes_crlf_and_cr() {
        let input = Bytes::from_static(b"data: test\r\ndata: test2\r\n");
        let res = SseFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: test\ndata: test2\n");

        let input = Bytes::from_static(b"data: test\rdata: test2\r");
        let res = SseFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: test\ndata: test2\n");
    }

    #[test]
    fn sse_fixer_fixes_data_case_and_data_space_variants() {
        let input = Bytes::from_static(b"Data:{\"test\": true}\n");
        let res = SseFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: {\"test\": true}\n");

        let input = Bytes::from_static(b"data :{\"test\": true}\n");
        let res = SseFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: {\"test\": true}\n");
    }

    #[test]
    fn sse_fixer_merges_consecutive_blank_lines() {
        let input = Bytes::from_static(b"data: test\n\n\n\ndata: test2\n");
        let res = SseFixer::fix_bytes(input);
        assert!(res.applied);
        assert_eq!(res.data.as_ref(), b"data: test\n\ndata: test2\n");
    }

    #[test]
    fn json_fixer_repairs_truncated_object() {
        let fixer = JsonFixer::new(200, 1024 * 1024);
        let input = Bytes::from_static(br#"{"key":"value""#);
        let res = fixer.fix_bytes(input);
        assert!(res.applied);
        assert!(serde_json::from_slice::<serde_json::Value>(res.data.as_ref()).is_ok());
    }

    #[test]
    fn json_fixer_repairs_common_truncations() {
        let fixer = JsonFixer::new(200, 1024 * 1024);

        for input in [
            Bytes::from_static(br#"{"key":"value""#),
            Bytes::from_static(br#"[1, 2, 3"#),
            Bytes::from_static(br#"{"key":"val"#),
            Bytes::from_static(br#"{"a": 1,}"#),
            Bytes::from_static(br#"[1, 2,]"#),
            Bytes::from_static(b"{\"key\":"),
            Bytes::from_static(br#"{"key":"value", "outer": {"inner": [1, 2"#),
        ] {
            let res = fixer.fix_bytes(input);
            assert!(serde_json::from_slice::<serde_json::Value>(res.data.as_ref()).is_ok());
        }
    }

    #[test]
    fn json_fixer_appends_null_when_missing_value() {
        let fixer = JsonFixer::new(200, 1024 * 1024);
        let input = Bytes::from_static(b"{\"key\":");
        let res = fixer.fix_bytes(input);
        let v: serde_json::Value = serde_json::from_slice(res.data.as_ref()).unwrap();
        assert_eq!(v, serde_json::json!({"key": null}));
    }

    #[test]
    fn json_fixer_depth_and_size_protection() {
        let input = Bytes::from_static(br#"{"a":{"b":{"c":{"d":"#);
        let fixer = JsonFixer::new(3, 1024 * 1024);
        let res = fixer.fix_bytes(input.clone());
        assert!(!res.applied);
        assert_eq!(res.data.as_ref(), input.as_ref());

        let input = Bytes::from_static(br#"{"key":"very long value"}"#);
        let fixer = JsonFixer::new(200, 10);
        let res = fixer.fix_bytes(input.clone());
        assert!(!res.applied);
        assert_eq!(res.data.as_ref(), input.as_ref());
    }

    #[test]
    fn response_fixer_non_stream_writes_special_setting_when_hit() {
        let config = ResponseFixerConfig {
            fix_encoding: true,
            fix_sse_format: true,
            fix_truncated_json: true,
            max_json_depth: DEFAULT_MAX_JSON_DEPTH,
            max_fix_size: DEFAULT_MAX_FIX_SIZE,
        };

        let mut bom_json = Vec::new();
        bom_json.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        bom_json.extend_from_slice(br#"{"a":1}"#);

        let out = process_non_stream(Bytes::from(bom_json), config);
        assert_eq!(out.body.as_ref(), br#"{"a":1}"#);
        assert_eq!(out.header_value, "applied");
        assert!(out.special_setting.is_some());
    }

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

    #[tokio::test]
    async fn response_fixer_stream_fixes_truncated_json_in_data_line_across_chunks() {
        let special_settings = Arc::new(Mutex::new(Vec::new()));
        let config = ResponseFixerConfig {
            fix_encoding: true,
            fix_sse_format: true,
            fix_truncated_json: true,
            max_json_depth: DEFAULT_MAX_JSON_DEPTH,
            max_fix_size: DEFAULT_MAX_FIX_SIZE,
        };

        let upstream = VecBytesStream::new(vec![
            Ok(Bytes::from_static(b"data: {\"key\":")),
            Ok(Bytes::from_static(b"\n\n")),
        ]);

        let stream = ResponseFixerStream::new(upstream, config, special_settings.clone());
        let out = collect_ok_bytes(stream).await;
        assert_eq!(out, b"data: {\"key\":null}\n\n");

        let settings = special_settings.lock().unwrap();
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0]["type"], "response_fixer");
        assert_eq!(settings[0]["hit"], true);
    }

    #[tokio::test]
    async fn response_fixer_stream_valid_sse_should_not_write_special_settings() {
        let special_settings = Arc::new(Mutex::new(Vec::new()));
        let config = ResponseFixerConfig {
            fix_encoding: true,
            fix_sse_format: true,
            fix_truncated_json: true,
            max_json_depth: DEFAULT_MAX_JSON_DEPTH,
            max_fix_size: DEFAULT_MAX_FIX_SIZE,
        };

        let upstream = VecBytesStream::new(vec![Ok(Bytes::from_static(b"data: {\"a\":1}\n\n"))]);
        let stream = ResponseFixerStream::new(upstream, config, special_settings.clone());
        let out = collect_ok_bytes(stream).await;
        assert_eq!(out, b"data: {\"a\":1}\n\n");

        let settings = special_settings.lock().unwrap();
        assert!(settings.is_empty());
    }

    #[tokio::test]
    async fn response_fixer_stream_degrades_when_exceeding_max_fix_size_without_newlines() {
        let special_settings = Arc::new(Mutex::new(Vec::new()));
        let config = ResponseFixerConfig {
            fix_encoding: true,
            fix_sse_format: true,
            fix_truncated_json: true,
            max_json_depth: DEFAULT_MAX_JSON_DEPTH,
            max_fix_size: 12,
        };

        let upstream = VecBytesStream::new(vec![
            Ok(Bytes::from_static(b"data: {\"k\":")),
            Ok(Bytes::from_static(b"\"v\"")),
        ]);

        let mut stream = ResponseFixerStream::new(upstream, config, special_settings.clone());
        let first = next_item(&mut stream)
            .await
            .expect("should produce some output")
            .expect("should be ok");
        assert!(!first.is_empty());

        // 清理：拉取到结束，确保 finalize 运行
        while let Some(_item) = next_item(&mut stream).await {}

        let settings = special_settings.lock().unwrap();
        assert!(settings.is_empty());
    }
}
