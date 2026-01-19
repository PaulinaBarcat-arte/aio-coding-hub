//! Usage: Skills management related Tauri commands.

use crate::app_state::{ensure_db_ready, DbInitState};
use crate::{blocking, skills};

#[tauri::command]
pub(crate) async fn skill_repos_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<Vec<skills::SkillRepoSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("skill_repos_list", move || skills::repos_list(&app)).await
}

#[tauri::command]
pub(crate) async fn skill_repo_upsert(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    repo_id: Option<i64>,
    git_url: String,
    branch: String,
    enabled: bool,
) -> Result<skills::SkillRepoSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("skill_repo_upsert", move || {
        skills::repo_upsert(&app, repo_id, &git_url, &branch, enabled)
    })
    .await
}

#[tauri::command]
pub(crate) async fn skill_repo_delete(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    repo_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("skill_repo_delete", move || {
        skills::repo_delete(&app, repo_id)?;
        Ok(true)
    })
    .await
}

#[tauri::command]
pub(crate) async fn skills_installed_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<Vec<skills::InstalledSkillSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("skills_installed_list", move || {
        skills::installed_list(&app)
    })
    .await
}

#[tauri::command]
pub(crate) async fn skills_discover_available(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    refresh: bool,
) -> Result<Vec<skills::AvailableSkillSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    tauri::async_runtime::spawn_blocking(move || skills::discover_available(&app, refresh))
        .await
        .map_err(|e| format!("SKILL_TASK_JOIN: {e}"))?
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_install(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    git_url: String,
    branch: String,
    source_subdir: String,
    enabled_claude: bool,
    enabled_codex: bool,
    enabled_gemini: bool,
) -> Result<skills::InstalledSkillSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    tauri::async_runtime::spawn_blocking(move || {
        skills::install(
            &app,
            &git_url,
            &branch,
            &source_subdir,
            enabled_claude,
            enabled_codex,
            enabled_gemini,
        )
    })
    .await
    .map_err(|e| format!("SKILL_TASK_JOIN: {e}"))?
}

#[tauri::command]
pub(crate) async fn skill_set_enabled(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    skill_id: i64,
    cli_key: String,
    enabled: bool,
) -> Result<skills::InstalledSkillSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    tauri::async_runtime::spawn_blocking(move || {
        skills::set_enabled(&app, skill_id, &cli_key, enabled)
    })
    .await
    .map_err(|e| format!("SKILL_TASK_JOIN: {e}"))?
}

#[tauri::command]
pub(crate) async fn skill_uninstall(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    skill_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    tauri::async_runtime::spawn_blocking(move || skills::uninstall(&app, skill_id))
        .await
        .map_err(|e| format!("SKILL_TASK_JOIN: {e}"))??;
    Ok(true)
}

#[tauri::command]
pub(crate) async fn skills_local_list(
    app: tauri::AppHandle,
    cli_key: String,
) -> Result<Vec<skills::LocalSkillSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || skills::local_list(&app, &cli_key))
        .await
        .map_err(|e| format!("SKILL_TASK_JOIN: {e}"))?
}

#[tauri::command]
pub(crate) async fn skill_import_local(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
    dir_name: String,
) -> Result<skills::InstalledSkillSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    tauri::async_runtime::spawn_blocking(move || skills::import_local(&app, &cli_key, &dir_name))
        .await
        .map_err(|e| format!("SKILL_TASK_JOIN: {e}"))?
}

#[tauri::command]
pub(crate) async fn skills_paths_get(
    app: tauri::AppHandle,
    cli_key: String,
) -> Result<skills::SkillsPaths, String> {
    blocking::run("skills_paths_get", move || {
        skills::paths_get(&app, &cli_key)
    })
    .await
}
