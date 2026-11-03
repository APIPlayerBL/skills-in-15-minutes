use crate::state::AppState;
use axum::Router;
use std::sync::Arc;

pub mod auth;
pub mod certificates;
pub mod health;
pub mod lessons;
pub mod progress;
pub mod tracks;
pub mod users;

pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .merge(auth::router())
        .merge(users::router())
        .merge(tracks::router())
        .merge(lessons::router())
        .merge(progress::router())
        .merge(certificates::router());

    Router::new()
        .merge(health::router())
        .nest("/api/v1", api)
        .with_state((*state).clone())
}
