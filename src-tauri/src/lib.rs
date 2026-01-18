mod app_paths;
mod base_url_probe;
mod blocking;
mod circuit_breaker;
mod claude_model_validation;
mod claude_model_validation_history;
mod cli_manager;
mod cli_proxy;
mod cost;
mod cost_stats;
mod data_management;
mod db;
mod gateway;
mod mcp;
mod mcp_sync;
mod model_prices;
mod model_prices_sync;
mod notice;
mod prompt_sync;
mod prompts;
mod provider_circuit_breakers;
mod providers;
mod request_attempt_logs;
mod request_logs;
mod resident;
mod session_manager;
mod settings;
mod skills;
mod sort_modes;
mod usage;
mod usage_stats;

use std::sync::Mutex;
use tauri::utils::config::BundleType;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::OnceCell;

#[derive(Debug, Clone, serde::Serialize)]
struct AppAboutInfo {
    os: String,
    arch: String,
    profile: String,
    app_version: String,
    bundle_type: Option<String>,
    run_mode: String,
}

#[derive(Default)]
struct GatewayState(Mutex<gateway::GatewayManager>);

#[derive(Default)]
struct DbInitState(OnceCell<Result<(), String>>);

async fn ensure_db_ready(app: tauri::AppHandle, state: &DbInitState) -> Result<(), String> {
    state
        .0
        .get_or_init(|| async move { blocking::run("db_init", move || db::init(&app)).await })
        .await
        .clone()
}

#[derive(Debug, Clone, serde::Serialize)]
struct GatewayActiveSessionSummary {
    cli_key: String,
    session_id: String,
    session_suffix: String,
    provider_id: i64,
    provider_name: String,
    expires_at: i64,
    request_count: Option<i64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    total_cost_usd: Option<f64>,
    total_duration_ms: Option<i64>,
}

#[tauri::command]
async fn settings_get(app: tauri::AppHandle) -> Result<settings::AppSettings, String> {
    blocking::run("settings_get", move || settings::read(&app)).await
}

#[tauri::command]
fn app_about_get() -> AppAboutInfo {
    let bundle_type = tauri::utils::platform::bundle_type();
    let run_mode = match bundle_type {
        Some(BundleType::Nsis | BundleType::Msi | BundleType::Deb | BundleType::Rpm) => "installer",
        Some(BundleType::AppImage) => "portable",
        Some(BundleType::App | BundleType::Dmg) => "unknown",
        None => "unknown",
    }
    .to_string();

    AppAboutInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        profile: if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "release".to_string()
        },
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        bundle_type: bundle_type.map(|t| t.to_string()),
        run_mode,
    }
}

#[tauri::command]
fn notice_send(
    app: tauri::AppHandle,
    level: notice::NoticeLevel,
    title: Option<String>,
    body: String,
) -> Result<bool, String> {
    notice::emit(&app, notice::build(level, title, body))?;
    Ok(true)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn settings_set(
    app: tauri::AppHandle,
    preferred_port: u16,
    auto_start: bool,
    tray_enabled: Option<bool>,
    log_retention_days: u32,
    provider_cooldown_seconds: Option<u32>,
    provider_base_url_ping_cache_ttl_seconds: Option<u32>,
    upstream_first_byte_timeout_seconds: Option<u32>,
    upstream_stream_idle_timeout_seconds: Option<u32>,
    upstream_request_timeout_non_streaming_seconds: Option<u32>,
    intercept_anthropic_warmup_requests: Option<bool>,
    enable_thinking_signature_rectifier: Option<bool>,
    enable_response_fixer: Option<bool>,
    response_fixer_fix_encoding: Option<bool>,
    response_fixer_fix_sse_format: Option<bool>,
    response_fixer_fix_truncated_json: Option<bool>,
    failover_max_attempts_per_provider: u32,
    failover_max_providers_to_try: u32,
    circuit_breaker_failure_threshold: Option<u32>,
    circuit_breaker_open_duration_minutes: Option<u32>,
    update_releases_url: Option<String>,
) -> Result<settings::AppSettings, String> {
    let app_for_work = app.clone();
    let next_settings = blocking::run("settings_set", move || {
        let previous = settings::read(&app_for_work).unwrap_or_default();
        let update_releases_url = update_releases_url.unwrap_or(previous.update_releases_url);
        let tray_enabled = tray_enabled.unwrap_or(previous.tray_enabled);
        let provider_cooldown_seconds =
            provider_cooldown_seconds.unwrap_or(previous.provider_cooldown_seconds);
        let provider_base_url_ping_cache_ttl_seconds = provider_base_url_ping_cache_ttl_seconds
            .unwrap_or(previous.provider_base_url_ping_cache_ttl_seconds);
        let upstream_first_byte_timeout_seconds = upstream_first_byte_timeout_seconds
            .unwrap_or(previous.upstream_first_byte_timeout_seconds);
        let upstream_stream_idle_timeout_seconds = upstream_stream_idle_timeout_seconds
            .unwrap_or(previous.upstream_stream_idle_timeout_seconds);
        let upstream_request_timeout_non_streaming_seconds =
            upstream_request_timeout_non_streaming_seconds
                .unwrap_or(previous.upstream_request_timeout_non_streaming_seconds);
        let intercept_anthropic_warmup_requests = intercept_anthropic_warmup_requests
            .unwrap_or(previous.intercept_anthropic_warmup_requests);
        let enable_thinking_signature_rectifier = enable_thinking_signature_rectifier
            .unwrap_or(previous.enable_thinking_signature_rectifier);
        let enable_response_fixer = enable_response_fixer.unwrap_or(previous.enable_response_fixer);
        let response_fixer_fix_encoding =
            response_fixer_fix_encoding.unwrap_or(previous.response_fixer_fix_encoding);
        let response_fixer_fix_sse_format =
            response_fixer_fix_sse_format.unwrap_or(previous.response_fixer_fix_sse_format);
        let response_fixer_fix_truncated_json =
            response_fixer_fix_truncated_json.unwrap_or(previous.response_fixer_fix_truncated_json);
        let circuit_breaker_failure_threshold =
            circuit_breaker_failure_threshold.unwrap_or(previous.circuit_breaker_failure_threshold);
        let circuit_breaker_open_duration_minutes = circuit_breaker_open_duration_minutes
            .unwrap_or(previous.circuit_breaker_open_duration_minutes);
        let mut next_auto_start = auto_start;

        #[cfg(desktop)]
        {
            if auto_start != previous.auto_start {
                use tauri_plugin_autostart::ManagerExt;

                let result = if auto_start {
                    app_for_work
                        .autolaunch()
                        .enable()
                        .map_err(|e| format!("failed to enable autostart: {e}"))
                } else {
                    app_for_work
                        .autolaunch()
                        .disable()
                        .map_err(|e| format!("failed to disable autostart: {e}"))
                };

                if let Err(err) = result {
                    eprintln!("autostart sync error: {err}");
                    next_auto_start = previous.auto_start;
                }
            }
        }

        let settings = settings::AppSettings {
            schema_version: settings::SCHEMA_VERSION,
            preferred_port,
            auto_start: next_auto_start,
            tray_enabled,
            log_retention_days,
            provider_cooldown_seconds,
            provider_base_url_ping_cache_ttl_seconds,
            upstream_first_byte_timeout_seconds,
            upstream_stream_idle_timeout_seconds,
            upstream_request_timeout_non_streaming_seconds,
            update_releases_url,
            failover_max_attempts_per_provider,
            failover_max_providers_to_try,
            circuit_breaker_failure_threshold,
            circuit_breaker_open_duration_minutes,
            enable_circuit_breaker_notice: previous.enable_circuit_breaker_notice,
            intercept_anthropic_warmup_requests,
            enable_thinking_signature_rectifier,
            enable_codex_session_id_completion: previous.enable_codex_session_id_completion,
            enable_response_fixer,
            response_fixer_fix_encoding,
            response_fixer_fix_sse_format,
            response_fixer_fix_truncated_json,
        };

        let next_settings = settings::write(&app_for_work, &settings)?;
        Ok(next_settings)
    })
    .await?;

    app.state::<resident::ResidentState>()
        .set_tray_enabled(next_settings.tray_enabled);
    Ok(next_settings)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn settings_gateway_rectifier_set(
    app: tauri::AppHandle,
    intercept_anthropic_warmup_requests: bool,
    enable_thinking_signature_rectifier: bool,
    enable_response_fixer: bool,
    response_fixer_fix_encoding: bool,
    response_fixer_fix_sse_format: bool,
    response_fixer_fix_truncated_json: bool,
) -> Result<settings::AppSettings, String> {
    let app_for_work = app.clone();
    blocking::run("settings_gateway_rectifier_set", move || {
        let mut settings = settings::read(&app_for_work).unwrap_or_default();
        settings.schema_version = settings::SCHEMA_VERSION;

        settings.intercept_anthropic_warmup_requests = intercept_anthropic_warmup_requests;
        settings.enable_thinking_signature_rectifier = enable_thinking_signature_rectifier;
        settings.enable_response_fixer = enable_response_fixer;
        settings.response_fixer_fix_encoding = response_fixer_fix_encoding;
        settings.response_fixer_fix_sse_format = response_fixer_fix_sse_format;
        settings.response_fixer_fix_truncated_json = response_fixer_fix_truncated_json;

        settings::write(&app_for_work, &settings)
    })
    .await
}

#[tauri::command]
async fn settings_circuit_breaker_notice_set(
    app: tauri::AppHandle,
    enable_circuit_breaker_notice: bool,
) -> Result<settings::AppSettings, String> {
    let app_for_work = app.clone();
    blocking::run("settings_circuit_breaker_notice_set", move || {
        let mut settings = settings::read(&app_for_work).unwrap_or_default();
        settings.schema_version = settings::SCHEMA_VERSION;
        settings.enable_circuit_breaker_notice = enable_circuit_breaker_notice;
        settings::write(&app_for_work, &settings)
    })
    .await
}

#[tauri::command]
async fn settings_codex_session_id_completion_set(
    app: tauri::AppHandle,
    enable_codex_session_id_completion: bool,
) -> Result<settings::AppSettings, String> {
    let app_for_work = app.clone();
    blocking::run("settings_codex_session_id_completion_set", move || {
        let mut settings = settings::read(&app_for_work).unwrap_or_default();
        settings.schema_version = settings::SCHEMA_VERSION;
        settings.enable_codex_session_id_completion = enable_codex_session_id_completion;
        settings::write(&app_for_work, &settings)
    })
    .await
}

#[tauri::command]
async fn cli_manager_claude_info_get(
    app: tauri::AppHandle,
) -> Result<cli_manager::ClaudeCliInfo, String> {
    blocking::run("cli_manager_claude_info_get", move || {
        cli_manager::claude_info_get(&app)
    })
    .await
}

#[tauri::command]
async fn cli_manager_codex_info_get(
    app: tauri::AppHandle,
) -> Result<cli_manager::SimpleCliInfo, String> {
    blocking::run("cli_manager_codex_info_get", move || {
        cli_manager::codex_info_get(&app)
    })
    .await
}

#[tauri::command]
async fn cli_manager_gemini_info_get(
    app: tauri::AppHandle,
) -> Result<cli_manager::SimpleCliInfo, String> {
    blocking::run("cli_manager_gemini_info_get", move || {
        cli_manager::gemini_info_get(&app)
    })
    .await
}

#[tauri::command]
async fn cli_manager_claude_env_set(
    app: tauri::AppHandle,
    mcp_timeout_ms: Option<u64>,
    disable_error_reporting: bool,
) -> Result<cli_manager::ClaudeEnvState, String> {
    blocking::run("cli_manager_claude_env_set", move || {
        cli_manager::claude_env_set(&app, mcp_timeout_ms, disable_error_reporting)
    })
    .await
}

#[tauri::command]
fn gateway_status(state: tauri::State<'_, GatewayState>) -> gateway::GatewayStatus {
    let manager = state.0.lock().unwrap_or_else(|e| e.into_inner());
    manager.status()
}

#[tauri::command]
fn gateway_check_port_available(port: u16) -> bool {
    if port < 1024 {
        return false;
    }
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

#[tauri::command]
async fn gateway_sessions_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    state: tauri::State<'_, GatewayState>,
    limit: Option<u32>,
) -> Result<Vec<GatewayActiveSessionSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;

    let limit = limit.unwrap_or(50).min(200) as usize;
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let sessions = {
        let manager = state.0.lock().map_err(|_| "gateway state poisoned")?;
        manager.active_sessions(now_unix, limit)
    };

    if sessions.is_empty() {
        return Ok(Vec::new());
    }

    let provider_ids: Vec<i64> = sessions.iter().map(|s| s.provider_id).collect();
    let session_ids: Vec<String> = sessions.iter().map(|s| s.session_id.clone()).collect();

    let app_for_names = app.clone();
    let provider_names = blocking::run("providers_names_by_id", move || {
        providers::names_by_id(&app_for_names, &provider_ids)
    })
    .await?;

    let app_for_agg = app.clone();
    let session_stats = blocking::run("request_logs_aggregate_by_session_ids", move || {
        request_logs::aggregate_by_session_ids(&app_for_agg, &session_ids)
    })
    .await?;

    Ok(sessions
        .into_iter()
        .map(|s| {
            let cli_key = s.cli_key;
            let session_id = s.session_id;
            let session_suffix = s.session_suffix;
            let provider_id = s.provider_id;
            let expires_at = s.expires_at;

            let provider_name = provider_names
                .get(&provider_id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());

            let stats = session_stats.get(&(cli_key.clone(), session_id.clone()));

            GatewayActiveSessionSummary {
                cli_key,
                session_id,
                session_suffix,
                provider_id,
                provider_name,
                expires_at,
                request_count: stats.map(|row| row.request_count).filter(|v| *v > 0),
                total_input_tokens: stats.map(|row| row.total_input_tokens).filter(|v| *v > 0),
                total_output_tokens: stats.map(|row| row.total_output_tokens).filter(|v| *v > 0),
                total_cost_usd: stats
                    .map(|row| row.total_cost_usd_femto)
                    .filter(|v| *v > 0)
                    .map(|v| v as f64 / 1_000_000_000_000_000.0),
                total_duration_ms: stats.map(|row| row.total_duration_ms).filter(|v| *v > 0),
            }
        })
        .collect())
}

#[tauri::command]
async fn gateway_circuit_status(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
) -> Result<Vec<gateway::GatewayProviderCircuitStatus>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("gateway_circuit_status", move || {
        let state = app.state::<GatewayState>();
        let manager = state.0.lock().map_err(|_| "gateway state poisoned")?;
        manager.circuit_status(&app, &cli_key)
    })
    .await
}

#[tauri::command]
async fn gateway_circuit_reset_provider(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("gateway_circuit_reset_provider", move || {
        let state = app.state::<GatewayState>();
        let manager = state.0.lock().map_err(|_| "gateway state poisoned")?;
        manager.circuit_reset_provider(&app, provider_id)?;
        Ok(true)
    })
    .await
}

#[tauri::command]
async fn gateway_circuit_reset_cli(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
) -> Result<usize, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("gateway_circuit_reset_cli", move || {
        let state = app.state::<GatewayState>();
        let manager = state.0.lock().map_err(|_| "gateway state poisoned")?;
        manager.circuit_reset_cli(&app, &cli_key)
    })
    .await
}

#[tauri::command]
async fn gateway_start(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    preferred_port: Option<u16>,
) -> Result<gateway::GatewayStatus, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let status = blocking::run("gateway_start", {
        let app = app.clone();
        move || {
            let state = app.state::<GatewayState>();
            let mut manager = state.0.lock().map_err(|_| "gateway state poisoned")?;
            manager.start(&app, preferred_port)
        }
    })
    .await?;

    let _ = app.emit("gateway:status", status.clone());
    if let Some(base_origin) = status.base_url.as_deref() {
        // Best-effort: if any CLI proxy is enabled, keep its config aligned with the actual gateway port.
        let app_for_sync = app.clone();
        let base_origin = base_origin.to_string();
        let _ = blocking::run("cli_proxy_sync_enabled_after_gateway_start", move || {
            cli_proxy::sync_enabled(&app_for_sync, &base_origin)
        })
        .await;
    }
    Ok(status)
}

#[tauri::command]
async fn gateway_stop(
    app: tauri::AppHandle,
    state: tauri::State<'_, GatewayState>,
) -> Result<gateway::GatewayStatus, String> {
    let running = {
        let mut manager = state.0.lock().map_err(|_| "gateway state poisoned")?;
        manager.take_running()
    };

    if let Some((shutdown, mut task, mut log_task, mut attempt_log_task, mut circuit_task)) =
        running
    {
        let _ = shutdown.send(());

        let stop_timeout = std::time::Duration::from_secs(3);
        let join_all = async {
            let _ = tokio::join!(
                &mut task,
                &mut log_task,
                &mut attempt_log_task,
                &mut circuit_task
            );
        };

        if tokio::time::timeout(stop_timeout, join_all).await.is_err() {
            eprintln!("gateway stop timeout; aborting server task");
            task.abort();

            let abort_grace = std::time::Duration::from_secs(1);
            let _ = tokio::time::timeout(abort_grace, async {
                let _ = tokio::join!(
                    &mut task,
                    &mut log_task,
                    &mut attempt_log_task,
                    &mut circuit_task
                );
            })
            .await;
        }
    }

    let status = gateway_status(state);
    let _ = app.emit("gateway:status", status.clone());
    Ok(status)
}

#[tauri::command]
async fn providers_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
) -> Result<Vec<providers::ProviderSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("providers_list", move || {
        providers::list_by_cli(&app, &cli_key)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn provider_upsert(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: Option<i64>,
    cli_key: String,
    name: String,
    base_urls: Vec<String>,
    base_url_mode: String,
    api_key: Option<String>,
    enabled: bool,
    cost_multiplier: f64,
    priority: Option<i64>,
) -> Result<providers::ProviderSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("provider_upsert", move || {
        providers::upsert(
            &app,
            provider_id,
            &cli_key,
            &name,
            base_urls,
            &base_url_mode,
            api_key.as_deref(),
            enabled,
            cost_multiplier,
            priority,
        )
    })
    .await
}

#[tauri::command]
async fn provider_set_enabled(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: i64,
    enabled: bool,
) -> Result<providers::ProviderSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("provider_set_enabled", move || {
        providers::set_enabled(&app, provider_id, enabled)
    })
    .await
}

#[tauri::command]
async fn provider_delete(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("provider_delete", move || {
        providers::delete(&app, provider_id)?;
        Ok(true)
    })
    .await
}

#[tauri::command]
async fn providers_reorder(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
    ordered_provider_ids: Vec<i64>,
) -> Result<Vec<providers::ProviderSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("providers_reorder", move || {
        providers::reorder(&app, &cli_key, ordered_provider_ids)
    })
    .await
}

#[tauri::command]
async fn base_url_ping_ms(base_url: String) -> Result<u64, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("aio-coding-hub-ping/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("PING_HTTP_CLIENT_INIT: {e}"))?;
    base_url_probe::probe_base_url_ms(&client, &base_url, std::time::Duration::from_secs(3)).await
}

#[tauri::command]
async fn claude_provider_validate_model(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: i64,
    base_url: String,
    request_json: String,
) -> Result<claude_model_validation::ClaudeModelValidationResult, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    claude_model_validation::validate_provider_model(&app, provider_id, &base_url, &request_json)
        .await
}

#[tauri::command]
async fn claude_provider_get_api_key_plaintext(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: i64,
) -> Result<String, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    claude_model_validation::get_provider_api_key_plaintext(&app, provider_id).await
}

#[tauri::command]
async fn claude_validation_history_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: i64,
    limit: Option<u32>,
) -> Result<Vec<claude_model_validation_history::ClaudeModelValidationRunRow>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 500) as usize;
    blocking::run("claude_validation_history_list", move || {
        claude_model_validation_history::list_runs(&app, provider_id, Some(limit))
    })
    .await
}

#[tauri::command]
async fn claude_validation_history_clear_provider(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    provider_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("claude_validation_history_clear_provider", move || {
        claude_model_validation_history::clear_provider(&app, provider_id)
    })
    .await
}

#[tauri::command]
async fn sort_modes_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<Vec<sort_modes::SortModeSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_modes_list", move || sort_modes::list_modes(&app)).await
}

#[tauri::command]
async fn sort_mode_create(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    name: String,
) -> Result<sort_modes::SortModeSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_mode_create", move || {
        sort_modes::create_mode(&app, &name)
    })
    .await
}

#[tauri::command]
async fn sort_mode_rename(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    mode_id: i64,
    name: String,
) -> Result<sort_modes::SortModeSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_mode_rename", move || {
        sort_modes::rename_mode(&app, mode_id, &name)
    })
    .await
}

#[tauri::command]
async fn sort_mode_delete(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    mode_id: i64,
) -> Result<bool, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_mode_delete", move || {
        sort_modes::delete_mode(&app, mode_id)?;
        Ok(true)
    })
    .await
}

#[tauri::command]
async fn sort_mode_active_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<Vec<sort_modes::SortModeActiveRow>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_mode_active_list", move || {
        sort_modes::list_active(&app)
    })
    .await
}

#[tauri::command]
async fn sort_mode_active_set(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
    mode_id: Option<i64>,
) -> Result<sort_modes::SortModeActiveRow, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_mode_active_set", move || {
        sort_modes::set_active(&app, &cli_key, mode_id)
    })
    .await
}

#[tauri::command]
async fn sort_mode_providers_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    mode_id: i64,
    cli_key: String,
) -> Result<Vec<i64>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_mode_providers_list", move || {
        sort_modes::list_mode_providers(&app, mode_id, &cli_key)
    })
    .await
}

#[tauri::command]
async fn sort_mode_providers_set_order(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    mode_id: i64,
    cli_key: String,
    ordered_provider_ids: Vec<i64>,
) -> Result<Vec<i64>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("sort_mode_providers_set_order", move || {
        sort_modes::set_mode_providers_order(&app, mode_id, &cli_key, ordered_provider_ids)
    })
    .await
}

#[tauri::command]
async fn model_prices_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
) -> Result<Vec<model_prices::ModelPriceSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("model_prices_list", move || {
        model_prices::list_by_cli(&app, &cli_key)
    })
    .await
}

#[tauri::command]
async fn model_price_upsert(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
    model: String,
    price_json: String,
) -> Result<model_prices::ModelPriceSummary, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("model_price_upsert", move || {
        model_prices::upsert(&app, &cli_key, &model, &price_json)
    })
    .await
}

#[tauri::command]
async fn model_prices_sync_basellm(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    force: Option<bool>,
) -> Result<model_prices_sync::ModelPricesSyncReport, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    model_prices_sync::sync_basellm(&app, force.unwrap_or(false)).await
}

#[tauri::command]
async fn prompts_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
) -> Result<Vec<prompts::PromptSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("prompts_list", move || prompts::list_by_cli(&app, &cli_key)).await
}

#[tauri::command]
async fn prompts_default_sync_from_files(
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
async fn prompt_upsert(
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
async fn prompt_set_enabled(
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
async fn prompt_delete(
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

#[tauri::command]
async fn mcp_servers_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<Vec<mcp::McpServerSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("mcp_servers_list", move || mcp::list_all(&app)).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn mcp_server_upsert(
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
async fn mcp_server_set_enabled(
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
async fn mcp_server_delete(
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
fn mcp_parse_json(json_text: String) -> Result<mcp::McpParseResult, String> {
    mcp::parse_json(&json_text)
}

#[tauri::command]
async fn mcp_import_servers(
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

#[tauri::command]
async fn skill_repos_list(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<Vec<skills::SkillRepoSummary>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("skill_repos_list", move || skills::repos_list(&app)).await
}

#[tauri::command]
async fn skill_repo_upsert(
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
async fn skill_repo_delete(
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
async fn skills_installed_list(
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
async fn skills_discover_available(
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
async fn skill_install(
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
async fn skill_set_enabled(
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
async fn skill_uninstall(
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
async fn skills_local_list(
    app: tauri::AppHandle,
    cli_key: String,
) -> Result<Vec<skills::LocalSkillSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || skills::local_list(&app, &cli_key))
        .await
        .map_err(|e| format!("SKILL_TASK_JOIN: {e}"))?
}

#[tauri::command]
async fn skill_import_local(
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
async fn skills_paths_get(
    app: tauri::AppHandle,
    cli_key: String,
) -> Result<skills::SkillsPaths, String> {
    blocking::run("skills_paths_get", move || {
        skills::paths_get(&app, &cli_key)
    })
    .await
}

#[tauri::command]
async fn app_data_dir_get(app: tauri::AppHandle) -> Result<String, String> {
    blocking::run("app_data_dir_get", move || {
        let dir = app_paths::app_data_dir(&app)?;
        Ok(dir.to_string_lossy().to_string())
    })
    .await
}

#[tauri::command]
async fn db_disk_usage_get(app: tauri::AppHandle) -> Result<data_management::DbDiskUsage, String> {
    blocking::run("db_disk_usage_get", move || {
        data_management::db_disk_usage_get(&app)
    })
    .await
}

#[tauri::command]
async fn request_logs_clear_all(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
) -> Result<data_management::ClearRequestLogsResult, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("request_logs_clear_all", move || {
        data_management::request_logs_clear_all(&app)
    })
    .await
}

#[tauri::command]
async fn app_data_reset(
    app: tauri::AppHandle,
    state: tauri::State<'_, GatewayState>,
) -> Result<bool, String> {
    // Best-effort: stop gateway first to avoid concurrent writes locking sqlite files.
    let _ = gateway_stop(app.clone(), state).await;
    blocking::run("app_data_reset", move || {
        data_management::app_data_reset(&app)
    })
    .await
}

#[tauri::command]
fn app_exit(app: tauri::AppHandle) -> Result<bool, String> {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        app.exit(0);
    });
    Ok(true)
}

#[tauri::command]
fn app_restart(app: tauri::AppHandle) -> Result<bool, String> {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        app.request_restart();
    });
    Ok(true)
}

#[tauri::command]
async fn request_logs_list(
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
async fn request_logs_list_all(
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
async fn request_logs_list_after_id(
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
async fn request_logs_list_after_id_all(
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
async fn request_log_get(
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
async fn request_log_get_by_trace_id(
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
async fn request_attempt_logs_by_trace_id(
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

#[tauri::command]
async fn usage_summary(
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
async fn usage_summary_v2(
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
async fn usage_leaderboard_provider(
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
async fn usage_leaderboard_day(
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
async fn usage_leaderboard_v2(
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
async fn usage_hourly_series(
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

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn cost_summary_v1(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    provider_id: Option<i64>,
    model: Option<String>,
) -> Result<cost_stats::CostSummaryV1, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("cost_summary_v1", move || {
        cost_stats::summary_v1(
            &app,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            provider_id,
            model.as_deref(),
        )
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn cost_trend_v1(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    provider_id: Option<i64>,
    model: Option<String>,
) -> Result<Vec<cost_stats::CostTrendRowV1>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    blocking::run("cost_trend_v1", move || {
        cost_stats::trend_v1(
            &app,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            provider_id,
            model.as_deref(),
        )
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn cost_breakdown_provider_v1(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    provider_id: Option<i64>,
    model: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<cost_stats::CostProviderBreakdownRowV1>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 200) as usize;
    blocking::run("cost_breakdown_provider_v1", move || {
        cost_stats::breakdown_provider_v1(
            &app,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            provider_id,
            model.as_deref(),
            limit,
        )
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn cost_breakdown_model_v1(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    provider_id: Option<i64>,
    model: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<cost_stats::CostModelBreakdownRowV1>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 200) as usize;
    blocking::run("cost_breakdown_model_v1", move || {
        cost_stats::breakdown_model_v1(
            &app,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            provider_id,
            model.as_deref(),
            limit,
        )
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn cost_scatter_cli_provider_model_v1(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    provider_id: Option<i64>,
    model: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<cost_stats::CostScatterCliProviderModelRowV1>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(500).clamp(1, 5000) as usize;
    blocking::run("cost_scatter_cli_provider_model_v1", move || {
        cost_stats::scatter_cli_provider_model_v1(
            &app,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            provider_id,
            model.as_deref(),
            limit,
        )
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn cost_top_requests_v1(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    provider_id: Option<i64>,
    model: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<cost_stats::CostTopRequestRowV1>, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let limit = limit.unwrap_or(50).clamp(1, 200) as usize;
    blocking::run("cost_top_requests_v1", move || {
        cost_stats::top_requests_v1(
            &app,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            provider_id,
            model.as_deref(),
            limit,
        )
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn cost_backfill_missing_v1(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    period: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    cli_key: Option<String>,
    provider_id: Option<i64>,
    model: Option<String>,
    max_rows: Option<u32>,
) -> Result<cost_stats::CostBackfillReportV1, String> {
    ensure_db_ready(app.clone(), db_state.inner()).await?;
    let max_rows = max_rows.unwrap_or(5000).clamp(1, 10_000) as usize;
    blocking::run("cost_backfill_missing_v1", move || {
        cost_stats::backfill_missing_v1(
            &app,
            &period,
            start_ts,
            end_ts,
            cli_key.as_deref(),
            provider_id,
            model.as_deref(),
            max_rows,
        )
    })
    .await
}

#[tauri::command]
async fn cli_proxy_status_all(
    app: tauri::AppHandle,
) -> Result<Vec<cli_proxy::CliProxyStatus>, String> {
    blocking::run("cli_proxy_status_all", move || cli_proxy::status_all(&app)).await
}

#[tauri::command]
async fn cli_proxy_set_enabled(
    app: tauri::AppHandle,
    db_state: tauri::State<'_, DbInitState>,
    cli_key: String,
    enabled: bool,
) -> Result<cli_proxy::CliProxyResult, String> {
    let base_origin = if enabled {
        ensure_db_ready(app.clone(), db_state.inner()).await?;

        blocking::run("cli_proxy_set_enabled_ensure_gateway", {
            let app = app.clone();
            move || {
                let state = app.state::<GatewayState>();
                let mut manager = state.0.lock().map_err(|_| "gateway state poisoned")?;
                let status = if manager.status().running {
                    manager.status()
                } else {
                    let settings = settings::read(&app).unwrap_or_default();
                    let status = manager.start(&app, Some(settings.preferred_port))?;
                    let _ = app.emit("gateway:status", status.clone());
                    status
                };

                Ok(status.base_url.unwrap_or_else(|| {
                    format!(
                        "http://127.0.0.1:{}",
                        status.port.unwrap_or(settings::DEFAULT_GATEWAY_PORT)
                    )
                }))
            }
        })
        .await?
    } else {
        blocking::run("cli_proxy_set_enabled_read_settings", {
            let app = app.clone();
            move || {
                let settings = settings::read(&app).unwrap_or_default();
                Ok(format!("http://127.0.0.1:{}", settings.preferred_port))
            }
        })
        .await?
    };

    blocking::run("cli_proxy_set_enabled_apply", move || {
        cli_proxy::set_enabled(&app, &cli_key, enabled, &base_origin)
    })
    .await
}

#[tauri::command]
async fn cli_proxy_sync_enabled(
    app: tauri::AppHandle,
    base_origin: String,
) -> Result<Vec<cli_proxy::CliProxyResult>, String> {
    blocking::run("cli_proxy_sync_enabled", move || {
        cli_proxy::sync_enabled(&app, &base_origin)
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(DbInitState::default())
        .manage(GatewayState::default())
        .manage(resident::ResidentState::default())
        .plugin(tauri_plugin_opener::init());

    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            resident::show_main_window(app);
        }));

    let app = builder
        .on_window_event(resident::on_window_event)
        .setup(|app| {
            #[cfg(desktop)]
            {
                if let Err(err) = app
                    .handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())
                {
                    eprintln!("updater init error: {err}");
                }

                if let Err(err) = resident::setup_tray(app.handle()) {
                    eprintln!("tray init error: {err}");
                }
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let db_state = app_handle.state::<DbInitState>();
                if let Err(err) = ensure_db_ready(app_handle.clone(), db_state.inner()).await {
                    eprintln!("db init error: {err}");
                    return;
                }

                // M1: auto-start gateway on app launch (required for seamless CLI proxy experience).
                // Port conflicts are handled by the gateway's bind-first-available strategy.
                let settings = match blocking::run("startup_read_settings", {
                    let app_handle = app_handle.clone();
                    move || Ok(settings::read(&app_handle).unwrap_or_default())
                })
                .await
                {
                    Ok(cfg) => cfg,
                    Err(err) => {
                        eprintln!("settings read error: {err}");
                        settings::AppSettings::default()
                    }
                };

                app_handle
                    .state::<resident::ResidentState>()
                    .set_tray_enabled(settings.tray_enabled);

                let status = match blocking::run("startup_gateway_autostart", {
                    let app_handle = app_handle.clone();
                    move || {
                        let state = app_handle.state::<GatewayState>();
                        let mut manager = state.0.lock().unwrap_or_else(|e| e.into_inner());
                        manager.start(&app_handle, Some(settings.preferred_port))
                    }
                })
                .await
                {
                    Ok(status) => status,
                    Err(err) => {
                        eprintln!("gateway auto-start error: {err}");
                        return;
                    }
                };

                let _ = app_handle.emit("gateway:status", status.clone());
                if let Some(base_origin) = status.base_url.as_deref() {
                    // Best-effort: if any CLI proxy is enabled, keep its config aligned with the actual gateway port.
                    let base_origin = base_origin.to_string();
                    let _ = blocking::run("startup_cli_proxy_sync_enabled", {
                        let app_handle = app_handle.clone();
                        move || cli_proxy::sync_enabled(&app_handle, &base_origin)
                    })
                    .await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings_get,
            app_about_get,
            notice_send,
            settings_set,
            settings_gateway_rectifier_set,
            settings_circuit_breaker_notice_set,
            settings_codex_session_id_completion_set,
            cli_manager_claude_info_get,
            cli_manager_codex_info_get,
            cli_manager_gemini_info_get,
            cli_manager_claude_env_set,
            gateway_start,
            gateway_stop,
            gateway_status,
            gateway_check_port_available,
            gateway_sessions_list,
            providers_list,
            provider_upsert,
            provider_set_enabled,
            provider_delete,
            providers_reorder,
            base_url_ping_ms,
            claude_provider_validate_model,
            claude_provider_get_api_key_plaintext,
            claude_validation_history_list,
            claude_validation_history_clear_provider,
            sort_modes_list,
            sort_mode_create,
            sort_mode_rename,
            sort_mode_delete,
            sort_mode_active_list,
            sort_mode_active_set,
            sort_mode_providers_list,
            sort_mode_providers_set_order,
            model_prices_list,
            model_price_upsert,
            model_prices_sync_basellm,
            prompts_list,
            prompts_default_sync_from_files,
            prompt_upsert,
            prompt_set_enabled,
            prompt_delete,
            mcp_servers_list,
            mcp_server_upsert,
            mcp_server_set_enabled,
            mcp_server_delete,
            mcp_parse_json,
            mcp_import_servers,
            skill_repos_list,
            skill_repo_upsert,
            skill_repo_delete,
            skills_installed_list,
            skills_discover_available,
            skill_install,
            skill_set_enabled,
            skill_uninstall,
            skills_local_list,
            skill_import_local,
            skills_paths_get,
            request_logs_list,
            request_logs_list_all,
            request_logs_list_after_id,
            request_logs_list_after_id_all,
            request_log_get,
            request_log_get_by_trace_id,
            request_attempt_logs_by_trace_id,
            app_data_dir_get,
            db_disk_usage_get,
            request_logs_clear_all,
            app_data_reset,
            app_exit,
            app_restart,
            gateway_circuit_status,
            gateway_circuit_reset_provider,
            gateway_circuit_reset_cli,
            usage_summary,
            usage_summary_v2,
            usage_leaderboard_provider,
            usage_leaderboard_day,
            usage_leaderboard_v2,
            usage_hourly_series,
            cost_summary_v1,
            cost_trend_v1,
            cost_breakdown_provider_v1,
            cost_breakdown_model_v1,
            cost_scatter_cli_provider_model_v1,
            cost_top_requests_v1,
            cost_backfill_missing_v1,
            cli_proxy_status_all,
            cli_proxy_set_enabled,
            cli_proxy_sync_enabled
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen {
            has_visible_windows,
            ..
        } = event
        {
            if !has_visible_windows {
                resident::show_main_window(app_handle);
            }
        }
    });
}
