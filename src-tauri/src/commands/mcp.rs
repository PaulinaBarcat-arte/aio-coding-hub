//! Usage: MCP server management related Tauri commands.

use crate::app_state::{ensure_db_ready, DbInitState};
use crate::{blocking, mcp};

#[tauri::command]
pub(crate) async fn mcp_servers_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<Vec<mcp::McpServerSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("mcp_servers_list", move || mcp::list_all(&app)).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn mcp_server_upsert(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    server_id: Option<i64>,
    server_key: String,
    name: String,
    transport: String,
    command: Option<String>,
    args: Vec<String>,
    env: std::collections::BTreeMap<String, String>,
    cwd: Option<String>,
    url: Option<String>,
    headers: std::collections::BTreeMap<String, String>,
    enabled_claude: bool,
    enabled_codex: bool,
    enabled_gemini: bool,
) -> Result<mcp::McpServerSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("mcp_server_upsert", move || {
        mcp::upsert(
            &app,
            server_id,
            &server_key,
            &name,
            &transport,
            command.as_deref(),
            args,
            env,
            cwd.as_deref(),
            url.as_deref(),
            headers,
            enabled_claude,
            enabled_codex,
            enabled_gemini,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn mcp_server_set_enabled(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    server_id: i64,
    cli_key: String,
    enabled: bool,
) -> Result<mcp::McpServerSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("mcp_server_set_enabled", move || {
        mcp::set_enabled(&app, server_id, &cli_key, enabled)
    })
    .await
}

#[tauri::command]
pub(crate) async fn mcp_server_delete(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    server_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("mcp_server_delete", move || {
        mcp::delete(&app, server_id)?;
        Ok(true)
    })
    .await
}

#[tauri::command]
pub(crate) fn mcp_parse_json(json_text: String) -> Result<mcp::McpParseResult, String> {
    mcp::parse_json(&json_text)
}

#[tauri::command]
pub(crate) async fn mcp_import_servers(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    servers: Vec<mcp::McpImportServer>,
) -> Result<mcp::McpImportReport, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("mcp_import_servers", move || {
        mcp::import_servers(&app, servers)
    })
    .await
}
