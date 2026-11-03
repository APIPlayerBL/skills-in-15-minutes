use crate::error::Result;
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde::Deserialize;
use sqlx::Row;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me).patch(patch_me))
        .route("/me/progress", get(my_progress))
        .route("/me/schedule", get(my_schedule))
        .route("/me/progress/summary", get(progress_summary))
}

async fn me(State(st): State<AppState>, auth: AuthUser) -> Result<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT id,email,display_name,role,created_at FROM users WHERE id=?")
        .bind(&auth.user_id)
        .fetch_one(&st.db)
        .await?;
    let completed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM lesson_progress WHERE user_id=? AND status='completed'",
    )
    .bind(&auth.user_id)
    .fetch_one(&st.db)
    .await
    .unwrap_or(0);
    let enrolled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM enrollments WHERE user_id=?")
        .bind(&auth.user_id)
        .fetch_one(&st.db)
        .await
        .unwrap_or(0);
    Ok(Json(serde_json::json!({
        "id": row.get::<String,_>("id"),
        "email": row.get::<String,_>("email"),
        "display_name": row.get::<Option<String>,_>("display_name"),
        "role": row.get::<String,_>("role"),
        "created_at": row.get::<String,_>("created_at"),
        "stats": {"tracks_enrolled": enrolled, "lessons_completed": completed, "streak_days": 0}
    })))
}

#[derive(Deserialize)]
struct PatchMe {
    display_name: Option<String>,
}

async fn patch_me(
    State(st): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PatchMe>,
) -> Result<Json<serde_json::Value>> {
    if let Some(name) = &req.display_name {
        sqlx::query("UPDATE users SET display_name=?, updated_at=? WHERE id=?")
            .bind(name)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(&auth.user_id)
            .execute(&st.db)
            .await?;
    }
    me(State(st), auth).await
}

async fn my_progress(
    State(st): State<AppState>,
    auth: AuthUser,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>> {
    let track_filter = q.get("track_id").cloned();
    let rows = if let Some(tf) = track_filter {
        sqlx::query("SELECT lp.lesson_id,lp.status,lp.score,lp.attempts_count FROM lesson_progress lp JOIN lessons l ON l.id=lp.lesson_id WHERE lp.user_id=? AND l.track_id=?")
            .bind(&auth.user_id).bind(tf).fetch_all(&st.db).await?
    } else {
        sqlx::query(
            "SELECT lesson_id,status,score,attempts_count FROM lesson_progress WHERE user_id=?",
        )
        .bind(&auth.user_id)
        .fetch_all(&st.db)
        .await?
    };
    let mut items = Vec::new();
    for r in rows {
        items.push(serde_json::json!({
            "lesson_id": r.get::<String,_>("lesson_id"),
            "status": r.get::<String,_>("status"),
            "score": r.get::<Option<i64>,_>("score"),
            "attempts_count": r.get::<i64,_>("attempts_count"),
        }));
    }
    Ok(Json(serde_json::json!({"items": items})))
}

async fn my_schedule(
    State(st): State<AppState>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>> {
    let rows = sqlx::query("SELECT s.lesson_id,s.due_at,s.state,l.track_id,l.title FROM schedules s JOIN lessons l ON l.id=s.lesson_id WHERE s.user_id=? ORDER BY s.due_at LIMIT 50")
        .bind(&auth.user_id).fetch_all(&st.db).await?;
    let mut items = Vec::new();
    for r in rows {
        items.push(serde_json::json!({
            "lesson_id": r.get::<String,_>("lesson_id"),
            "track_id": r.get::<String,_>("track_id"),
            "title": r.get::<String,_>("title"),
            "due_at": r.get::<String,_>("due_at"),
            "state": r.get::<String,_>("state"),
        }));
    }
    Ok(Json(serde_json::json!({"items": items})))
}

async fn progress_summary(
    State(st): State<AppState>,
    auth: AuthUser,
) -> Result<Json<serde_json::Value>> {
    let rows = sqlx::query("SELECT t.id as track_id, COUNT(lp.lesson_id) as total, SUM(CASE WHEN lp.status='completed' THEN 1 ELSE 0 END) as completed FROM tracks t LEFT JOIN lessons l ON l.track_id=t.id LEFT JOIN lesson_progress lp ON lp.lesson_id=l.id AND lp.user_id=? GROUP BY t.id")
        .bind(&auth.user_id).fetch_all(&st.db).await?;
    let mut items = Vec::new();
    for r in rows {
        let total: i64 = r.get("total");
        let completed: i64 = r.try_get("completed").unwrap_or(0);
        let pct = if total > 0 {
            completed * 100 / total
        } else {
            0
        };
        items.push(serde_json::json!({"track_id": r.get::<String,_>("track_id"), "completed": completed, "total": total, "percent": pct}));
    }
    Ok(Json(serde_json::json!({"items": items})))
}
