use crate::error::{AppError, Result};
use crate::middleware::AuthUser;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use sqlx::Row;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lessons/:id", get(get_lesson))
        .route("/lessons/:id/complete", post(complete_lesson))
        .route("/lessons/:id/progress", get(lesson_progress))
}

async fn get_lesson(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT id,track_id,slug,title,order_index,duration_min,version,content_json,content_hash FROM lessons WHERE id=?")
        .bind(&id).fetch_optional(&st.db).await?
        .ok_or(AppError::NotFound("lesson not found".into()))?;
    let content: serde_json::Value =
        serde_json::from_str(row.get::<String, _>("content_json").as_str())
            .unwrap_or(serde_json::json!({}));
    let tasks = sqlx::query(
        "SELECT id,kind,prompt,options_json,weight FROM lesson_tasks WHERE lesson_id=?",
    )
    .bind(&id)
    .fetch_all(&st.db)
    .await?;
    let mut task_list = Vec::new();
    for t in tasks {
        let opts: Option<String> = t.get("options_json");
        task_list.push(serde_json::json!({
            "id": t.get::<String,_>("id"),
            "kind": t.get::<String,_>("kind"),
            "prompt": t.get::<String,_>("prompt"),
            "options": opts.as_deref().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
            "weight": t.get::<i64,_>("weight"),
        }));
    }
    let mut c = content.clone();
    if let Some(obj) = c.as_object_mut() {
        obj.insert("tasks".into(), serde_json::json!(task_list.clone()));
    }
    Ok(Json(serde_json::json!({
        "id": row.get::<String,_>("id"),
        "track_id": row.get::<String,_>("track_id"),
        "slug": row.get::<String,_>("slug"),
        "title": row.get::<String,_>("title"),
        "order_index": row.get::<i64,_>("order_index"),
        "duration_min": row.get::<i64,_>("duration_min"),
        "version": row.get::<i64,_>("version"),
        "content_hash": row.get::<String,_>("content_hash"),
        "content": c
    })))
}

#[derive(Deserialize)]
struct CompleteReq {
    task_answers: Vec<TaskAnswer>,
    client_started_at: Option<String>,
}
#[derive(Deserialize)]
struct TaskAnswer {
    task_id: String,
    answer: serde_json::Value,
}

async fn complete_lesson(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<CompleteReq>,
) -> Result<Json<serde_json::Value>> {
    // idempotency: if already completed with same client_started_at return cached (simple: if status completed, return no double)
    let progress_row = sqlx::query(
        "SELECT status,score,idempotency_key FROM lesson_progress WHERE user_id=? AND lesson_id=?",
    )
    .bind(&auth.user_id)
    .bind(&id)
    .fetch_optional(&st.db)
    .await?;
    let idem = req.client_started_at.clone().unwrap_or_default();
    if let Some(r) = &progress_row {
        let status: String = r.get("status");
        if status == "completed" {
            // already completed; idempotent no-op
            let score: Option<i64> = r.get("score");
            return Ok(Json(serde_json::json!({
                "lesson_id": id,
                "score": score.unwrap_or(0),
                "passed": score.unwrap_or(0) >= 60,
                "task_results": [],
                "progress": {"status":"completed", "attempts_count": 0},
                "next": {"lesson_id": serde_json::Value::Null},
                "schedule": {"due_at": null}
            })));
        }
    }

    // load tasks
    let tasks = sqlx::query("SELECT id,kind,answer_key,weight FROM lesson_tasks WHERE lesson_id=?")
        .bind(&id)
        .fetch_all(&st.db)
        .await?;
    let mut total_weight = 0i64;
    let mut earned = 0i64;
    let mut results = Vec::new();
    for t in &tasks {
        let tid: String = t.get("id");
        let kind: String = t.get("kind");
        let key: String = t.get("answer_key");
        let weight: i64 = t.get("weight");
        total_weight += weight;
        let submitted = req.task_answers.iter().find(|a| a.task_id == tid);
        let (correct, score) = if let Some(a) = submitted {
            grade(&kind, &a.answer, &key)
        } else {
            (false, 0)
        };
        if correct {
            earned += weight;
        }
        // record attempt
        sqlx::query("INSERT INTO attempts (id,user_id,task_id,submitted_answer,correct,score,created_at) VALUES (?,?,?,?,?,?,?)")
            .bind(uuid::Uuid::new_v4().to_string()).bind(&auth.user_id).bind(&tid)
            .bind(a_to_string(submitted.map(|x| &x.answer)).unwrap_or_default())
            .bind(correct as i64).bind(score).bind(chrono::Utc::now().to_rfc3339())
            .execute(&st.db).await?;
        results.push(serde_json::json!({"task_id": tid, "correct": correct, "score": score}));
    }
    let score = if total_weight > 0 {
        earned * 100 / total_weight
    } else {
        100
    };
    let passed = score >= 60;
    let ts = chrono::Utc::now().to_rfc3339();
    // update progress
    sqlx::query("INSERT OR REPLACE INTO lesson_progress (user_id,lesson_id,status,score,attempts_count,first_completed_at,last_activity_at,idempotency_key) VALUES (?,?,?,?,?,?,?,?)")
        .bind(&auth.user_id).bind(&id).bind(if passed {"completed"} else {"in_progress"})
        .bind(score).bind(1i64)
        .bind(if passed { Some(ts.clone()) } else { None })
        .bind(&ts).bind(&idem).execute(&st.db).await?;

    // schedule update simple skipped for MVP

    Ok(Json(serde_json::json!({
        "lesson_id": id,
        "score": score,
        "passed": passed,
        "task_results": results,
        "progress": {"status": if passed {"completed"} else {"in_progress"}, "attempts_count": 1},
        "next": {"lesson_id": null},
        "schedule": {"due_at": null, "interval_days": 1, "state": if passed {"review"} else {"relearn"}}
    })))
}

fn grade(kind: &str, answer: &serde_json::Value, key: &str) -> (bool, i64) {
    let ans = a_to_string(Some(answer)).unwrap_or_default();
    let norm = ans.trim().to_lowercase();
    let keyn = key.trim().to_lowercase();
    match kind {
        "mcq" | "multi" => {
            let c = norm == keyn;
            (c, if c { 1 } else { 0 })
        }
        _ => {
            // short/code: token compare (contains)
            let c = norm == keyn || (keyn.len() > 3 && norm.contains(&keyn));
            (c, if c { 1 } else { 0 })
        }
    }
}

fn a_to_string(v: Option<&serde_json::Value>) -> Option<String> {
    match v? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => Some(
            a.iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        other => Some(other.to_string()),
    }
}

async fn lesson_progress(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    use sqlx::Row;
    let row = sqlx::query("SELECT status,score,attempts_count,last_activity_at FROM lesson_progress WHERE user_id=? AND lesson_id=?")
        .bind(&auth.user_id).bind(&id).fetch_optional(&st.db).await?;
    match row {
        Some(r) => Ok(Json(serde_json::json!({
            "status": r.get::<String,_>("status"),
            "score": r.get::<Option<i64>,_>("score"),
            "attempts_count": r.get::<i64,_>("attempts_count"),
            "last_activity_at": r.get::<String,_>("last_activity_at"),
        }))),
        None => Ok(Json(serde_json::json!({"status":"not_started"}))),
    }
}
#[cfg(test)]
mod route_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grade_mcq_exact_case_insensitive() {
        let (c, s) = grade("mcq", &json!("B"), "b");
        assert!(c);
        assert_eq!(s, 1);
        let (c, s) = grade("mcq", &json!("  B  "), "b");
        assert!(c);
        assert_eq!(s, 1);
    }

    #[test]
    fn grade_mcq_wrong() {
        let (c, s) = grade("mcq", &json!("x"), "b");
        assert!(!c);
        assert_eq!(s, 0);
    }

    #[test]
    fn grade_short_exact_and_contains() {
        let (c, _) = grade("short", &json!("reschedule meeting"), "reschedule meeting");
        assert!(c);
        let (c, _) = grade(
            "short",
            &json!("Please reschedule meeting today"),
            "reschedule meeting",
        );
        assert!(c);
        let (c, _) = grade("short", &json!("nope"), "reschedule meeting");
        assert!(!c);
    }

    #[test]
    fn grade_short_key_short_no_contains() {
        let (c, _) = grade("short", &json!("ab contains ab"), "ab");
        assert!(!c);
        let (c, _) = grade("short", &json!("AB"), "ab");
        assert!(c);
    }

    #[test]
    fn grade_multi_exact_only() {
        let (c, _) = grade("multi", &json!("a,b"), "a,b");
        assert!(c);
        let (c, _) = grade("multi", &json!("a,b,c"), "a,b");
        assert!(!c);
    }

    #[test]
    fn grade_code_contains() {
        let (c, _) = grade("code", &json!("print(\"hello\")"), "hello");
        assert!(c);
    }
}
