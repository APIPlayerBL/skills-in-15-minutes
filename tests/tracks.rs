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

#[tokio::test]
async fn list_tracks_and_pagination_bounds() {
    let app = test_app().await;
    // default
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
    assert_eq!(resp.status(), StatusCode::OK);
    let json = common::read_body(resp).await;
    assert!(json["total"].as_i64().unwrap() >= 4);
    assert_eq!(json["limit"], 20);
    assert_eq!(json["offset"], 0);
    let items = json["items"].as_array().unwrap();
    assert!(!items.is_empty());

    // limit=0 coerces to 1 (current implementation)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/tracks?limit=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = common::read_body(resp).await;
    assert_eq!(json["limit"], 1);

    // limit=1000 coerces to 100
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/tracks?limit=1000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = common::read_body(resp).await;
    assert_eq!(json["limit"], 100);
}

#[tokio::test]
async fn get_track_by_id_or_slug_and_lessons() {
    let app = test_app().await;
    // list first
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
    let slug = json["items"][0]["slug"].as_str().unwrap().to_string();
    let id = json["items"][0]["id"].as_str().unwrap().to_string();

    for key in [id.as_str(), slug.as_str()] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/tracks/{}", key))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let j = common::read_body(resp).await;
        assert_eq!(j["id"], id);
        assert!(!j["lessons"].as_array().unwrap().is_empty());
    }

    // not found
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/tracks/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn skills_and_skill_tracks() {
    let app = test_app().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = common::read_body(resp).await;
    let items = json["items"].as_array().unwrap();
    assert!(!items.is_empty());
    let slug = items[0]["slug"].as_str().unwrap().to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/skills/{}/tracks", slug))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert!(j["items"].as_array().is_some());
    // unknown skill
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills/nope/tracks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn enroll_requires_auth_and_is_idempotent() {
    let app = test_app().await;
    // unauth
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/tracks/1/enroll")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let token = auth_token(&app, "enroll@example.com").await;
    // resolve a real id
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
    let id = json["items"][0]["id"].as_str().unwrap().to_string();

    // first enroll
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/tracks/{}/enroll", id))
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert_eq!(j["enrolled"], true);

    // second enroll idempotent
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/tracks/{}/enroll", id))
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // progress initialized: my_progress
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
    let j = common::read_body(resp).await;
    assert!(!j["items"].as_array().unwrap().is_empty());
}
