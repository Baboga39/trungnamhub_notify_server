mod auth;
mod models;
mod routes;
mod store;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    http::{HeaderValue, Method},
    middleware,
    routing::{get, patch, post},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::store::NotificationStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<NotificationStore>,
    pub jwt_secret: String,
    pub internal_key: String,
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "trungnamhub_notify_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5002);

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "supersecret".to_string());
    let internal_key = std::env::var("NOTIFY_INTERNAL_KEY")
        .unwrap_or_else(|_| "trungnam-internal-notify-key".to_string());
    let data_file = std::env::var("NOTIFY_DATA_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/notifications.json"));

    let state = AppState {
        store: Arc::new(NotificationStore::new(data_file)),
        jwt_secret,
        internal_key,
    };

    let cors = build_cors_layer();

    let protected_routes = Router::new()
        .route("/api/v1/notifications", get(routes::list_notifications))
        .route("/api/v1/notifications/stream", get(routes::stream_notifications))
        .route("/api/v1/notifications/read-all", patch(routes::mark_all_read))
        .route("/api/v1/notifications/{id}/read", patch(routes::mark_read))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::jwt_auth_middleware,
        ));

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/internal/notify", post(routes::internal_notify))
        .merge(protected_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("trungnamhub_notify_server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(listener, app)
        .await
        .expect("Server exited with error");
}

fn build_cors_layer() -> CorsLayer {
    let origins_raw = std::env::var("CORS_ORIGINS").unwrap_or_default();
    let origins: Vec<HeaderValue> = origins_raw
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| HeaderValue::from_str(s).ok())
        .collect();

    let base = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::OPTIONS])
        .allow_headers(Any);

    if origins.is_empty() {
        base.allow_origin(Any)
    } else {
        base.allow_origin(origins)
    }
}
