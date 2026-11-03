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

async fn complete_all_lessons(app: &axum::Router, token: &str) {
    // find an english track
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
    let item = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["slug"] == "english-work-track")
        .unwrap();
    let track_id = item["id"].as_str().unwrap().to_string();
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
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/tracks/{}", track_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let j = common::read_body(resp).await;
    for lesson in j["lessons"].as_array().unwrap() {
        let lid = lesson["id"].as_str().unwrap();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/lessons/{}", lid))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let jl = common::read_body(resp).await;
        let tasks = jl["content"]["tasks"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let answers: Vec<_> = if tasks.len() == 2 {
            vec![
                json!({"task_id": tasks[0]["id"], "answer": "b"}),
                json!({"task_id": tasks[1]["id"], "answer": "Reschedule meeting"}),
            ]
        } else {
            tasks
                .iter()
                .map(|t| json!({"task_id": t["id"], "answer": "a"}))
                .collect()
        };
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("/api/v1/lessons/{}/complete", lid))
            .header("authorization", common::bearer(token))
            .header("content-type", "application/json")
            .body(Body::from(json!({"task_answers": answers}).to_string()))
            .unwrap();
        let _ = app.clone().oneshot(req).await.unwrap();
    }
}

#[tokio::test]
async fn certificate_requires_full_completion() {
    let app = test_app().await;
    let token = auth_token(&app, "c1@example.com").await;
    // ensure track id
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
    let item = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["slug"] == "english-work-track")
        .unwrap();
    let track_id = item["id"].as_str().unwrap().to_string();
    // enroll and try issue before completion
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/tracks/{}/enroll", track_id))
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/tracks/{}/certificate", track_id))
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn certificate_issue_verify_print_duplicate() {
    let app = test_app().await;
    let token = auth_token(&app, "c2@example.com").await;
    complete_all_lessons(&app, &token).await;
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
    let item = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["slug"] == "english-work-track")
        .unwrap();
    let track_id = item["id"].as_str().unwrap().to_string();

    // issue
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/tracks/{}/certificate", track_id))
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = common::read_body(resp).await;
    let cert_id = json["id"].as_str().unwrap().to_string();
    let verify_token = json["verify_token"].as_str().unwrap().to_string();

    // duplicate
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/v1/tracks/{}/certificate", track_id))
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert_eq!(j["id"], cert_id);

    // get
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/certificates/{}", cert_id))
                .header("authorization", common::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert_eq!(j["id"], cert_id);
    // print
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/certificates/{}/print", cert_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // verify public
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/verify/{}", verify_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let j = common::read_body(resp).await;
    assert_eq!(j["valid"], true);
    // unknown token
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/verify/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
