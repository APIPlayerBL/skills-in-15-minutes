use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

fn content_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

pub async fn seed_all(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    const CURRICULUM: &str = include_str!("../../content/curriculum.json");
    let data: Value = serde_json::from_str(CURRICULUM).expect("curriculum.json");

    if let Some(skills) = data.get("skills").and_then(|v| v.as_array()) {
        for skill in skills {
            let slug = skill["slug"].as_str().unwrap_or_default();
            let title = skill["title"].as_str().unwrap_or_default();
            let desc = skill["description"].as_str().unwrap_or_default();
            let category = skill["category"].as_str().unwrap_or("general");
            let id = Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO skills (id, slug, title, description, category) VALUES (?,?,?,?,?)",
            )
            .bind(&id)
            .bind(slug)
            .bind(title)
            .bind(desc)
            .bind(category)
            .execute(pool)
            .await;
        }
    }

    if let Some(tracks) = data.get("tracks").and_then(|v| v.as_array()) {
        for track in tracks {
            let slug = track["slug"].as_str().unwrap_or_default();
            let title = track["title"].as_str().unwrap_or_default();
            let desc = track["description"].as_str().unwrap_or_default();
            let difficulty = track["difficulty"].as_str().unwrap_or("beginner");
            let minutes = track["estimated_minutes"].as_i64().unwrap_or(15);
            let skill_slug = track["skill"].as_str().unwrap_or_default();
            let skill_id: Option<String> =
                sqlx::query_scalar("SELECT id FROM skills WHERE slug=?")
                    .bind(skill_slug)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();

            let track_uuid = Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO tracks (id, slug, title, description, difficulty, estimated_minutes, published, version) VALUES (?,?,?,?,?,?,1,1)",
            )
            .bind(&track_uuid)
            .bind(slug)
            .bind(title)
            .bind(desc)
            .bind(difficulty)
            .bind(minutes)
            .execute(pool)
            .await;

            let _ = sqlx::query(
                "UPDATE tracks SET title=?, description=?, difficulty=?, estimated_minutes=?, published=1 WHERE slug=?",
            )
            .bind(title)
            .bind(desc)
            .bind(difficulty)
            .bind(minutes)
            .bind(slug)
            .execute(pool)
            .await;

            let track_id: String = sqlx::query_scalar("SELECT id FROM tracks WHERE slug=?")
                .bind(slug)
                .fetch_one(pool)
                .await?;

            if let Some(sid) = skill_id {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO track_skills (track_id, skill_id) VALUES (?,?)",
                )
                .bind(&track_id)
                .bind(sid)
                .execute(pool)
                .await;
            }

            if let Some(lessons) = track.get("lessons").and_then(|v| v.as_array()) {
                for (order, lesson) in lessons.iter().enumerate() {
                    let lesson_slug = lesson["slug"].as_str().unwrap_or_default();
                    let lesson_title = lesson["title"].as_str().unwrap_or_default();
                    let dur = lesson["minutes"].as_i64().unwrap_or(15).clamp(5, 20);
                    let intro = lesson["intro"].as_str().unwrap_or_default();

                    let tasks_json: Vec<Value> = lesson
                        .get("tasks")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .enumerate()
                                .map(|(i, t)| {
                                    serde_json::json!({
                                        "id": format!("task-{}", i + 1),
                                        "kind": t.get("kind").and_then(|x| x.as_str()).unwrap_or("mcq"),
                                        "prompt": t.get("prompt").and_then(|x| x.as_str()).unwrap_or_default(),
                                        "options": t.get("options").cloned().unwrap_or(Value::Null),
                                        "weight": t.get("weight").and_then(|x| x.as_i64()).unwrap_or(1),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let content = serde_json::json!({
                        "intro": intro,
                        "blocks": [{"type": "text", "value": intro}],
                        "tasks": tasks_json,
                    });
                    let cjson = content.to_string();
                    let hash = content_hash(&cjson);

                    let existing: Option<String> = sqlx::query_scalar(
                        "SELECT id FROM lessons WHERE track_id=? AND slug=?",
                    )
                    .bind(&track_id)
                    .bind(lesson_slug)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();

                    let lesson_id = if let Some(id) = existing {
                        let _ = sqlx::query(
                            "UPDATE lessons SET title=?, duration_min=?, content_json=?, content_hash=?, published=1, order_index=? WHERE id=?",
                        )
                        .bind(lesson_title)
                        .bind(dur)
                        .bind(&cjson)
                        .bind(&hash)
                        .bind((order + 1) as i64)
                        .bind(&id)
                        .execute(pool)
                        .await;
                        id
                    } else {
                        let id = Uuid::new_v4().to_string();
                        let _ = sqlx::query(
                            "INSERT INTO lessons (id, track_id, slug, title, order_index, duration_min, content_json, content_hash, version, published) VALUES (?,?,?,?,?,?,?, ?,1,1)",
                        )
                        .bind(&id)
                        .bind(&track_id)
                        .bind(lesson_slug)
                        .bind(lesson_title)
                        .bind((order + 1) as i64)
                        .bind(dur)
                        .bind(&cjson)
                        .bind(&hash)
                        .execute(pool)
                        .await;
                        id
                    };

                    if let Some(tasks) = lesson.get("tasks").and_then(|v| v.as_array()) {
                        for (i, t) in tasks.iter().enumerate() {
                            let task_id = format!("{}-{}", lesson_id, i + 1);
                            let kind = t.get("kind").and_then(|x| x.as_str()).unwrap_or("mcq");
                            let prompt = t.get("prompt").and_then(|x| x.as_str()).unwrap_or_default();
                            let options = t.get("options").cloned().unwrap_or(Value::Null);
                            let options_json = serde_json::to_string(&options).ok();
                            let answer = t.get("answer").and_then(|x| x.as_str()).unwrap_or_default();
                            let weight = t.get("weight").and_then(|x| x.as_i64()).unwrap_or(1);

                            let exists: Option<i64> = sqlx::query_scalar(
                                "SELECT 1 FROM lesson_tasks WHERE id=?",
                            )
                            .bind(&task_id)
                            .fetch_optional(pool)
                            .await
                            .ok()
                            .flatten();
                            let exists = exists.is_some();

                            if exists {
                                let _ = sqlx::query(
                                    "UPDATE lesson_tasks SET kind=?, prompt=?, options_json=?, answer_key=?, weight=? WHERE id=?",
                                )
                                .bind(kind)
                                .bind(prompt)
                                .bind(&options_json)
                                .bind(answer)
                                .bind(weight)
                                .bind(&task_id)
                                .execute(pool)
                                .await;
                            } else {
                                let _ = sqlx::query(
                                    "INSERT INTO lesson_tasks (id, lesson_id, kind, prompt, options_json, answer_key, weight) VALUES (?,?,?,?,?,?,?)",
                                )
                                .bind(&task_id)
                                .bind(&lesson_id)
                                .bind(kind)
                                .bind(prompt)
                                .bind(&options_json)
                                .bind(answer)
                                .bind(weight)
                                .execute(pool)
                                .await;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
