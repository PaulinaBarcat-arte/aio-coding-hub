//! Usage: Request logs and trace detail related Tauri commands.

use crate::app_state::{ensure_db_ready, DbInitState};
use crate::{blocking, request_attempt_logs, request_logs};

#[tauri::command]
pub(crate) async fn request_logs_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
    limit: Option<u32>,
) -> Result<Vec<request_logs::RequestLogSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 500) as usize;
    blocking::run("request_logs_list", move || {
        request_logs::list_recent(&app, &cli_key, limit)
    })
    .await
}

#[tauri::command]
pub(crate) async fn request_logs_list_all(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    limit: Option<u32>,
) -> Result<Vec<request_logs::RequestLogSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 500) as usize;
    blocking::run("request_logs_list_all", move || {
        request_logs::list_recent_all(&app, limit)
    })
    .await
}

#[tauri::command]
pub(crate) async fn request_logs_list_after_id(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
    after_id: i64,
    limit: Option<u32>,
) -> Result<Vec<request_logs::RequestLogSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 500) as usize;
    blocking::run("request_logs_list_after_id", move || {
        request_logs::list_after_id(&app, &cli_key, after_id, limit)
    })
    .await
}

#[tauri::command]
pub(crate) async fn request_logs_list_after_id_all(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    after_id: i64,
    limit: Option<u32>,
) -> Result<Vec<request_logs::RequestLogSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 500) as usize;
    blocking::run("request_logs_list_after_id_all", move || {
        request_logs::list_after_id_all(&app, after_id, limit)
    })
    .await
}

#[tauri::command]
pub(crate) async fn request_log_get(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    log_id: i64,
) -> Result<request_logs::RequestLogDetail, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("request_log_get", move || {
        request_logs::get_by_id(&app, log_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn request_log_get_by_trace_id(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    trace_id: String,
) -> Result<Option<request_logs::RequestLogDetail>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("request_log_get_by_trace_id", move || {
        request_logs::get_by_trace_id(&app, &trace_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn request_attempt_logs_by_trace_id(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    trace_id: String,
    limit: Option<u32>,
) -> Result<Vec<request_attempt_logs::RequestAttemptLog>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 200) as usize;
    blocking::run("request_attempt_logs_by_trace_id", move || {
        request_attempt_logs::list_by_trace_id(&app, &trace_id, limit)
    })
    .await
}
