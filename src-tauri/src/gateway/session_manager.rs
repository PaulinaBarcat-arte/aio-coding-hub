use axum::http::HeaderMap;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

const DEFAULT_SESSION_TTL_SECS: i64 = 300;
const MAX_SESSION_ID_LEN: usize = 256;
const MAX_BINDINGS: usize = 5000;
const SESSION_SUFFIX_LEN: usize = 8;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveSessionSnapshot {
    pub cli_key: String,
    pub session_id: String,
    pub session_suffix: String,
    pub provider_id: i64,
    pub expires_at: i64,
}

#[derive(Debug)]
pub struct SessionManager {
    ttl_secs: i64,
    bindings: Mutex<HashMap<SessionKey, SessionBinding>>,
}

#[derive(Debug, Clone)]
struct SessionBinding {
    provider_id: i64,
    sort_mode_id: Option<i64>,
    provider_order: Option<Vec<i64>>,
    expires_at: i64,
}

#[derive(Debug, Clone, Eq)]
struct SessionKey {
    cli_key: String,
    session_id: String,
}

impl PartialEq for SessionKey {
    fn eq(&self, other: &Self) -> bool {
        self.cli_key == other.cli_key && self.session_id == other.session_id
    }
}

impl Hash for SessionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.cli_key.hash(state);
        self.session_id.hash(state);
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            ttl_secs: DEFAULT_SESSION_TTL_SECS,
            bindings: Mutex::new(HashMap::new()),
        }
    }

    pub fn extract_session_id_from_json(
        headers: &HeaderMap,
        root: Option<&Value>,
    ) -> Option<String> {
        // 1) client headers
        if let Some(v) = header_string(headers, "session_id") {
            if let Some(id) = sanitize_session_id(&v) {
                return Some(id);
            }
        }
        if let Some(v) = header_string(headers, "x-session-id") {
            if let Some(id) = sanitize_session_id(&v) {
                return Some(id);
            }
        }

        // 2) best-effort JSON extraction
        if let Some(root) = root {
            // Common: { "session_id": "..." }
            if let Some(id) = root.get("session_id").and_then(|v| v.as_str()) {
                if let Some(id) = sanitize_session_id(id) {
                    return Some(id);
                }
            }

            // Common: { "conversation_id": "..." } or { "thread_id": "..." }
            for key in ["conversation_id", "thread_id", "chat_id"] {
                if let Some(id) = root.get(key).and_then(|v| v.as_str()) {
                    if let Some(id) = sanitize_session_id(id) {
                        return Some(id);
                    }
                }
            }

            // Codex-style: prompt_cache_key (UUID-like, prefer when present)
            if let Some(id) = root.get("prompt_cache_key").and_then(|v| v.as_str()) {
                let trimmed = id.trim();
                if trimmed.len() > 20 {
                    if let Some(id) = sanitize_session_id(trimmed) {
                        return Some(id);
                    }
                }
            }

            // claude-code-hub: { "metadata": { "session_id": "..." } }
            if let Some(meta) = root.get("metadata").and_then(|v| v.as_object()) {
                if let Some(id) = meta.get("session_id").and_then(|v| v.as_str()) {
                    if let Some(id) = sanitize_session_id(id) {
                        return Some(id);
                    }
                }

                // claude-code-hub: metadata.user_id contains "_session_" marker (Claude Code)
                if let Some(user_id) = meta.get("user_id").and_then(|v| v.as_str()) {
                    let marker = "_session_";
                    if let Some(idx) = user_id.find(marker) {
                        let extracted = &user_id[idx + marker.len()..];
                        if let Some(id) = sanitize_session_id(extracted) {
                            return Some(id);
                        }
                    }
                }
            }

            // Codex-style fallback: previous_response_id
            if let Some(prev) = root.get("previous_response_id").and_then(|v| v.as_str()) {
                if let Some(id) = sanitize_session_id(prev) {
                    if let Some(out) = sanitize_session_id(&format!("codex_prev_{id}")) {
                        return Some(out);
                    }
                }
            }
        }

        // 3) deterministic session id fallback (align claude-code-hub)
        deterministic_session_id(headers).and_then(|id| sanitize_session_id(&id))
    }

    pub fn get_bound_provider(
        &self,
        cli_key: &str,
        session_id: &str,
        now_unix: i64,
    ) -> Option<i64> {
        let key = SessionKey {
            cli_key: cli_key.to_string(),
            session_id: session_id.to_string(),
        };

        let mut guard = self
            .bindings
            .lock()
            .expect("session_manager mutex poisoned");
        match guard.get(&key) {
            Some(binding) if binding.expires_at > now_unix => {
                (binding.provider_id > 0).then_some(binding.provider_id)
            }
            Some(_) => {
                guard.remove(&key);
                None
            }
            None => None,
        }
    }

    // Returns `Some(sort_mode_id)` when a session binding exists (even if the bound mode is `None`).
    pub fn get_bound_sort_mode_id(
        &self,
        cli_key: &str,
        session_id: &str,
        now_unix: i64,
    ) -> Option<Option<i64>> {
        let key = SessionKey {
            cli_key: cli_key.to_string(),
            session_id: session_id.to_string(),
        };

        let mut guard = self
            .bindings
            .lock()
            .expect("session_manager mutex poisoned");
        match guard.get(&key) {
            Some(binding) if binding.expires_at > now_unix => Some(binding.sort_mode_id),
            Some(_) => {
                guard.remove(&key);
                None
            }
            None => None,
        }
    }

    // Bind (or refresh) the session's sort_mode for stickiness across retries.
    // If a binding already exists, its sort_mode_id is preserved and only TTL is refreshed.
    pub fn bind_sort_mode(
        &self,
        cli_key: &str,
        session_id: &str,
        sort_mode_id: Option<i64>,
        provider_order: Option<Vec<i64>>,
        now_unix: i64,
    ) {
        if cli_key.trim().is_empty() || session_id.trim().is_empty() {
            return;
        }

        let key = SessionKey {
            cli_key: cli_key.to_string(),
            session_id: session_id.to_string(),
        };

        let mut guard = self
            .bindings
            .lock()
            .expect("session_manager mutex poisoned");
        if guard.len() >= MAX_BINDINGS {
            drop_expired(&mut guard, now_unix);
            if guard.len() >= MAX_BINDINGS {
                guard.clear();
            }
        }

        if let Some(existing) = guard.get_mut(&key) {
            if existing.expires_at > now_unix {
                existing.expires_at = now_unix.saturating_add(self.ttl_secs.max(1));
                if existing.provider_order.is_none() {
                    existing.provider_order = provider_order;
                }
                return;
            }
            guard.remove(&key);
        }

        guard.insert(
            key,
            SessionBinding {
                provider_id: 0,
                sort_mode_id,
                provider_order,
                expires_at: now_unix.saturating_add(self.ttl_secs.max(1)),
            },
        );
    }

    pub fn get_bound_provider_order(
        &self,
        cli_key: &str,
        session_id: &str,
        now_unix: i64,
    ) -> Option<Vec<i64>> {
        let key = SessionKey {
            cli_key: cli_key.to_string(),
            session_id: session_id.to_string(),
        };

        let mut guard = self
            .bindings
            .lock()
            .expect("session_manager mutex poisoned");
        match guard.get(&key) {
            Some(binding) if binding.expires_at > now_unix => binding.provider_order.clone(),
            Some(_) => {
                guard.remove(&key);
                None
            }
            None => None,
        }
    }

    pub fn bind_success(
        &self,
        cli_key: &str,
        session_id: &str,
        provider_id: i64,
        sort_mode_id: Option<i64>,
        now_unix: i64,
    ) {
        if cli_key.trim().is_empty() || session_id.trim().is_empty() || provider_id <= 0 {
            return;
        }

        let key = SessionKey {
            cli_key: cli_key.to_string(),
            session_id: session_id.to_string(),
        };

        let mut guard = self
            .bindings
            .lock()
            .expect("session_manager mutex poisoned");
        if guard.len() >= MAX_BINDINGS {
            drop_expired(&mut guard, now_unix);
            if guard.len() >= MAX_BINDINGS {
                guard.clear();
            }
        }

        let expires_at = now_unix.saturating_add(self.ttl_secs.max(1));
        if let Some(existing) = guard.get_mut(&key) {
            if existing.expires_at > now_unix {
                existing.provider_id = provider_id;
                existing.expires_at = expires_at;
                if existing.sort_mode_id.is_none() {
                    existing.sort_mode_id = sort_mode_id;
                }
                return;
            }
            guard.remove(&key);
        }

        guard.insert(
            key,
            SessionBinding {
                provider_id,
                sort_mode_id,
                provider_order: None,
                expires_at,
            },
        );
    }

    pub fn list_active(&self, now_unix: i64, limit: usize) -> Vec<ActiveSessionSnapshot> {
        if limit == 0 {
            return Vec::new();
        }

        let mut guard = self
            .bindings
            .lock()
            .expect("session_manager mutex poisoned");
        drop_expired(&mut guard, now_unix);

        let mut rows: Vec<ActiveSessionSnapshot> = guard
            .iter()
            .map(|(k, v)| ActiveSessionSnapshot {
                cli_key: k.cli_key.clone(),
                session_id: k.session_id.clone(),
                session_suffix: session_suffix(&k.session_id),
                provider_id: v.provider_id,
                expires_at: v.expires_at,
            })
            .collect();

        rows.sort_by(|a, b| b.expires_at.cmp(&a.expires_at));
        rows.truncate(limit);
        rows
    }
}

fn header_string(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn deterministic_session_id(headers: &HeaderMap) -> Option<String> {
    let api_key_prefix = header_string(headers, "x-api-key")
        .or_else(|| header_string(headers, "x-goog-api-key"))
        .and_then(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            let prefix: String = trimmed.chars().take(10).collect();
            sanitize_deterministic_part(&prefix)
        });

    let user_agent =
        header_string(headers, "user-agent").and_then(|v| sanitize_deterministic_part(&v));

    let forwarded_for = header_string(headers, "x-forwarded-for").and_then(|raw| {
        raw.split(',')
            .map(str::trim)
            .find(|v| !v.is_empty())
            .and_then(sanitize_deterministic_part)
    });
    let real_ip = header_string(headers, "x-real-ip").and_then(|v| sanitize_deterministic_part(&v));
    let ip = forwarded_for.or(real_ip);

    let parts: Vec<String> = [user_agent, ip, api_key_prefix]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() {
        return None;
    }

    let joined = parts.join(":");
    let hash = Sha256::digest(joined.as_bytes());
    let hex = format!("{hash:x}");
    let short = hex.get(..32)?;
    Some(format!("sess_{short}"))
}

fn sanitize_deterministic_part(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = trimmed.to_string();
    out.retain(|c| c != '\n' && c != '\r' && c != '\t');
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn sanitize_session_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = if trimmed.len() > MAX_SESSION_ID_LEN {
        trimmed[..MAX_SESSION_ID_LEN].to_string()
    } else {
        trimmed.to_string()
    };
    // Avoid newlines/whitespace causing log injection if someone mistakenly logs it.
    out.retain(|c| c != '\n' && c != '\r' && c != '\t');
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn session_suffix(session_id: &str) -> String {
    let suffix: Vec<char> = session_id.chars().rev().take(SESSION_SUFFIX_LEN).collect();
    suffix.into_iter().rev().collect()
}

fn drop_expired(map: &mut HashMap<SessionKey, SessionBinding>, now_unix: i64) {
    map.retain(|_, v| v.expires_at > now_unix);
}
