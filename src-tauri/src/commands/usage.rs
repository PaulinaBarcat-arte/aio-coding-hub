//! Usage: Usage statistics related Tauri commands.

use crate::app_state::{ensure_db_ready, DbInitState};
use crate::{blocking, usage_stats};

#[tauri::command]
pub(crate) async fn usage_summary(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    range: String,
    cli_key: Option<String>,
) -> Result<usage_stats::UsageSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("usage_summary", move || {
        usage_stats::summary(&app, &range, cli_key.as_deref())
    })
    .await
}

#[tauri::command]
pub(crate) async fn usage_summary_v2(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
) -> Result<usage_stats::UsageSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("usage_summary_v2", move || {
        usage_stats::summary_v2(&app, &period, start_ts, end_ts, cli_key.as_deref())
    })
    .await
}

#[tauri::command]
pub(crate) async fn usage_leaderboard_provider(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    range: String,
    cli_key: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<usage_stats::UsageProviderRow>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(10).clamp(1, 50) as usize;
    blocking::run("usage_leaderboard_provider", move || {
        usage_stats::leaderboard_provider(&app, &range, cli_key.as_deref(), limit)
    })
    .await
}

#[tauri::command]
pub(crate) async fn usage_leaderboard_day(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    range: String,
    cli_key: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<usage_stats::UsageDayRow>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(10).clamp(1, 50) as usize;
    blocking::run("usage_leaderboard_day", move || {
        usage_stats::leaderboard_day(&app, &range, cli_key.as_deref(), limit)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn usage_leaderboard_v2(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    scope: String,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<usage_stats::UsageLeaderboardRow>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(25).clamp(1, 200) as usize;
    blocking::run("usage_leaderboard_v2", move || {
        usage_stats::leaderboard_v2(
            &app,
            &scope,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            limit,
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn usage_hourly_series(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    days: u32,
) -> Result<Vec<usage_stats::UsageHourlyRow>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let days = days.clamp(1, 60);
    blocking::run("usage_hourly_series", move || {
        usage_stats::hourly_series(&app, days)
    })
    .await
}
