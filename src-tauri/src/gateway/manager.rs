use crate::{
    circuit_breaker, provider_circuit_breakers, providers, request_attempt_logs, request_logs,
    session_manager, settings,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::oneshot;

use super::codex_session_id::CodexSessionIdCache;
use super::events::GatewayLogEvent;
use super::proxy::{ProviderBaseUrlPingCache, RecentErrorCache};
use super::routes::build_router;
use super::util::now_unix_seconds;
use super::{GatewayProviderCircuitStatus, GatewayStatus};

struct RunningGateway {
    port: u16,
    base_url: String,
    circuit: Arc<circuit_breaker::CircuitBreaker>,
    session: Arc<session_manager::SessionManager>,
    shutdown: oneshot::Sender<()>,
    task: tauri::async_runtime::JoinHandle<()>,
    log_task: tauri::async_runtime::JoinHandle<()>,
    attempt_log_task: tauri::async_runtime::JoinHandle<()>,
    circuit_task: tauri::async_runtime::JoinHandle<()>,
}

type RunningGatewayHandles = (
    oneshot::Sender<()>,
    tauri::async_runtime::JoinHandle<()>,
    tauri::async_runtime::JoinHandle<()>,
    tauri::async_runtime::JoinHandle<()>,
    tauri::async_runtime::JoinHandle<()>,
);

#[derive(Default)]
pub struct GatewayManager {
    running: Option<RunningGateway>,
}

#[derive(Clone)]
pub(super) struct GatewayAppState {
    pub(super) app: tauri::AppHandle,
    pub(super) client: reqwest::Client,
    pub(super) log_tx: tokio::sync::mpsc::Sender<request_logs::RequestLogInsert>,
    pub(super) attempt_log_tx:
        tokio::sync::mpsc::Sender<request_attempt_logs::RequestAttemptLogInsert>,
    pub(super) circuit: Arc<circuit_breaker::CircuitBreaker>,
    pub(super) session: Arc<session_manager::SessionManager>,
    pub(super) codex_session_cache: Arc<Mutex<CodexSessionIdCache>>,
    pub(super) recent_errors: Arc<Mutex<RecentErrorCache>>,
    pub(super) latency_cache: Arc<Mutex<ProviderBaseUrlPingCache>>,
}
fn port_candidates(preferred: Option<u16>) -> impl Iterator<Item = u16> {
    let mut candidates = Vec::with_capacity(
        (settings::MAX_GATEWAY_PORT - settings::DEFAULT_GATEWAY_PORT + 2) as usize,
    );

    if let Some(p) = preferred {
        if p > 0 {
            candidates.push(p);
        }
    }

    for port in settings::DEFAULT_GATEWAY_PORT..=settings::MAX_GATEWAY_PORT {
        if candidates.first().copied() == Some(port) {
            continue;
        }
        candidates.push(port);
    }

    candidates.into_iter()
}

fn bind_first_available(preferred: Option<u16>) -> Result<(u16, std::net::TcpListener), String> {
    for port in port_candidates(preferred) {
        let std_listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
            Ok(l) => l,
            Err(_) => continue,
        };

        if std_listener.set_nonblocking(true).is_err() {
            continue;
        }

        return Ok((port, std_listener));
    }

    Err(format!(
        "no available port in range {}..{}",
        settings::DEFAULT_GATEWAY_PORT,
        settings::MAX_GATEWAY_PORT
    ))
}

impl GatewayManager {
    pub fn status(&self) -> GatewayStatus {
        match &self.running {
            Some(r) => GatewayStatus {
                running: true,
                port: Some(r.port),
                base_url: Some(r.base_url.clone()),
            },
            None => GatewayStatus {
                running: false,
                port: None,
                base_url: None,
            },
        }
    }

    pub fn active_sessions(
        &self,
        now_unix: i64,
        limit: usize,
    ) -> Vec<session_manager::ActiveSessionSnapshot> {
        match &self.running {
            Some(r) => r.session.list_active(now_unix, limit),
            None => Vec::new(),
        }
    }

    pub fn start(
        &mut self,
        app: &tauri::AppHandle,
        preferred_port: Option<u16>,
    ) -> Result<GatewayStatus, String> {
        if self.running.is_some() {
            return Ok(self.status());
        }

        let requested_port = preferred_port
            .filter(|p| *p > 0)
            .unwrap_or(settings::DEFAULT_GATEWAY_PORT);

        let (port, std_listener) = bind_first_available(preferred_port)?;

        let base_url = format!("http://127.0.0.1:{port}");
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

        if port != requested_port {
            if let Ok(mut current) = settings::read(app) {
                if current.preferred_port != port {
                    current.preferred_port = port;
                    let _ = settings::write(app, &current);
                }
            }

            let payload = GatewayLogEvent {
                level: "warn",
                error_code: "GW_PORT_IN_USE",
                message: format!("端口 {requested_port} 被占用，已自动切换到 {port}"),
                requested_port,
                bound_port: port,
                base_url: base_url.clone(),
            };
            let _ = app.emit("gateway:log", payload);
        }

        let client = reqwest::Client::builder()
            .user_agent(format!(
                "aio-coding-hub-gateway/{}",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|e| format!("GW_HTTP_CLIENT_INIT: {e}"))?;

        let (log_tx, log_task) = request_logs::start_buffered_writer(app.clone());
        let (attempt_log_tx, attempt_log_task) =
            request_attempt_logs::start_buffered_writer(app.clone());
        let (circuit_tx, circuit_task) =
            provider_circuit_breakers::start_buffered_writer(app.clone());

        let retention_days = settings::log_retention_days_fail_open(app);
        let app_for_cleanup = app.clone();
        std::mem::drop(tauri::async_runtime::spawn_blocking(move || {
            if let Err(err) = request_logs::cleanup_expired(&app_for_cleanup, retention_days) {
                eprintln!("request_logs startup cleanup error: {err}");
            }
            if let Err(err) =
                request_attempt_logs::cleanup_expired(&app_for_cleanup, retention_days)
            {
                eprintln!("request_attempt_logs startup cleanup error: {err}");
            }
        }));

        let circuit_initial = match provider_circuit_breakers::load_all(app) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("provider_circuit_breakers load_all error: {err}");
                Default::default()
            }
        };

        let circuit_config = match settings::read(app) {
            Ok(cfg) => circuit_breaker::CircuitBreakerConfig {
                failure_threshold: cfg.circuit_breaker_failure_threshold.max(1),
                open_duration_secs: (cfg.circuit_breaker_open_duration_minutes as i64)
                    .saturating_mul(60),
            },
            Err(_) => circuit_breaker::CircuitBreakerConfig::default(),
        };
        let circuit = Arc::new(circuit_breaker::CircuitBreaker::new(
            circuit_config,
            circuit_initial,
            Some(circuit_tx),
        ));
        let circuit_for_manager = circuit.clone();
        let session = Arc::new(session_manager::SessionManager::new());
        let codex_session_cache = Arc::new(Mutex::new(CodexSessionIdCache::default()));
        let recent_errors = Arc::new(Mutex::new(RecentErrorCache::default()));
        let latency_cache = Arc::new(Mutex::new(ProviderBaseUrlPingCache::default()));

        let state = GatewayAppState {
            app: app.clone(),
            client,
            log_tx,
            attempt_log_tx,
            circuit,
            session: session.clone(),
            codex_session_cache,
            recent_errors,
            latency_cache,
        };

        let app = build_router(state);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let task = tauri::async_runtime::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(std_listener) {
                Ok(l) => l,
                Err(err) => {
                    eprintln!("gateway listener error on {bind_addr}: {err}");
                    return;
                }
            };

            let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });

            if let Err(err) = serve.await {
                eprintln!("gateway server error on {bind_addr}: {err}");
            }
        });

        self.running = Some(RunningGateway {
            port,
            base_url,
            circuit: circuit_for_manager,
            session,
            shutdown: shutdown_tx,
            task,
            log_task,
            attempt_log_task,
            circuit_task,
        });

        Ok(self.status())
    }

    pub fn circuit_status(
        &self,
        app: &tauri::AppHandle,
        cli_key: &str,
    ) -> Result<Vec<GatewayProviderCircuitStatus>, String> {
        let provider_ids: Vec<i64> = providers::list_by_cli(app, cli_key)?
            .into_iter()
            .map(|p| p.id)
            .collect();

        if provider_ids.is_empty() {
            return Ok(Vec::new());
        }

        let now_unix = now_unix_seconds() as i64;

        if let Some(r) = &self.running {
            return Ok(provider_ids
                .into_iter()
                .map(|provider_id| {
                    let check = r.circuit.should_allow(provider_id, now_unix);
                    let snap = check.after;
                    GatewayProviderCircuitStatus {
                        provider_id,
                        state: snap.state.as_str().to_string(),
                        failure_count: snap.failure_count,
                        failure_threshold: snap.failure_threshold,
                        open_until: snap.open_until,
                        cooldown_until: snap.cooldown_until,
                    }
                })
                .collect());
        }

        let persisted = provider_circuit_breakers::load_all(app).unwrap_or_default();
        let cfg = settings::read(app).unwrap_or_default();
        let failure_threshold = cfg.circuit_breaker_failure_threshold.max(1);

        Ok(provider_ids
            .into_iter()
            .map(|provider_id| {
                if let Some(item) = persisted.get(&provider_id) {
                    let expired = item.state == circuit_breaker::CircuitState::Open
                        && item.open_until.map(|t| now_unix >= t).unwrap_or(true);
                    if expired {
                        return GatewayProviderCircuitStatus {
                            provider_id,
                            state: circuit_breaker::CircuitState::Closed.as_str().to_string(),
                            failure_count: 0,
                            failure_threshold,
                            open_until: None,
                            cooldown_until: None,
                        };
                    }
                    GatewayProviderCircuitStatus {
                        provider_id,
                        state: item.state.as_str().to_string(),
                        failure_count: item.failure_count,
                        failure_threshold,
                        open_until: item.open_until,
                        cooldown_until: None,
                    }
                } else {
                    GatewayProviderCircuitStatus {
                        provider_id,
                        state: circuit_breaker::CircuitState::Closed.as_str().to_string(),
                        failure_count: 0,
                        failure_threshold,
                        open_until: None,
                        cooldown_until: None,
                    }
                }
            })
            .collect())
    }

    pub fn circuit_reset_provider(
        &self,
        app: &tauri::AppHandle,
        provider_id: i64,
    ) -> Result<(), String> {
        if provider_id <= 0 {
            return Err("SEC_INVALID_INPUT: provider_id must be > 0".to_string());
        }

        if let Some(r) = &self.running {
            let now_unix = now_unix_seconds() as i64;
            r.circuit.reset(provider_id, now_unix);
        }

        let _ = provider_circuit_breakers::delete_by_provider_id(app, provider_id)?;
        Ok(())
    }

    pub fn circuit_reset_cli(
        &self,
        app: &tauri::AppHandle,
        cli_key: &str,
    ) -> Result<usize, String> {
        let provider_ids: Vec<i64> = providers::list_by_cli(app, cli_key)?
            .into_iter()
            .map(|p| p.id)
            .collect();

        if provider_ids.is_empty() {
            return Ok(0);
        }

        if let Some(r) = &self.running {
            let now_unix = now_unix_seconds() as i64;
            for provider_id in &provider_ids {
                r.circuit.reset(*provider_id, now_unix);
            }
        }

        let _ = provider_circuit_breakers::delete_by_provider_ids(app, &provider_ids)?;
        Ok(provider_ids.len())
    }

    pub fn take_running(&mut self) -> Option<RunningGatewayHandles> {
        self.running.take().map(|r| {
            (
                r.shutdown,
                r.task,
                r.log_task,
                r.attempt_log_task,
                r.circuit_task,
            )
        })
    }
}
