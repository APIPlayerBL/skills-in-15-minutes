use crate::state::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/readyz", get(ready))
}

async fn ready(State(st): State<AppState>) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let ok = sqlx::query("SELECT 1").execute(&st.db).await.is_ok();
    if ok {
        (axum::http::StatusCode::OK, Json(json!({"status":"ready"})))
    } else {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status":"not ready"})),
        )
    }
}
