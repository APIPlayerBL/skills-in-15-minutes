#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::http::Uri;
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

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

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticAssets;

async fn static_handler(uri: Uri) -> Response {
    let raw = uri.path();
    let path = if raw.is_empty() || raw == "/" {
        "index.html".to_string()
    } else {
        raw.trim_start_matches('/').to_string()
    };
    if let Some(file) = StaticAssets::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        let mut resp = Response::builder()
            .status(StatusCode::OK)
            .body(axum::body::Body::from(file.data.into_owned()))
            .unwrap();
        resp.headers_mut()
            .insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
        resp
    } else if let Some(file) = StaticAssets::get("index.html") {
        let mut resp = Response::builder()
            .status(StatusCode::OK)
            .body(axum::body::Body::from(file.data.into_owned()))
            .unwrap();
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            "text/html; charset=utf-8".parse().unwrap(),
        );
        resp
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(axum::body::Body::from("not found"))
            .unwrap()
    }
}

fn main() {
    dotenvy::dotenv().ok();

    let cfg = Config::from_env();
    let db_url = cfg.database_url.clone();
    let addr = cfg.bind_addr.clone();

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let pool = rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
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
        pool
    });

    let state = Arc::new(AppState::new(pool, cfg.clone()));
    let state_for_server = state.clone();
    let addr_for_server = addr.clone();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("server runtime");
        rt.block_on(async {
            let app: Router = routes::router(state_for_server.clone())
                .layer({
                    let origins = std::env::var("CORS_ORIGINS")
                        .unwrap_or_else(|_| "http://127.0.0.1:8080,http://localhost:8080,http://127.0.0.1:8099,http://localhost:8099".to_string());
                    let origins: Vec<axum::http::HeaderValue> =
                        origins.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    CorsLayer::new().allow_origin(origins).allow_methods(Any).allow_headers(Any)
                })
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .fallback(get(static_handler));
            let listener = tokio::net::TcpListener::bind(&addr_for_server)
                .await
                .unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });

    thread::sleep(Duration::from_secs(2));

    let url = format!("http://{}", addr);
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Навыки за 15 минут")
        .with_inner_size(LogicalSize::new(1080.0, 720.0))
        .build(&event_loop)
        .unwrap();

    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .build(&window)
        .expect("webview");

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
