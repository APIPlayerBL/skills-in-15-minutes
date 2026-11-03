use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

pub mod config;
pub mod domain;
pub mod error;
pub mod middleware;
pub mod repo;
pub mod routes;
pub mod services;
pub mod state;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let cfg = Config::from_env();
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&cfg.database_url)
        .await
        .expect("db connect");
    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("PRAGMA busy_timeout=5000;")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await
        .ok();

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    crate::services::seed::seed_all(&pool).await.ok();

    let state = Arc::new(AppState::new(pool, cfg.clone()));
    let app: Router = routes::router(state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .fallback_service(ServeDir::new("static"));

    let addr = cfg.bind_addr.clone();
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
