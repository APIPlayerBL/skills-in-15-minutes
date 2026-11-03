use crate::error::{AppError, Result};
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use sqlx::Row;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tracks", get(list_tracks))
        .route("/tracks/:id", get(get_track))
        .route("/tracks/:id/enroll", post(enroll))
        .route("/tracks/:id/certificate", get(get_cert))
        .route("/skills", get(list_skills))
        .route("/skills/:slug/tracks", get(skills_tracks))
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct TrackQuery {
    published: Option<bool>,
    skill: Option<String>,
    difficulty: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_tracks(
    State(st): State<AppState>,
    Query(q): Query<TrackQuery>,
) -> Result<Json<serde_json::Value>> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0);
    let rows = sqlx::query("SELECT id,slug,title,description,difficulty,estimated_minutes,version,published FROM tracks WHERE published=1 ORDER BY title LIMIT ? OFFSET ?")
        .bind(limit).bind(offset).fetch_all(&st.db).await?;
    let mut items = Vec::new();
    for r in rows {
        items.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "slug": r.get::<String,_>("slug"),
            "title": r.get::<String,_>("title"),
            "description": r.get::<Option<String>,_>("description"),
            "difficulty": r.get::<String,_>("difficulty"),
            "estimated_minutes": r.get::<i64,_>("estimated_minutes"),
            "version": r.get::<i64,_>("version"),
        }));
    }
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&st.db)
        .await
        .unwrap_or(0);
    Ok(Json(
        serde_json::json!({"items": items, "total": total, "limit": limit, "offset": offset}),
    ))
}

async fn get_track(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    use sqlx::Row;
    let row = sqlx::query("SELECT id,slug,title,description,difficulty,estimated_minutes,version FROM tracks WHERE (id=? OR slug=?) AND published=1")
        .bind(&id).bind(&id).fetch_optional(&st.db).await?
        .ok_or(AppError::NotFound("track not found".into()))?;
    let track_id: String = row.get("id");
    let lessons = sqlx::query("SELECT id,title,order_index,duration_min FROM lessons WHERE track_id=? AND published=1 ORDER BY order_index")
        .bind(&track_id).fetch_all(&st.db).await?;
    let mut ls = Vec::new();
    for l in lessons {
        ls.push(serde_json::json!({
            "id": l.get::<String,_>("id"),
            "title": l.get::<String,_>("title"),
            "order_index": l.get::<i64,_>("order_index"),
            "duration_min": l.get::<i64,_>("duration_min"),
        }));
    }
    Ok(Json(serde_json::json!({
        "id": track_id,
        "slug": row.get::<String,_>("slug"),
        "title": row.get::<String,_>("title"),
        "description": row.get::<Option<String>,_>("description"),
        "difficulty": row.get::<String,_>("difficulty"),
        "estimated_minutes": row.get::<i64,_>("estimated_minutes"),
        "version": row.get::<i64,_>("version"),
        "lessons": ls
    })))
}

async fn enroll(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    // resolve track id
    let track_id: String = sqlx::query_scalar("SELECT id FROM tracks WHERE id=? OR slug=?")
        .bind(&id)
        .bind(&id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound("track not found".into()))?;
    let ts = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT OR IGNORE INTO enrollments (user_id,track_id,started_at) VALUES (?,?,?)")
        .bind(&auth.user_id)
        .bind(&track_id)
        .bind(&ts)
        .execute(&st.db)
        .await?;
    // schedule lessons due now+day by order_index
    let lessons =
        sqlx::query("SELECT id FROM lessons WHERE track_id=? AND published=1 ORDER BY order_index")
            .bind(&track_id)
            .fetch_all(&st.db)
            .await?;
    for (i, l) in lessons.iter().enumerate() {
        let lid: String = l.get("id");
        let due = (chrono::Utc::now() + chrono::Duration::days(i as i64)).to_rfc3339();
        sqlx::query("INSERT OR IGNORE INTO schedules (id,user_id,lesson_id,due_at,interval_days,ease,lapses,state,updated_at) VALUES (?,?,?,?,?,?,?,?,?)")
            .bind(uuid::Uuid::new_v4().to_string()).bind(&auth.user_id).bind(&lid).bind(&due).bind(1i64).bind(2.5f64).bind(0i64).bind("new").bind(&ts)
            .execute(&st.db).await?;
        sqlx::query("INSERT OR IGNORE INTO lesson_progress (user_id,lesson_id,status,attempts_count,last_activity_at) VALUES (?,?,?,0,?)")
            .bind(&auth.user_id).bind(&lid).bind("not_started").bind(&ts).execute(&st.db).await?;
    }
    Ok(Json(
        serde_json::json!({"enrolled": true, "track_id": track_id}),
    ))
}

async fn get_cert(
    State(st): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let track_id: String = sqlx::query_scalar("SELECT id FROM tracks WHERE id=? OR slug=?")
        .bind(&id)
        .bind(&id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound("track not found".into()))?;
    // just placeholder list user's certs for track
    Ok(Json(
        serde_json::json!({"track_id": track_id, "certificate": null}),
    ))
}

async fn list_skills(State(st): State<AppState>) -> Result<Json<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT id,slug,title,description,category FROM skills ORDER BY category,title",
    )
    .fetch_all(&st.db)
    .await?;
    let mut items = Vec::new();
    for r in rows {
        items.push(serde_json::json!({
            "id": r.get::<String,_>("id"),
            "slug": r.get::<String,_>("slug"),
            "title": r.get::<String,_>("title"),
            "description": r.get::<Option<String>,_>("description"),
            "category": r.get::<String,_>("category"),
        }));
    }
    Ok(Json(serde_json::json!({"items": items})))
}

async fn skills_tracks(
    State(st): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let skill_id: String = sqlx::query_scalar("SELECT id FROM skills WHERE slug=?")
        .bind(&slug)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound("skill not found".into()))?;
    let rows = sqlx::query("SELECT t.id,t.slug,t.title FROM tracks t JOIN track_skills ts ON ts.track_id=t.id WHERE ts.skill_id=?")
        .bind(&skill_id).fetch_all(&st.db).await?;
    let mut items = Vec::new();
    for r in rows {
        items.push(serde_json::json!({"id":r.get::<String,_>("id"),"slug":r.get::<String,_>("slug"),"title":r.get::<String,_>("title")}));
    }
    Ok(Json(serde_json::json!({"items": items})))
}
