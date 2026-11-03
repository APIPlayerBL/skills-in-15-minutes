mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::test_app;
use tower::ServiceExt;

#[tokio::test]
async fn healthz_ok() {
    let app = test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = common::read_body(resp).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn readyz_ok() {
    let app = test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = common::read_body(resp).await;
    assert_eq!(json["status"], "ready");
}
