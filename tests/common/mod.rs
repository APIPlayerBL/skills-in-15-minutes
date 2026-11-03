#![allow(dead_code, unused_imports)]
use axum::Router;
use skills15::{config::Config, routes, state::AppState};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

pub async fn test_app() -> Router {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect memory db");
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await
        .ok();
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    skills15::services::seed::seed_all(&pool)
        .await
        .expect("seed");
    let state = Arc::new(AppState::new(
        pool,
        Config {
            database_url: "sqlite::memory:".to_string(),
            bind_addr: "127.0.0.1:0".to_string(),
            jwt_secret: "test-secret-at-least-32-chars-long!!!!!!!!".to_string(),
        },
    ));
    routes::router(state)
}

pub fn bearer(token: &str) -> String {
    format!("Bearer {}", token)
}

pub async fn read_body(resp: axum::response::Response) -> serde_json::Value {
    use http_body_util::BodyExt;
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }
}
