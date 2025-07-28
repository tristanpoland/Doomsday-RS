use crate::auth::{create_auth_provider, AuthProvider};
use crate::config::Config;
use crate::core::Core;
use crate::duration::DurationParser;
use crate::types::{AuthRequest, InfoResponse, RefreshRequest};
use crate::version;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{Json, Response};
use axum::routing::{get, post};
use axum::Router;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub core: Core,
    pub auth: Arc<dyn AuthProvider>,
}

pub struct DoomsdayServer {
    app_state: AppState,
    config: Config,
}

impl DoomsdayServer {
    pub async fn new(config: Config) -> crate::Result<Self> {
        tracing::info!("Creating new DoomsdayServer instance");

        tracing::info!("Initializing core system...");
        let core = Core::new(config.clone()).await?;
        tracing::info!("Core system initialized successfully");

        tracing::info!(
            "Setting up authentication provider: {:?}",
            config.server.auth.auth_type
        );
        let auth = create_auth_provider(&config.server.auth)?;
        tracing::info!("Authentication provider configured");

        let app_state = AppState { core, auth };

        tracing::info!("DoomsdayServer instance created successfully");
        Ok(DoomsdayServer { app_state, config })
    }

    pub fn create_router(&self) -> Router {
        Router::new()
            .route("/v1/info", get(info_handler))
            .route("/v1/auth", post(auth_handler))
            .route("/v1/cache", get(cache_handler))
            .route("/v1/cache/refresh", post(refresh_handler))
            .route("/v1/scheduler", get(scheduler_handler))
            .nest("/", static_routes())
            .layer(
                ServiceBuilder::new()
                    .layer(axum::middleware::from_fn(request_logging_middleware))
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
            .with_state(self.app_state.clone())
    }

    pub async fn serve(&self) -> crate::Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.server.port));
        tracing::info!("🚀 Starting Doomsday Certificate Monitor Server");
        tracing::info!("📍 Server address: {}", addr);
        tracing::info!(
            "🔐 Authentication type: {}",
            self.config.server.auth.auth_type
        );

        let router = self.create_router();
        tracing::info!("🔗 HTTP router created with API endpoints");

        if let Some(tls_config) = &self.config.server.tls {
            // TODO: Implement TLS support
            tracing::warn!("🔒 TLS configuration found but not yet implemented");
        }

        tracing::info!("🔌 Binding to address: {}", addr);
        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            tracing::error!("❌ Failed to bind to address {}: {}", addr, e);
            crate::DoomsdayError::internal(format!("Failed to bind to address: {}", e))
        })?;

        tracing::info!("✅ Server bound successfully, ready to accept connections");
        tracing::info!("🌐 Dashboard available at: http://{}", addr);
        tracing::info!("📊 API endpoints:");
        tracing::info!("   GET  /v1/info - Server information");
        tracing::info!("   POST /v1/auth - Authentication");
        tracing::info!("   GET  /v1/cache - Certificate cache");
        tracing::info!("   POST /v1/cache/refresh - Refresh cache");
        tracing::info!("   GET  /v1/scheduler - Scheduler status");

        let server = axum::serve(listener, router).with_graceful_shutdown(shutdown_signal());

        tracing::info!("🎯 Server is now running and ready to serve requests");

        server.await.map_err(|e| {
            tracing::error!("💥 Server error: {}", e);
            crate::DoomsdayError::internal(format!("Server error: {}", e))
        })?;

        tracing::info!("🛑 Server shutdown complete");
        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("📡 Received Ctrl+C signal, initiating graceful shutdown...");
        },
        _ = terminate => {
            tracing::info!("📡 Received terminate signal, initiating graceful shutdown...");
        },
    }
}

async fn request_logging_middleware(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let remote_addr = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    tracing::info!("Incoming request: {} {} from {}", method, uri, remote_addr);

    let response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status();

    if status.is_server_error() {
        tracing::error!(
            "Request completed: {} {} -> {} in {:?}",
            method,
            uri,
            status,
            duration
        );
    } else if status.is_client_error() {
        tracing::warn!(
            "Request completed: {} {} -> {} in {:?}",
            method,
            uri,
            status,
            duration
        );
    } else {
        tracing::info!(
            "Request completed: {} {} -> {} in {:?}",
            method,
            uri,
            status,
            duration
        );
    }

    response
}

async fn info_handler(State(state): State<AppState>) -> Json<InfoResponse> {
    tracing::debug!("Handling info request");
    let response = InfoResponse {
        version: version::version(),
        auth_required: state.auth.requires_auth(),
    };
    tracing::debug!(
        "Info response: version={}, auth_required={}",
        response.version,
        response.auth_required
    );
    Json(response)
}

async fn auth_handler(
    State(state): State<AppState>,
    Json(request): Json<AuthRequest>,
) -> Result<Json<crate::types::AuthResponse>, StatusCode> {
    tracing::info!(
        "Authentication request received for user: {}",
        request.username
    );

    match state.auth.authenticate(&request).await {
        Ok(response) => {
            tracing::info!("Authentication successful for user: {}", request.username);
            Ok(Json(response))
        }
        Err(e) => {
            tracing::warn!("Authentication failed for user {}: {}", request.username, e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[derive(Deserialize)]
struct CacheQuery {
    beyond: Option<String>,
    within: Option<String>,
}

async fn cache_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: CookieJar,
    Query(query): Query<CacheQuery>,
) -> Result<Json<Vec<crate::types::CacheItem>>, StatusCode> {
    tracing::debug!(
        "Cache request received with filters: beyond={:?}, within={:?}",
        query.beyond,
        query.within
    );

    // Check authentication
    if state.auth.requires_auth() {
        tracing::debug!("Authentication required, validating token");
        let token = extract_token(&headers, &cookies).ok_or_else(|| {
            tracing::warn!("No authentication token provided");
            StatusCode::UNAUTHORIZED
        })?;

        if !state.auth.validate_token(&token).await.unwrap_or(false) {
            tracing::warn!("Invalid authentication token provided");
            return Err(StatusCode::UNAUTHORIZED);
        }
        tracing::debug!("Authentication successful");
    }

    let cache = state.core.get_cache();
    let items = cache.list();
    tracing::info!("Retrieved {} certificates from cache", items.len());

    // Apply filters
    let filtered_items = if query.beyond.is_some() || query.within.is_some() {
        let now = Utc::now();

        let filtered: Vec<_> = items
            .into_iter()
            .filter(|item| {
                let time_until_expiry = item.not_after - now;

                // Check "beyond" filter (certificates expiring beyond the specified duration)
                if let Some(beyond_str) = &query.beyond {
                    if let Ok(beyond_duration) = DurationParser::parse(beyond_str) {
                        if time_until_expiry <= beyond_duration {
                            return false;
                        }
                    }
                }

                // Check "within" filter (certificates expiring within the specified duration)
                if let Some(within_str) = &query.within {
                    if let Ok(within_duration) = DurationParser::parse(within_str) {
                        if time_until_expiry > within_duration {
                            return false;
                        }
                    }
                }

                true
            })
            .collect();

        tracing::info!("Applied filters, returning {} certificates", filtered.len());
        filtered
    } else {
        tracing::debug!("No filters applied, returning all certificates");
        items
    };

    Ok(Json(filtered_items))
}

async fn refresh_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: CookieJar,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<crate::types::PopulateStats>, StatusCode> {
    tracing::info!(
        "Cache refresh request received: backends={:?}",
        request.backends
    );

    // Check authentication
    if state.auth.requires_auth() {
        tracing::debug!("Authentication required for refresh operation");
        let token = extract_token(&headers, &cookies).ok_or_else(|| {
            tracing::warn!("No authentication token provided for refresh");
            StatusCode::UNAUTHORIZED
        })?;

        if !state.auth.validate_token(&token).await.unwrap_or(false) {
            tracing::warn!("Invalid authentication token for refresh operation");
            return Err(StatusCode::UNAUTHORIZED);
        }
        tracing::debug!("Authentication successful for refresh");
    }

    let stats = if let Some(backends) = request.backends {
        tracing::info!("Refreshing specific backends: {:?}", backends);
        // Refresh specific backends
        let mut total_stats = crate::types::PopulateStats {
            num_certs: 0,
            num_paths: 0,
            duration_ms: 0,
        };

        for backend_name in &backends {
            tracing::info!("Starting refresh for backend: {}", backend_name);
            match state.core.refresh_backend(backend_name).await {
                Ok(backend_stats) => {
                    tracing::info!(
                        "Backend {} refresh completed: {} certs, {} paths, {}ms",
                        backend_name,
                        backend_stats.num_certs,
                        backend_stats.num_paths,
                        backend_stats.duration_ms
                    );
                    total_stats.num_certs += backend_stats.num_certs;
                    total_stats.num_paths += backend_stats.num_paths;
                    total_stats.duration_ms += backend_stats.duration_ms;
                }
                Err(e) => {
                    tracing::error!("Failed to refresh backend {}: {}", backend_name, e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }

        tracing::info!(
            "All specified backends refreshed successfully: {} total certs",
            total_stats.num_certs
        );
        total_stats
    } else {
        tracing::info!("Refreshing all backends");
        // Refresh all backends
        match state.core.populate_cache().await {
            Ok(stats) => {
                tracing::info!(
                    "All backends refresh completed: {} certs, {} paths, {}ms",
                    stats.num_certs,
                    stats.num_paths,
                    stats.duration_ms
                );
                stats
            }
            Err(e) => {
                tracing::error!("Failed to refresh cache: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    };

    Ok(Json(stats))
}

async fn scheduler_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: CookieJar,
) -> Result<Json<crate::types::SchedulerInfo>, StatusCode> {
    tracing::debug!("Scheduler info request received");

    // Check authentication
    if state.auth.requires_auth() {
        tracing::debug!("Authentication required for scheduler info");
        let token = extract_token(&headers, &cookies).ok_or_else(|| {
            tracing::warn!("No authentication token provided for scheduler info");
            StatusCode::UNAUTHORIZED
        })?;

        if !state.auth.validate_token(&token).await.unwrap_or(false) {
            tracing::warn!("Invalid authentication token for scheduler info");
            return Err(StatusCode::UNAUTHORIZED);
        }
        tracing::debug!("Authentication successful for scheduler info");
    }

    let info = state.core.get_scheduler().get_info();
    tracing::debug!(
        "Scheduler info retrieved: {} pending tasks, {} running tasks",
        info.pending_tasks,
        info.running_tasks
    );
    Ok(Json(info))
}

fn extract_token(headers: &HeaderMap, cookies: &CookieJar) -> Option<String> {
    // Try to get token from header first
    if let Some(auth_header) = headers.get("X-Doomsday-Token") {
        if let Ok(token) = auth_header.to_str() {
            tracing::debug!("Token found in X-Doomsday-Token header");
            return Some(token.to_string());
        }
    }

    // Try to get token from cookie
    if let Some(cookie) = cookies.get("doomsday-token") {
        tracing::debug!("Token found in doomsday-token cookie");
        return Some(cookie.value().to_string());
    }

    tracing::debug!("No authentication token found in headers or cookies");
    None
}

fn static_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/static/*file", get(static_file_handler))
}

async fn dashboard_handler() -> &'static str {
    tracing::debug!("Serving dashboard page");
    // TODO: Serve the actual dashboard HTML
    "<!DOCTYPE html>
<html>
<head>
    <title>Doomsday Certificate Monitor</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { background: #2196F3; color: white; padding: 20px; margin: -20px -20px 20px -20px; }
        .status { padding: 10px; margin: 10px 0; border-radius: 4px; }
        .expired { background: #ffebee; border-left: 4px solid #f44336; }
        .expiring { background: #fff3e0; border-left: 4px solid #ff9800; }
        .ok { background: #e8f5e8; border-left: 4px solid #4caf50; }
    </style>
</head>
<body>
    <div class='header'>
        <h1>🔒 Doomsday Certificate Monitor</h1>
        <p>Certificate expiration tracking dashboard</p>
    </div>
    <div class='status expired'>
        <h3>⚠️ Expired Certificates</h3>
        <p>Please refresh the page or check the API for current data.</p>
    </div>
    <div class='status expiring'>
        <h3>⏰ Expiring Soon</h3>
        <p>Certificates expiring within 30 days.</p>
    </div>
    <div class='status ok'>
        <h3>✅ OK Certificates</h3>
        <p>Certificates in good standing.</p>
    </div>
    <script>
        // TODO: Add JavaScript to fetch and display real certificate data
        console.log('Doomsday Dashboard loaded');
    </script>
</body>
</html>"
}

async fn static_file_handler() -> &'static str {
    tracing::warn!("Static file serving not yet implemented");
    // TODO: Serve static files
    "Static file serving not implemented yet"
}
