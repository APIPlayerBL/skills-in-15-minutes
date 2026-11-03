mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::test_app;
use serde_json::json;
use tower::ServiceExt;

async fn register_user(app: &axum::Router, email: &str, password: &str) -> (String, String) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": email, "password": password, "display_name": "Тест"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let cookie = resp
        .headers()
        .get("set-cookie")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    let json = common::read_body(resp).await;
    assert!(
        status == StatusCode::OK || status == StatusCode::CREATED,
        "register status {} body {}",
        status,
        json
    );
    let token = json["access_token"].as_str().unwrap().to_string();
    (token, cookie)
}

#[tokio::test]
async fn register_validation_password_short() {
    let app = test_app().await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"a@b.com","password":"short","display_name":"x"}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn register_validation_email() {
    let app = test_app().await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"no-at","password":"longenoughpass","display_name":"x"}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn register_and_duplicate_email() {
    let app = test_app().await;
    let (token, cookie) = register_user(&app, "dup@example.com", "password12345").await;
    assert!(!token.is_empty());
    assert!(cookie.contains("refresh_token="));
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"dup@example.com","password":"password12345"}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_wrong_password() {
    let app = test_app().await;
    register_user(&app, "login@example.com", "password12345").await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"login@example.com","password":"wrongwrongwrong"}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_success_and_me() {
    let app = test_app().await;
    register_user(&app, "me@example.com", "password12345").await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"me@example.com","password":"password12345"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = common::read_body(resp).await;
    let token = json["access_token"].as_str().unwrap();
    let req = Request::builder()
        .uri("/api/v1/me")
        .header("authorization", common::bearer(token))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = common::read_body(resp).await;
    assert_eq!(json["email"], "me@example.com");
    assert_eq!(json["role"], "user");
}

fn extract_refresh(cookie: &str) -> Option<String> {
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("refresh_token=") {
            return Some(v.to_string());
        }
    }
    None
}

#[tokio::test]
async fn refresh_and_logout() {
    let app = test_app().await;
    let (_token, cookie) = register_user(&app, "refresh@example.com", "password12345").await;
    let raw = extract_refresh(&cookie).expect("refresh cookie");
    // The cookie value stored includes maybe entire Set-Cookie; strip trailing? extract returns raw possibly includes attributes? We split by ; so okay.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header("cookie", format!("refresh_token={}", raw))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = common::read_body(resp).await;
    assert!(json["access_token"].as_str().is_some());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/logout")
        .header("cookie", format!("refresh_token={}", raw))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header("cookie", format!("refresh_token={}", raw))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
