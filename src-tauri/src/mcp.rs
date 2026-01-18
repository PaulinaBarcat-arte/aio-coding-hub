use crate::db;
use crate::mcp_sync;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct McpServerSummary {
    pub id: i64,
    pub server_key: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<String>,
    pub url: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub enabled_claude: bool,
    pub enabled_codex: bool,
    pub enabled_gemini: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpImportServer {
    pub server_key: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<String>,
    pub url: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub enabled_claude: bool,
    pub enabled_codex: bool,
    pub enabled_gemini: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpParseResult {
    pub servers: Vec<McpImportServer>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpImportReport {
    pub inserted: u32,
    pub updated: u32,
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn enabled_to_int(enabled: bool) -> i64 {
    if enabled {
        1
    } else {
        0
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn validate_transport(transport: &str) -> Result<(), String> {
    match transport {
        "stdio" | "http" => Ok(()),
        other => Err(format!("SEC_INVALID_INPUT: unsupported transport={other}")),
    }
}

fn validate_server_key(server_key: &str) -> Result<(), String> {
    let key = server_key.trim();
    if key.is_empty() {
        return Err("SEC_INVALID_INPUT: server_key is required".to_string());
    }
    if key.len() > 64 {
        return Err("SEC_INVALID_INPUT: server_key too long (max 64)".to_string());
    }

    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return Err("SEC_INVALID_INPUT: server_key is required".to_string());
    };
    if !first.is_ascii_alphanumeric() {
        return Err("SEC_INVALID_INPUT: server_key must start with [A-Za-z0-9]".to_string());
    }

    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err("SEC_INVALID_INPUT: server_key allows only [A-Za-z0-9_-]".to_string());
        }
    }

    Ok(())
}

fn server_key_exists(conn: &Connection, server_key: &str) -> Result<bool, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM mcp_servers WHERE server_key = ?1",
            params![server_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("DB_ERROR: failed to query mcp server_key: {e}"))?;
    Ok(exists.is_some())
}

fn generate_unique_server_key(conn: &Connection, name: &str) -> Result<String, String> {
    let base = suggest_key(name);
    let base = base.trim();
    let base = if base.is_empty() { "mcp-server" } else { base };

    // Fast path.
    if !server_key_exists(conn, base)? {
        validate_server_key(base)?;
        return Ok(base.to_string());
    }

    for idx in 2..1000 {
        let suffix = format!("-{idx}");
        let mut candidate = base.to_string();
        if candidate.len() + suffix.len() > 64 {
            candidate.truncate(64 - suffix.len());
        }
        candidate.push_str(&suffix);
        if !server_key_exists(conn, &candidate)? {
            validate_server_key(&candidate)?;
            return Ok(candidate);
        }
    }

    let fallback = format!("mcp-{}", now_unix_seconds());
    validate_server_key(&fallback)?;
    Ok(fallback)
}

fn args_to_json(args: &[String]) -> Result<String, String> {
    serde_json::to_string(args)
        .map_err(|e| format!("SEC_INVALID_INPUT: failed to serialize args: {e}"))
}

fn map_to_json(map: &BTreeMap<String, String>, hint: &str) -> Result<String, String> {
    serde_json::to_string(map)
        .map_err(|e| format!("SEC_INVALID_INPUT: failed to serialize {hint}: {e}"))
}

fn row_to_summary(row: &rusqlite::Row<'_>) -> Result<McpServerSummary, rusqlite::Error> {
    let args_json: String = row.get("args_json")?;
    let env_json: String = row.get("env_json")?;
    let headers_json: String = row.get("headers_json")?;

    let args = serde_json::from_str::<Vec<String>>(&args_json).unwrap_or_default();
    let env = serde_json::from_str::<BTreeMap<String, String>>(&env_json).unwrap_or_default();
    let headers =
        serde_json::from_str::<BTreeMap<String, String>>(&headers_json).unwrap_or_default();

    Ok(McpServerSummary {
        id: row.get("id")?,
        server_key: row.get("server_key")?,
        name: row.get("name")?,
        transport: row.get("transport")?,
        command: row.get("command")?,
        args,
        env,
        cwd: row.get("cwd")?,
        url: row.get("url")?,
        headers,
        enabled_claude: row.get::<_, i64>("enabled_claude")? != 0,
        enabled_codex: row.get::<_, i64>("enabled_codex")? != 0,
        enabled_gemini: row.get::<_, i64>("enabled_gemini")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn get_by_id(conn: &Connection, server_id: i64) -> Result<McpServerSummary, String> {
    conn.query_row(
        r#"
SELECT
  id,
  server_key,
  name,
  transport,
  command,
  args_json,
  env_json,
  cwd,
  url,
  headers_json,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
FROM mcp_servers
WHERE id = ?1
"#,
        params![server_id],
        row_to_summary,
    )
    .optional()
    .map_err(|e| format!("DB_ERROR: failed to query mcp server: {e}"))?
    .ok_or_else(|| "DB_NOT_FOUND: mcp server not found".to_string())
}

pub fn list_all(app: &tauri::AppHandle) -> Result<Vec<McpServerSummary>, String> {
    let conn = db::open_connection(app)?;

    let mut stmt = conn
        .prepare(
            r#"
SELECT
  id,
  server_key,
  name,
  transport,
  command,
  args_json,
  env_json,
  cwd,
  url,
  headers_json,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
FROM mcp_servers
ORDER BY updated_at DESC, id DESC
"#,
        )
        .map_err(|e| format!("DB_ERROR: failed to prepare query: {e}"))?;

    let rows = stmt
        .query_map([], row_to_summary)
        .map_err(|e| format!("DB_ERROR: failed to list mcp servers: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| format!("DB_ERROR: failed to read mcp row: {e}"))?);
    }
    Ok(items)
}

fn list_enabled_for_cli(
    conn: &Connection,
    cli_key: &str,
) -> Result<Vec<mcp_sync::McpServerForSync>, String> {
    let (col, _) = match cli_key {
        "claude" => ("enabled_claude", ".claude.json"),
        "codex" => ("enabled_codex", ".codex/config.toml"),
        "gemini" => ("enabled_gemini", ".gemini/settings.json"),
        _ => return Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    };

    let sql = format!(
        r#"
SELECT
  server_key,
  transport,
  command,
  args_json,
  env_json,
  cwd,
  url,
  headers_json
FROM mcp_servers
WHERE {col} = 1
ORDER BY server_key ASC
"#
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("DB_ERROR: failed to prepare enabled mcp query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let args_json: String = row.get("args_json")?;
            let env_json: String = row.get("env_json")?;
            let headers_json: String = row.get("headers_json")?;

            let args = serde_json::from_str::<Vec<String>>(&args_json).unwrap_or_default();
            let env =
                serde_json::from_str::<BTreeMap<String, String>>(&env_json).unwrap_or_default();
            let headers =
                serde_json::from_str::<BTreeMap<String, String>>(&headers_json).unwrap_or_default();

            Ok(mcp_sync::McpServerForSync {
                server_key: row.get("server_key")?,
                transport: row.get("transport")?,
                command: row.get("command")?,
                args,
                env,
                cwd: row.get("cwd")?,
                url: row.get("url")?,
                headers,
            })
        })
        .map_err(|e| format!("DB_ERROR: failed to query enabled mcp servers: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("DB_ERROR: failed to read enabled mcp row: {e}"))?);
    }
    Ok(out)
}

fn sync_all_cli(app: &tauri::AppHandle, conn: &Connection) -> Result<(), String> {
    let claude = list_enabled_for_cli(conn, "claude")?;
    mcp_sync::sync_cli(app, "claude", &claude)?;

    let codex = list_enabled_for_cli(conn, "codex")?;
    mcp_sync::sync_cli(app, "codex", &codex)?;

    let gemini = list_enabled_for_cli(conn, "gemini")?;
    mcp_sync::sync_cli(app, "gemini", &gemini)?;

    Ok(())
}

fn sync_one_cli(app: &tauri::AppHandle, conn: &Connection, cli_key: &str) -> Result<(), String> {
    let servers = list_enabled_for_cli(conn, cli_key)?;
    mcp_sync::sync_cli(app, cli_key, &servers)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn upsert(
    app: &tauri::AppHandle,
    server_id: Option<i64>,
    server_key: &str,
    name: &str,
    transport: &str,
    command: Option<&str>,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: Option<&str>,
    url: Option<&str>,
    headers: BTreeMap<String, String>,
    enabled_claude: bool,
    enabled_codex: bool,
    enabled_gemini: bool,
) -> Result<McpServerSummary, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("SEC_INVALID_INPUT: name is required".to_string());
    }

    let provided_key = server_key.trim();

    let transport = transport.trim().to_lowercase();
    validate_transport(&transport)?;

    let command = command.map(str::trim).filter(|v| !v.is_empty());
    let url = url.map(str::trim).filter(|v| !v.is_empty());
    let cwd = cwd.map(str::trim).filter(|v| !v.is_empty());

    if transport == "stdio" && command.is_none() {
        return Err("SEC_INVALID_INPUT: stdio command is required".to_string());
    }
    if transport == "http" && url.is_none() {
        return Err("SEC_INVALID_INPUT: http url is required".to_string());
    }

    let args: Vec<String> = args
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let args_json = args_to_json(&args)?;
    let env_json = map_to_json(&env, "env")?;
    let headers_json = map_to_json(&headers, "headers")?;

    let mut conn = db::open_connection(app)?;
    let now = now_unix_seconds();

    let tx = conn
        .transaction()
        .map_err(|e| format!("DB_ERROR: failed to start transaction: {e}"))?;

    let resolved_key = match server_id {
        None => {
            if provided_key.is_empty() {
                generate_unique_server_key(&tx, name)?
            } else {
                validate_server_key(provided_key)?;
                provided_key.to_string()
            }
        }
        Some(id) => {
            let existing_key: Option<String> = tx
                .query_row(
                    "SELECT server_key FROM mcp_servers WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| format!("DB_ERROR: failed to query mcp server: {e}"))?;

            let Some(existing_key) = existing_key else {
                return Err("DB_NOT_FOUND: mcp server not found".to_string());
            };

            if !provided_key.is_empty() && existing_key != provided_key {
                return Err(
                    "SEC_INVALID_INPUT: server_key cannot be changed for existing server"
                        .to_string(),
                );
            }

            existing_key
        }
    };

    let normalized_name = normalize_name(name);

    let prev_claude_target = mcp_sync::read_target_bytes(app, "claude")?;
    let prev_claude_manifest = mcp_sync::read_manifest_bytes(app, "claude")?;
    let prev_codex_target = mcp_sync::read_target_bytes(app, "codex")?;
    let prev_codex_manifest = mcp_sync::read_manifest_bytes(app, "codex")?;
    let prev_gemini_target = mcp_sync::read_target_bytes(app, "gemini")?;
    let prev_gemini_manifest = mcp_sync::read_manifest_bytes(app, "gemini")?;

    let id = match server_id {
        None => {
            tx.execute(
                r#"
INSERT INTO mcp_servers(
  server_key,
  name,
  normalized_name,
  transport,
  command,
  args_json,
  env_json,
  cwd,
  url,
  headers_json,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
"#,
                params![
                    resolved_key,
                    name,
                    normalized_name,
                    transport,
                    command,
                    args_json,
                    env_json,
                    cwd,
                    url,
                    headers_json,
                    enabled_to_int(enabled_claude),
                    enabled_to_int(enabled_codex),
                    enabled_to_int(enabled_gemini),
                    now,
                    now
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    format!("DB_CONSTRAINT: mcp server_key already exists: {resolved_key}")
                }
                other => format!("DB_ERROR: failed to insert mcp server: {other}"),
            })?;
            tx.last_insert_rowid()
        }
        Some(id) => {
            tx.execute(
                r#"
UPDATE mcp_servers
SET
  name = ?1,
  normalized_name = ?2,
  transport = ?3,
  command = ?4,
  args_json = ?5,
  env_json = ?6,
  cwd = ?7,
  url = ?8,
  headers_json = ?9,
  enabled_claude = ?10,
  enabled_codex = ?11,
  enabled_gemini = ?12,
  updated_at = ?13
WHERE id = ?14
"#,
                params![
                    name,
                    normalized_name,
                    transport,
                    command,
                    args_json,
                    env_json,
                    cwd,
                    url,
                    headers_json,
                    enabled_to_int(enabled_claude),
                    enabled_to_int(enabled_codex),
                    enabled_to_int(enabled_gemini),
                    now,
                    id
                ],
            )
            .map_err(|e| format!("DB_ERROR: failed to update mcp server: {e}"))?;
            id
        }
    };

    if let Err(err) = sync_all_cli(app, &tx) {
        let _ = mcp_sync::restore_target_bytes(app, "claude", prev_claude_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "claude", prev_claude_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "codex", prev_codex_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "codex", prev_codex_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "gemini", prev_gemini_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "gemini", prev_gemini_manifest);
        return Err(err);
    }

    if let Err(err) = tx.commit() {
        let _ = mcp_sync::restore_target_bytes(app, "claude", prev_claude_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "claude", prev_claude_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "codex", prev_codex_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "codex", prev_codex_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "gemini", prev_gemini_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "gemini", prev_gemini_manifest);
        return Err(format!("DB_ERROR: failed to commit: {err}"));
    }

    get_by_id(&conn, id)
}

pub fn set_enabled(
    app: &tauri::AppHandle,
    server_id: i64,
    cli_key: &str,
    enabled: bool,
) -> Result<McpServerSummary, String> {
    validate_cli_key(cli_key)?;

    let mut conn = db::open_connection(app)?;
    let now = now_unix_seconds();
    let tx = conn
        .transaction()
        .map_err(|e| format!("DB_ERROR: failed to start transaction: {e}"))?;

    let prev_target = mcp_sync::read_target_bytes(app, cli_key)?;
    let prev_manifest = mcp_sync::read_manifest_bytes(app, cli_key)?;

    let column = match cli_key {
        "claude" => "enabled_claude",
        "codex" => "enabled_codex",
        "gemini" => "enabled_gemini",
        _ => return Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    };

    let sql = format!("UPDATE mcp_servers SET {column} = ?1, updated_at = ?2 WHERE id = ?3");
    let changed = tx
        .execute(&sql, params![enabled_to_int(enabled), now, server_id])
        .map_err(|e| format!("DB_ERROR: failed to update mcp server: {e}"))?;
    if changed == 0 {
        return Err("DB_NOT_FOUND: mcp server not found".to_string());
    }

    if let Err(err) = sync_one_cli(app, &tx, cli_key) {
        let _ = mcp_sync::restore_target_bytes(app, cli_key, prev_target);
        let _ = mcp_sync::restore_manifest_bytes(app, cli_key, prev_manifest);
        return Err(err);
    }

    if let Err(err) = tx.commit() {
        let _ = mcp_sync::restore_target_bytes(app, cli_key, prev_target);
        let _ = mcp_sync::restore_manifest_bytes(app, cli_key, prev_manifest);
        return Err(format!("DB_ERROR: failed to commit: {err}"));
    }

    get_by_id(&conn, server_id)
}

pub fn delete(app: &tauri::AppHandle, server_id: i64) -> Result<(), String> {
    let mut conn = db::open_connection(app)?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("DB_ERROR: failed to start transaction: {e}"))?;

    let prev_claude_target = mcp_sync::read_target_bytes(app, "claude")?;
    let prev_claude_manifest = mcp_sync::read_manifest_bytes(app, "claude")?;
    let prev_codex_target = mcp_sync::read_target_bytes(app, "codex")?;
    let prev_codex_manifest = mcp_sync::read_manifest_bytes(app, "codex")?;
    let prev_gemini_target = mcp_sync::read_target_bytes(app, "gemini")?;
    let prev_gemini_manifest = mcp_sync::read_manifest_bytes(app, "gemini")?;

    let changed = tx
        .execute("DELETE FROM mcp_servers WHERE id = ?1", params![server_id])
        .map_err(|e| format!("DB_ERROR: failed to delete mcp server: {e}"))?;
    if changed == 0 {
        return Err("DB_NOT_FOUND: mcp server not found".to_string());
    }

    if let Err(err) = sync_all_cli(app, &tx) {
        let _ = mcp_sync::restore_target_bytes(app, "claude", prev_claude_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "claude", prev_claude_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "codex", prev_codex_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "codex", prev_codex_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "gemini", prev_gemini_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "gemini", prev_gemini_manifest);
        return Err(err);
    }

    if let Err(err) = tx.commit() {
        let _ = mcp_sync::restore_target_bytes(app, "claude", prev_claude_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "claude", prev_claude_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "codex", prev_codex_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "codex", prev_codex_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "gemini", prev_gemini_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "gemini", prev_gemini_manifest);
        return Err(format!("DB_ERROR: failed to commit: {err}"));
    }

    Ok(())
}

fn is_code_switch_r_shape(root: &serde_json::Value) -> bool {
    root.get("claude").is_some() || root.get("codex").is_some() || root.get("gemini").is_some()
}

fn suggest_key(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
            continue;
        }

        if lower == '_' || lower == '-' {
            if !out.is_empty() && !prev_dash {
                out.push(lower);
                prev_dash = true;
            }
            continue;
        }

        if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }

    let out = out.trim_matches('-').trim_matches('_').to_string();
    let mut key = if out.is_empty() {
        "mcp-server".to_string()
    } else {
        out
    };
    if !key.chars().next().unwrap_or('a').is_ascii_alphanumeric() {
        key = format!("mcp-{key}");
    }
    if key.len() > 64 {
        key.truncate(64);
    }
    key
}

fn ensure_unique_key(base: &str, used: &mut HashSet<String>) -> String {
    if !used.contains(base) {
        used.insert(base.to_string());
        return base.to_string();
    }

    for idx in 2..1000 {
        let suffix = format!("-{idx}");
        let mut candidate = base.to_string();
        if candidate.len() + suffix.len() > 64 {
            candidate.truncate(64 - suffix.len());
        }
        candidate.push_str(&suffix);
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
    }

    let fallback = format!("mcp-{}", now_unix_seconds());
    used.insert(fallback.clone());
    fallback
}

fn extract_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect()
}

fn extract_string_map(value: Option<&serde_json::Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return out;
    };
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            out.insert(k.to_string(), s.to_string());
        }
    }
    out
}

fn normalize_transport_from_json(spec: &serde_json::Value) -> Option<String> {
    let raw = spec
        .get("type")
        .and_then(|v| v.as_str())
        .or_else(|| spec.get("transport").and_then(|v| v.as_str()))
        .or_else(|| spec.get("transport_type").and_then(|v| v.as_str()));
    let raw = raw?;
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "stdio" => Some("stdio".to_string()),
        "http" => Some("http".to_string()),
        "sse" => Some("http".to_string()),
        _ => None,
    }
}

fn parse_code_switch_r(root: &serde_json::Value) -> Result<Vec<McpImportServer>, String> {
    let mut by_name: HashMap<String, McpImportServer> = HashMap::new();

    for (cli_key, enabled_field) in [
        ("claude", "enabled_claude"),
        ("codex", "enabled_codex"),
        ("gemini", "enabled_gemini"),
    ] {
        let Some(section) = root.get(cli_key) else {
            continue;
        };
        let Some(servers) = section.get("servers").and_then(|v| v.as_object()) else {
            continue;
        };

        for (name, entry) in servers {
            let enabled = entry
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let spec = entry
                .get("server")
                .or_else(|| entry.get("spec"))
                .unwrap_or(entry);

            let transport =
                normalize_transport_from_json(spec).unwrap_or_else(|| "stdio".to_string());

            let command = spec
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let url = spec
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let cwd = spec
                .get("cwd")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let args = extract_string_array(spec.get("args"));
            let env = extract_string_map(spec.get("env"));
            let headers =
                extract_string_map(spec.get("headers").or_else(|| spec.get("http_headers")));

            if transport == "stdio" && command.as_deref().unwrap_or("").trim().is_empty() {
                return Err(format!(
                    "SEC_INVALID_INPUT: import {cli_key} server '{name}' missing command"
                ));
            }
            if transport == "http" && url.as_deref().unwrap_or("").trim().is_empty() {
                return Err(format!(
                    "SEC_INVALID_INPUT: import {cli_key} server '{name}' missing url"
                ));
            }

            let item = by_name
                .entry(name.to_string())
                .or_insert_with(|| McpImportServer {
                    server_key: String::new(),
                    name: name.to_string(),
                    transport: transport.clone(),
                    command: command.clone(),
                    args: args.clone(),
                    env: env.clone(),
                    cwd: cwd.clone(),
                    url: url.clone(),
                    headers: headers.clone(),
                    enabled_claude: false,
                    enabled_codex: false,
                    enabled_gemini: false,
                });

            // If the same server name appears in multiple platform sections, require compatible specs.
            if item.transport != transport
                || item.command != command
                || item.url != url
                || item.args != args
            {
                return Err(format!(
                    "SEC_INVALID_INPUT: import conflict for server '{name}' across platforms"
                ));
            }

            match enabled_field {
                "enabled_claude" => item.enabled_claude = enabled,
                "enabled_codex" => item.enabled_codex = enabled,
                "enabled_gemini" => item.enabled_gemini = enabled,
                _ => {}
            }
        }
    }

    let mut used_keys = HashSet::new();
    let mut out: Vec<McpImportServer> = by_name
        .into_values()
        .map(|mut item| {
            let base = suggest_key(&item.name);
            let key = ensure_unique_key(&base, &mut used_keys);
            item.server_key = key;
            item
        })
        .collect();

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn parse_json(json_text: &str) -> Result<McpParseResult, String> {
    let json_text = json_text.trim();
    if json_text.is_empty() {
        return Err("SEC_INVALID_INPUT: JSON is required".to_string());
    }

    let root: serde_json::Value = serde_json::from_str(json_text)
        .map_err(|e| format!("SEC_INVALID_INPUT: invalid JSON: {e}"))?;

    let servers = if is_code_switch_r_shape(&root) {
        parse_code_switch_r(&root)?
    } else if let Some(arr) = root.as_array() {
        // Optional: support simplified array format used by this project.
        let mut out = Vec::new();
        for item in arr {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.trim().is_empty() {
                continue;
            }
            let base = suggest_key(&name);
            let transport = obj
                .get("transport")
                .and_then(|v| v.as_str())
                .unwrap_or("stdio")
                .trim()
                .to_lowercase();
            let command = obj
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            out.push(McpImportServer {
                server_key: base,
                name,
                transport,
                command,
                args: extract_string_array(obj.get("args")),
                env: extract_string_map(obj.get("env")),
                cwd: obj
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                url,
                headers: extract_string_map(obj.get("headers")),
                enabled_claude: obj
                    .get("enabled_claude")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                enabled_codex: obj
                    .get("enabled_codex")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                enabled_gemini: obj
                    .get("enabled_gemini")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            });
        }
        out
    } else {
        return Err("SEC_INVALID_INPUT: unsupported JSON shape".to_string());
    };

    Ok(McpParseResult { servers })
}

fn upsert_by_name(
    tx: &Connection,
    input: &McpImportServer,
    now: i64,
) -> Result<(bool, i64), String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("SEC_INVALID_INPUT: name is required".to_string());
    }
    let transport = input.transport.trim().to_lowercase();
    validate_transport(&transport)?;

    let command = input
        .command
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let url = input
        .url
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let cwd = input
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    if transport == "stdio" && command.is_none() {
        return Err(format!(
            "SEC_INVALID_INPUT: stdio command is required for server='{}'",
            name
        ));
    }
    if transport == "http" && url.is_none() {
        return Err(format!(
            "SEC_INVALID_INPUT: http url is required for server='{}'",
            name
        ));
    }

    let args_json = args_to_json(&input.args)?;
    let env_json = map_to_json(&input.env, "env")?;
    let headers_json = map_to_json(&input.headers, "headers")?;

    let normalized_name = normalize_name(name);
    let existing_id: Option<i64> = tx
        .query_row(
            r#"
SELECT id
FROM mcp_servers
WHERE normalized_name = ?1
ORDER BY updated_at DESC, id DESC
LIMIT 1
"#,
            params![normalized_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("DB_ERROR: failed to query mcp server by name: {e}"))?;

    match existing_id {
        None => {
            let resolved_key = generate_unique_server_key(tx, name)?;
            tx.execute(
                r#"
INSERT INTO mcp_servers(
  server_key,
  name,
  normalized_name,
  transport,
  command,
  args_json,
  env_json,
  cwd,
  url,
  headers_json,
  enabled_claude,
  enabled_codex,
  enabled_gemini,
  created_at,
  updated_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
"#,
                params![
                    resolved_key,
                    name,
                    normalized_name,
                    transport,
                    command,
                    args_json,
                    env_json,
                    cwd,
                    url,
                    headers_json,
                    enabled_to_int(input.enabled_claude),
                    enabled_to_int(input.enabled_codex),
                    enabled_to_int(input.enabled_gemini),
                    now,
                    now
                ],
            )
            .map_err(|e| format!("DB_ERROR: failed to insert mcp server: {e}"))?;

            Ok((true, tx.last_insert_rowid()))
        }
        Some(id) => {
            tx.execute(
                r#"
UPDATE mcp_servers
SET
  name = ?1,
  normalized_name = ?2,
  transport = ?3,
  command = ?4,
  args_json = ?5,
  env_json = ?6,
  cwd = ?7,
  url = ?8,
  headers_json = ?9,
  enabled_claude = ?10,
  enabled_codex = ?11,
  enabled_gemini = ?12,
  updated_at = ?13
WHERE id = ?14
"#,
                params![
                    name,
                    normalized_name,
                    transport,
                    command,
                    args_json,
                    env_json,
                    cwd,
                    url,
                    headers_json,
                    enabled_to_int(input.enabled_claude),
                    enabled_to_int(input.enabled_codex),
                    enabled_to_int(input.enabled_gemini),
                    now,
                    id
                ],
            )
            .map_err(|e| format!("DB_ERROR: failed to update mcp server: {e}"))?;

            Ok((false, id))
        }
    }
}

pub fn import_servers(
    app: &tauri::AppHandle,
    servers: Vec<McpImportServer>,
) -> Result<McpImportReport, String> {
    if servers.is_empty() {
        return Err("SEC_INVALID_INPUT: servers is required".to_string());
    }

    let mut conn = db::open_connection(app)?;
    let now = now_unix_seconds();

    let tx = conn
        .transaction()
        .map_err(|e| format!("DB_ERROR: failed to start transaction: {e}"))?;

    let prev_claude_target = mcp_sync::read_target_bytes(app, "claude")?;
    let prev_claude_manifest = mcp_sync::read_manifest_bytes(app, "claude")?;
    let prev_codex_target = mcp_sync::read_target_bytes(app, "codex")?;
    let prev_codex_manifest = mcp_sync::read_manifest_bytes(app, "codex")?;
    let prev_gemini_target = mcp_sync::read_target_bytes(app, "gemini")?;
    let prev_gemini_manifest = mcp_sync::read_manifest_bytes(app, "gemini")?;

    let mut inserted = 0u32;
    let mut updated = 0u32;

    let mut deduped: Vec<McpImportServer> = Vec::new();
    let mut index_by_name: HashMap<String, usize> = HashMap::new();
    for server in servers {
        let norm = normalize_name(&server.name);
        if norm.is_empty() {
            return Err("SEC_INVALID_INPUT: name is required".to_string());
        }
        if let Some(idx) = index_by_name.get(&norm).copied() {
            deduped[idx] = server;
            continue;
        }
        index_by_name.insert(norm, deduped.len());
        deduped.push(server);
    }

    for server in &deduped {
        let (is_insert, _id) = upsert_by_name(&tx, server, now)?;
        if is_insert {
            inserted += 1;
        } else {
            updated += 1;
        }
    }

    if let Err(err) = sync_all_cli(app, &tx) {
        let _ = mcp_sync::restore_target_bytes(app, "claude", prev_claude_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "claude", prev_claude_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "codex", prev_codex_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "codex", prev_codex_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "gemini", prev_gemini_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "gemini", prev_gemini_manifest);
        return Err(err);
    }

    if let Err(err) = tx.commit() {
        let _ = mcp_sync::restore_target_bytes(app, "claude", prev_claude_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "claude", prev_claude_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "codex", prev_codex_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "codex", prev_codex_manifest);
        let _ = mcp_sync::restore_target_bytes(app, "gemini", prev_gemini_target);
        let _ = mcp_sync::restore_manifest_bytes(app, "gemini", prev_gemini_manifest);
        return Err(format!("DB_ERROR: failed to commit: {err}"));
    }

    Ok(McpImportReport { inserted, updated })
}

fn validate_cli_key(cli_key: &str) -> Result<(), String> {
    match cli_key {
        "claude" | "codex" | "gemini" => Ok(()),
        _ => Err(format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}")),
    }
}
