#[allow(dead_code)]
pub mod aggregator;
#[allow(dead_code)]
pub mod dal;
#[allow(dead_code)]
pub mod models;

use aggregator::{AggregationService, HttpStatsFetcher};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use dal::{ServerRepository, SqliteRepository};
use models::ServerConfig;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Environment variable used to override the listening port.
pub const PORT_ENV: &str = "MY_SERVER_PORT";

/// Default listening port used when [`PORT_ENV`] is unset or invalid.
pub const DEFAULT_PORT: u16 = 3000;

/// Environment variable used to override the listening host address.
pub const HOST_ENV: &str = "MY_SERVER_HOST";

/// Default listening host. `0.0.0.0` binds all interfaces so the service is
/// reachable from other machines on the LAN, not only from localhost.
pub const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

/// Environment variable used to override the static web UI directory.
pub const WEBUI_ENV: &str = "MY_SERVER_WEBUI_DIR";

/// Default static web UI directory, relative to the process working directory.
/// Matches the source layout so `cargo run` from `my-src` keeps working.
pub const DEFAULT_WEBUI_DIR: &str = "src/my-server/webui";

#[derive(Clone)]
pub struct AppState {
    pub repo: SqliteRepository,
    pub aggregator: Arc<AggregationService<HttpStatsFetcher>>,
}

/// Parse a port from an optional string, falling back to [`DEFAULT_PORT`].
///
/// Invalid or out-of-range values are ignored so a malformed env var never
/// prevents the server from starting.
fn parse_port(value: Option<String>) -> u16 {
    value
        .and_then(|v| v.trim().parse::<u16>().ok())
        .filter(|&p| p != 0)
        .unwrap_or(DEFAULT_PORT)
}

/// Resolve the listening port from the [`PORT_ENV`] environment variable.
fn resolve_port() -> u16 {
    parse_port(std::env::var(PORT_ENV).ok())
}

/// Parse a host address from an optional string, falling back to [`DEFAULT_HOST`].
///
/// Invalid values are ignored so a malformed env var never prevents the server
/// from starting.
fn parse_host(value: Option<String>) -> IpAddr {
    value
        .and_then(|v| v.trim().parse::<IpAddr>().ok())
        .unwrap_or(DEFAULT_HOST)
}

/// Resolve the listening host from the [`HOST_ENV`] environment variable.
fn resolve_host() -> IpAddr {
    parse_host(std::env::var(HOST_ENV).ok())
}

/// Pick the web UI directory from an optional string, falling back to
/// [`DEFAULT_WEBUI_DIR`] when the value is absent or blank.
fn pick_webui_dir(value: Option<String>) -> String {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_WEBUI_DIR.to_string())
}

/// Resolve the web UI directory from the [`WEBUI_ENV`] environment variable.
fn resolve_webui_dir() -> String {
    pick_webui_dir(std::env::var(WEBUI_ENV).ok())
}

pub async fn get_servers(State(state): State<AppState>) -> Result<Json<Vec<ServerConfig>>, StatusCode> {
    match state.repo.list_servers().await {
        Ok(servers) => Ok(Json(servers)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_server(
    State(state): State<AppState>,
    Json(mut payload): Json<ServerConfig>,
) -> Result<Json<ServerConfig>, StatusCode> {
    match state.repo.create_server(&payload).await {
        Ok(id) => {
            payload.id = Some(id);
            Ok(Json(payload))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn update_server(
    Path(id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<ServerConfig>,
) -> Result<Json<ServerConfig>, StatusCode> {
    match state.repo.update_server(id, &payload).await {
        Ok(_) => Ok(Json(payload)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete_server(
    Path(id): Path<i64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.repo.delete_server(id).await {
        Ok(_) => Ok(Json(serde_json::json!({ "status": "success" }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_dashboard_stats(
    State(state): State<AppState>,
) -> Result<Json<aggregator::DashboardStats>, StatusCode> {
    let servers = state.repo.list_servers().await.unwrap_or_default();
    let stats = state.aggregator.aggregate(servers).await;
    Ok(Json(stats))
}

pub async fn run_server() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "my_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize DB
    let repo = SqliteRepository::new("sqlite:servers.db").await?;
    let fetcher = HttpStatsFetcher::new();
    let aggregator = AggregationService::new(fetcher);

    let state = AppState {
        repo,
        aggregator: Arc::new(aggregator),
    };

    let serve_dir = ServeDir::new(resolve_webui_dir());

    let api_routes = Router::new()
        .route("/servers", get(get_servers).post(create_server))
        .route("/servers/:id", put(update_server).delete(delete_server))
        .route("/dashboard/stats", get(get_dashboard_stats))
        .with_state(state);

    let app = Router::new()
        .nest("/api/v1", api_routes)
        .fallback_service(serve_dir)
        .layer(CorsLayer::permissive())
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = SocketAddr::new(resolve_host(), resolve_port());
    tracing::debug!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{pick_webui_dir, parse_host, parse_port, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_WEBUI_DIR};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn parse_port_uses_default_when_absent() {
        assert_eq!(parse_port(None), DEFAULT_PORT);
    }

    #[test]
    fn parse_host_uses_default_when_absent() {
        assert_eq!(parse_host(None), DEFAULT_HOST);
    }

    #[test]
    fn parse_host_reads_valid_value() {
        assert_eq!(
            parse_host(Some(" 127.0.0.1 ".to_string())),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
        );
    }

    #[test]
    fn parse_host_falls_back_on_invalid_value() {
        assert_eq!(parse_host(Some("not-an-ip".to_string())), DEFAULT_HOST);
        assert_eq!(parse_host(Some(String::new())), DEFAULT_HOST);
    }

    #[test]
    fn pick_webui_dir_uses_default_when_absent() {
        assert_eq!(pick_webui_dir(None), DEFAULT_WEBUI_DIR);
    }

    #[test]
    fn pick_webui_dir_uses_default_when_blank() {
        assert_eq!(pick_webui_dir(Some("   ".to_string())), DEFAULT_WEBUI_DIR);
    }

    #[test]
    fn pick_webui_dir_reads_custom_value() {
        assert_eq!(
            pick_webui_dir(Some("  /opt/app/webui  ".to_string())),
            "/opt/app/webui"
        );
    }

    #[test]
    fn parse_port_reads_valid_value() {
        assert_eq!(parse_port(Some(" 8088 ".to_string())), 8088);
    }

    #[test]
    fn parse_port_falls_back_on_invalid_value() {
        assert_eq!(parse_port(Some("not-a-port".to_string())), DEFAULT_PORT);
        assert_eq!(parse_port(Some("0".to_string())), DEFAULT_PORT);
        assert_eq!(parse_port(Some("70000".to_string())), DEFAULT_PORT);
    }
}
