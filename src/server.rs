use crate::auth::{create_auth_provider, AuthProvider};
use crate::config::Config;
use crate::core::Core;
use crate::duration::DurationParser;
use crate::types::{AuthRequest, InfoResponse, RefreshRequest};
use crate::version;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
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
        let core = Core::new(config.clone()).await?;
        let auth = create_auth_provider(&config.server.auth)?;
        
        let app_state = AppState { core, auth };
        
        Ok(DoomsdayServer {
            app_state,
            config,
        })
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
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive())
            )
            .with_state(self.app_state.clone())
    }
    
    pub async fn serve(&self) -> crate::Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.server.port));
        let router = self.create_router();
        
        tracing::info!("Starting Doomsday server on {}", addr);
        
        if let Some(tls_config) = &self.config.server.tls {
            // TODO: Implement TLS support
            tracing::warn!("TLS configuration found but not yet implemented");
        }
        
        let listener = tokio::net::TcpListener::bind(&addr).await
            .map_err(|e| crate::DoomsdayError::internal(format!("Failed to bind to address: {}", e)))?;
        
        axum::serve(listener, router)
            .await
            .map_err(|e| crate::DoomsdayError::internal(format!("Server error: {}", e)))?;
        
        Ok(())
    }
}

async fn info_handler(State(state): State<AppState>) -> Json<InfoResponse> {
    Json(InfoResponse {
        version: version::version(),
        auth_required: state.auth.requires_auth(),
    })
}

async fn auth_handler(
    State(state): State<AppState>,
    Json(request): Json<AuthRequest>,
) -> Result<Json<crate::types::AuthResponse>, StatusCode> {
    match state.auth.authenticate(&request).await {
        Ok(response) => Ok(Json(response)),
        Err(_) => Err(StatusCode::UNAUTHORIZED),
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
    // Check authentication
    if state.auth.requires_auth() {
        let token = extract_token(&headers, &cookies).ok_or(StatusCode::UNAUTHORIZED)?;
        
        if !state.auth.validate_token(&token).await.unwrap_or(false) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    
    let cache = state.core.get_cache();
    let items = cache.list();
    
    // Apply filters
    let filtered_items = if query.beyond.is_some() || query.within.is_some() {
        let now = Utc::now();
        
        items.into_iter().filter(|item| {
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
        }).collect()
    } else {
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
    // Check authentication
    if state.auth.requires_auth() {
        let token = extract_token(&headers, &cookies).ok_or(StatusCode::UNAUTHORIZED)?;
        
        if !state.auth.validate_token(&token).await.unwrap_or(false) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    
    let stats = if let Some(backends) = request.backends {
        // Refresh specific backends
        let mut total_stats = crate::types::PopulateStats {
            num_certs: 0,
            num_paths: 0,
            duration_ms: 0,
        };
        
        for backend_name in backends {
            match state.core.refresh_backend(&backend_name).await {
                Ok(backend_stats) => {
                    total_stats.num_certs += backend_stats.num_certs;
                    total_stats.num_paths += backend_stats.num_paths;
                    total_stats.duration_ms += backend_stats.duration_ms;
                },
                Err(e) => {
                    tracing::error!("Failed to refresh backend {}: {}", backend_name, e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
        
        total_stats
    } else {
        // Refresh all backends
        match state.core.populate_cache().await {
            Ok(stats) => stats,
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
    // Check authentication
    if state.auth.requires_auth() {
        let token = extract_token(&headers, &cookies).ok_or(StatusCode::UNAUTHORIZED)?;
        
        if !state.auth.validate_token(&token).await.unwrap_or(false) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    
    let info = state.core.get_scheduler().get_info();
    Ok(Json(info))
}

fn extract_token(headers: &HeaderMap, cookies: &CookieJar) -> Option<String> {
    // Try to get token from header first
    if let Some(auth_header) = headers.get("X-Doomsday-Token") {
        if let Ok(token) = auth_header.to_str() {
            return Some(token.to_string());
        }
    }
    
    // Try to get token from cookie
    if let Some(cookie) = cookies.get("doomsday-token") {
        return Some(cookie.value().to_string());
    }
    
    None
}

fn static_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/static/*file", get(static_file_handler))
}

async fn dashboard_handler() -> &'static str {
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
    // TODO: Serve static files
    "Static file serving not implemented yet"
}