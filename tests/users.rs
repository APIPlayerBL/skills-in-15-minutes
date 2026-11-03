mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::test_app;
use serde_json::json;
use tower::ServiceExt;

async fn auth_token(app: &axum::Router, email: &str) -> String {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": email, "password": "password12345", "display_name": "U"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let json = common::read_body(resp).await;
    json["access_token"].as_str().unwrap().to_string()
}

async fn enroll_first(app: &axum::Router, token: &str) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/tracks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = common::read_body(resp).await;
    let track_id = json["items"][0]["id"].as_str().unwrap().to_string();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/tracks/{}/enroll", track_id))
                .header("authorization", common::bearer(token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn me_and_patch() {
    let app = test_app().await;
    let token = auth_token(&app, "u1@example.com").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert_eq!(j["email"], "u1@example.com");
    assert_eq!(j["stats"]["tracks_enrolled"], 0);
    assert_eq!(j["stats"]["lessons_completed"], 0);

    let req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/me")
        .header("authorization", common::bearer(&token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"display_name":"Иван Updated"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert_eq!(j["display_name"], "Иван Updated");
}

#[tokio::test]
async fn progress_summary_and_schedule_and_my_progress() {
    let app = test_app().await;
    let token = auth_token(&app, "u2@example.com").await;
    enroll_first(&app, &token).await;
    // schedule
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me/schedule")
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert!(!j["items"].as_array().unwrap().is_empty());
    // my_progress
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me/progress")
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert!(!j["items"].as_array().unwrap().is_empty());
    // summary percent initial
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me/progress/summary")
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    let items = j["items"].as_array().unwrap();
    assert!(!items.is_empty());
    let total: i64 = items.iter().map(|x| x["total"].as_i64().unwrap()).sum();
    let completed: i64 = items.iter().map(|x| x["completed"].as_i64().unwrap()).sum();
    assert!(completed <= total);
}

#[tokio::test]
async fn patch_me_empty_and_unauthorized() {
    let app = test_app().await;
    // unauthorized
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // empty patch okay
    let token = auth_token(&app, "u3@example.com").await;
    let req = Request::builder()
        .method(Method::PATCH)
        .uri("/api/v1/me")
        .header("authorization", common::bearer(&token))
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
