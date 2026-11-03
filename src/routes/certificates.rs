use crate::error::{AppError, Result};
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use sqlx::Row;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tracks/:id/certificate", post(issue_cert))
        .route("/certificates/:id", get(get_cert))
        .route("/certificates/:id/print", get(print_cert))
        .route("/verify/:token", get(verify_cert))
}

async fn issue_cert(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let track_id: String = sqlx::query_scalar("SELECT id FROM tracks WHERE id=? OR slug=?")
        .bind(&id)
        .bind(&id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound("track not found".into()))?;
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM lessons WHERE track_id=? AND published=1")
            .bind(&track_id)
            .fetch_one(&st.db)
            .await
            .unwrap_or(0);
    let completed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM lesson_progress lp JOIN lessons l ON l.id=lp.lesson_id WHERE lp.user_id=? AND l.track_id=? AND lp.status='completed'",
    )
    .bind(&auth.user_id)
    .bind(&track_id)
    .fetch_one(&st.db)
    .await
    .unwrap_or(0);
    if completed < total || total == 0 {
        return Err(AppError::Conflict("track not fully completed".into()));
    }
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM certificates WHERE user_id=? AND track_id=?")
            .bind(&auth.user_id)
            .bind(&track_id)
            .fetch_optional(&st.db)
            .await?;
    if let Some(cid) = existing {
        let token: String = sqlx::query_scalar("SELECT verify_token FROM certificates WHERE id=?")
            .bind(&cid)
            .fetch_one(&st.db)
            .await?;
        let issued: String = sqlx::query_scalar("SELECT issued_at FROM certificates WHERE id=?")
            .bind(&cid)
            .fetch_one(&st.db)
            .await?;
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({"id": cid, "verify_token": token, "issued_at": issued})),
        ));
    }
    let cid = uuid::Uuid::new_v4().to_string();
    let token = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        rand::random::<[u8; 24]>(),
    );
    let ts = chrono::Utc::now().to_rfc3339();
    let meta = serde_json::json!({"score_threshold": 60, "completed": completed});
    sqlx::query("INSERT INTO certificates (id,user_id,track_id,issued_at,verify_token,metadata_json) VALUES (?,?,?,?,?,?)")
        .bind(&cid)
        .bind(&auth.user_id)
        .bind(&track_id)
        .bind(&ts)
        .bind(&token)
        .bind(meta.to_string())
        .execute(&st.db)
        .await?;
    sqlx::query("UPDATE enrollments SET completed_at=? WHERE user_id=? AND track_id=?")
        .bind(&ts)
        .bind(&auth.user_id)
        .bind(&track_id)
        .execute(&st.db)
        .await
        .ok();
    Ok((
        StatusCode::CREATED,
        Json(
            serde_json::json!({"id": cid, "verify_token": token, "issued_at": ts, "metadata": meta}),
        ),
    ))
}

async fn get_cert(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let row = sqlx::query(
        "SELECT id,verify_token,issued_at,metadata_json FROM certificates WHERE id=? AND user_id=?",
    )
    .bind(&id)
    .bind(&auth.user_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound("certificate not found".into()))?;
    Ok(Json(serde_json::json!({
        "id": row.get::<String,_>("id"),
        "verify_token": row.get::<String,_>("verify_token"),
        "issued_at": row.get::<String,_>("issued_at"),
        "metadata": serde_json::from_str::<serde_json::Value>(&row.get::<String,_>("metadata_json")).unwrap_or_default(),
    })))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn print_cert(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::response::Html<String>> {
    let row = sqlx::query(
        "SELECT c.id,c.issued_at,c.verify_token,u.display_name,t.title FROM certificates c JOIN users u ON u.id=c.user_id JOIN tracks t ON t.id=c.track_id WHERE c.id=?",
    )
    .bind(&id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound("certificate not found".into()))?;
    let name = row
        .get::<Option<String>, _>("display_name")
        .unwrap_or_default();
    let title: String = row.get("title");
    let issued: String = row.get("issued_at");
    let token: String = row.get("verify_token");
    let html = format!(
        "<html><body><h1>Certificate</h1><p>{}</p><p>Track: {}</p><p>Issued: {}</p><p>Verify: /verify/{}</p></body></html>",
        html_escape(&name),
        html_escape(&title),
        html_escape(&issued),
        html_escape(&token)
    );
    Ok(axum::response::Html(html))
}

async fn verify_cert(
    State(st): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let row = sqlx::query(
        "SELECT u.display_name,t.title,c.issued_at,c.metadata_json FROM certificates c JOIN users u ON u.id=c.user_id JOIN tracks t ON t.id=c.track_id WHERE c.verify_token=?",
    )
    .bind(&token)
    .fetch_optional(&st.db)
    .await?;
    match row {
        Some(r) => Ok(Json(serde_json::json!({
            "valid": true,
            "user_display_name": r.get::<Option<String>,_>("display_name"),
            "track_title": r.get::<String,_>("title"),
            "issued_at": r.get::<String,_>("issued_at"),
            "metadata": serde_json::from_str::<serde_json::Value>(&r.get::<String,_>("metadata_json")).unwrap_or_default(),
        }))),
        None => Err(AppError::NotFound("invalid token".into())),
    }
}
