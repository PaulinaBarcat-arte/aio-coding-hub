//! Usage: Prompt templates related Tauri commands.

use crate::app_state::{ensure_db_ready, DbInitState};
use crate::{blocking, prompts};

#[tauri::command]
pub(crate) async fn prompts_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
) -> Result<Vec<prompts::PromptSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("prompts_list", move || prompts::list_by_cli(&app, &cli_key)).await
}

#[tauri::command]
pub(crate) async fn prompts_default_sync_from_files(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<prompts::DefaultPromptSyncReport, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("prompts_default_sync_from_files", move || {
        prompts::default_sync_from_files(&app)
    })
    .await
}

#[tauri::command]
pub(crate) async fn prompt_upsert(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    prompt_id: Option<i64>,
    cli_key: String,
    name: String,
    content: String,
    enabled: bool,
) -> Result<prompts::PromptSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("prompt_upsert", move || {
        prompts::upsert(&app, prompt_id, &cli_key, &name, &content, enabled)
    })
    .await
}

#[tauri::command]
pub(crate) async fn prompt_set_enabled(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    prompt_id: i64,
    enabled: bool,
) -> Result<prompts::PromptSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("prompt_set_enabled", move || {
        prompts::set_enabled(&app, prompt_id, enabled)
    })
    .await
}

#[tauri::command]
pub(crate) async fn prompt_delete(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    prompt_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("prompt_delete", move || {
        prompts::delete(&app, prompt_id)?;
        Ok(true)
    })
    .await
}
