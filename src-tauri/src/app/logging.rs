//! Usage: Tracing/logging initialization (rolling file logs + best-effort cleanup).

use crate::{app_paths, settings};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;

const LOG_SUBDIR: &str = "logs";
const LOG_FILE_PREFIX: &str = "aio-coding-hub.log";
const CLEANUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

static TRACING_GUARD: OnceLock<Mutex<Option<WorkerGuard>>> = OnceLock::new();
static TRACING_INIT: OnceLock<()> = OnceLock::new();

pub(crate) fn init(app: &tauri::AppHandle) {
    TRACING_INIT.get_or_init(|| {
        let app = app.clone();
        if let Err(err) = init_impl(&app) {
            // Last-resort fallback: stderr logger (may be invisible on Windows release).
            let _ = tracing_subscriber::fmt()
                .with_env_filter(default_env_filter())
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .try_init();
            eprintln!("tracing init failed: {err}");
        }
    });
}

fn init_impl(app: &tauri::AppHandle) -> Result<(), String> {
    let log_dir = ensure_log_dir(app)?;
    let env_filter = default_env_filter();

    let file_appender = tracing_appender::rolling::daily(&log_dir, LOG_FILE_PREFIX);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    TRACING_GUARD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "logging guard mutex poisoned".to_string())?
        .replace(guard);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    #[cfg(debug_assertions)]
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer);

    #[cfg(debug_assertions)]
    let subscriber = subscriber.with(stdout_layer);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| format!("failed to set global tracing subscriber: {e}"))?;

    // Capture `log` crate records (from dependencies) into `tracing` when possible.
    // If another logger is already set (e.g. by a dependency), skip silently.
    let _ = tracing_log::LogTracer::init();

    tracing::info!(log_dir = %log_dir.display(), "tracing initialized");

    spawn_cleanup_task(app.clone(), log_dir);

    Ok(())
}

fn default_env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        #[cfg(debug_assertions)]
        {
            tracing_subscriber::EnvFilter::new("info,aio_coding_hub_lib=debug,aio_coding_hub=debug")
        }
        #[cfg(not(debug_assertions))]
        {
            tracing_subscriber::EnvFilter::new("info")
        }
    })
}

fn ensure_log_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app_paths::app_data_dir(app)?;
    let dir = base.join(LOG_SUBDIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create log dir {}: {e}", dir.display()))?;
    Ok(dir)
}

fn spawn_cleanup_task(app: tauri::AppHandle, log_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        let app_for_cleanup = app.clone();
        let log_dir_for_cleanup = log_dir.clone();
        std::mem::drop(tauri::async_runtime::spawn_blocking(move || {
            cleanup_once(&app_for_cleanup, &log_dir_for_cleanup);
        }));

        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        // First tick is immediate; skip it so we don't run twice at startup.
        interval.tick().await;
        loop {
            interval.tick().await;
            let app_for_cleanup = app.clone();
            let log_dir_for_cleanup = log_dir.clone();
            std::mem::drop(tauri::async_runtime::spawn_blocking(move || {
                cleanup_once(&app_for_cleanup, &log_dir_for_cleanup);
            }));
        }
    });
}

fn cleanup_once(app: &tauri::AppHandle, log_dir: &Path) {
    let retention_days = settings::log_retention_days_fail_open(app).max(1);
    match cleanup_logs(log_dir, retention_days) {
        Ok(deleted) if deleted > 0 => {
            tracing::info!(retention_days, deleted, "cleaned up old log files");
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(retention_days, "log cleanup failed: {}", err);
        }
    }
}

fn cleanup_logs(log_dir: &Path, retention_days: u32) -> Result<usize, String> {
    let retention_days = retention_days.max(1);
    let now = SystemTime::now();
    let cutoff = now
        .checked_sub(Duration::from_secs(
            (retention_days as u64).saturating_mul(24 * 60 * 60),
        ))
        .unwrap_or(UNIX_EPOCH);

    let mut deleted = 0usize;
    let entries = std::fs::read_dir(log_dir).map_err(|e| format!("read_dir failed: {e}"))?;
    for entry in entries {
        let entry = match entry {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!("log cleanup: read_dir entry error: {}", err);
                continue;
            }
        };

        let path = entry.path();
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !name.starts_with(LOG_FILE_PREFIX) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(path = %path.display(), "log cleanup: metadata error: {}", err);
                continue;
            }
        };
        if !meta.is_file() {
            continue;
        }

        let modified = meta.modified().unwrap_or(UNIX_EPOCH);
        if modified >= cutoff {
            continue;
        }

        match std::fs::remove_file(&path) {
            Ok(()) => deleted = deleted.saturating_add(1),
            Err(err) => {
                tracing::warn!(path = %path.display(), "log cleanup: remove failed: {}", err);
            }
        }
    }

    Ok(deleted)
}
