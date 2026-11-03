use sqlx::SqlitePool;
use uuid::Uuid;

async fn ins_skill(pool: &SqlitePool, slug: &str, title: &str, desc: &str, cat: &str) {
    let id = Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO skills (id,slug,title,description,category) VALUES (?,?,?,?,?)",
    )
    .bind(&id)
    .bind(slug)
    .bind(title)
    .bind(desc)
    .bind(cat)
    .execute(pool)
    .await;
}

async fn ins_track(
    pool: &SqlitePool,
    slug: &str,
    title: &str,
    desc: &str,
    diff: &str,
    mins: i64,
    skill_slug: &str,
) -> String {
    let id = Uuid::new_v4().to_string();
    let _ = sqlx::query("INSERT OR IGNORE INTO tracks (id,slug,title,description,difficulty,estimated_minutes,published,version) VALUES (?,?,?,?,?,?,1,1)")
        .bind(&id).bind(slug).bind(title).bind(desc).bind(diff).bind(mins)
        .execute(pool).await;
    // get id
    let row: Option<(String,)> = sqlx::query_as("SELECT id FROM tracks WHERE slug=?")
        .bind(slug)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    let tid = row.map(|r| r.0).unwrap_or(id);
    // skill link
    if let Ok(Some((sid,))) = sqlx::query_as::<_, (String,)>("SELECT id FROM skills WHERE slug=?")
        .bind(skill_slug)
        .fetch_optional(pool)
        .await
    {
        let _ = sqlx::query("INSERT OR IGNORE INTO track_skills (track_id,skill_id) VALUES (?,?)")
            .bind(&tid)
            .bind(sid)
            .execute(pool)
            .await;
    }
    tid
}

#[allow(clippy::too_many_arguments)]
async fn ins_lesson(
    pool: &SqlitePool,
    track_id: &str,
    slug: &str,
    title: &str,
    order: i64,
    dur: i64,
    intro: &str,
    tasks: Vec<(&str, &str, &str, &str, i64)>,
) -> String {
    // tasks: (kind, prompt, options_json, answer_key, weight)
    let id = Uuid::new_v4().to_string();
    let content_tasks: Vec<serde_json::Value> = tasks.iter().enumerate().map(|(i,t)| {
        serde_json::json!({"id": format!("task-{}", i+1), "kind": t.0, "prompt": t.1, "options": if t.2.is_empty(){serde_json::Value::Null} else {serde_json::from_str(t.2).unwrap_or(serde_json::json!([]))}, "weight": t.4})
    }).collect();
    let content = serde_json::json!({
        "intro": intro,
        "blocks": [{"type":"text","value":intro}],
        "tasks": content_tasks
    });
    let cjson = content.to_string();
    // hash
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(cjson.as_bytes());
    let hash = hex::encode(h.finalize());
    let _ = sqlx::query("INSERT OR IGNORE INTO lessons (id,track_id,slug,title,order_index,duration_min,content_json,content_hash,version,published) VALUES (?,?,?,?,?,?,?, ?,1,1)")
        .bind(&id).bind(track_id).bind(slug).bind(title).bind(order).bind(dur).bind(&cjson).bind(&hash)
        .execute(pool).await;
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM lessons WHERE track_id=? AND slug=?")
            .bind(track_id)
            .bind(slug)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let lid = row.map(|r| r.0).unwrap_or(id);
    // tasks
    for (i, t) in tasks.iter().enumerate() {
        let tid = format!("{}-{}", &lid, i + 1);
        let _ = sqlx::query("INSERT OR IGNORE INTO lesson_tasks (id,lesson_id,kind,prompt,options_json,answer_key,weight) VALUES (?,?,?,?,?,?,?)")
            .bind(&tid).bind(&lid).bind(t.0).bind(t.1).bind(t.2).bind(t.3).bind(t.4)
            .execute(pool).await;
    }
    lid
}

pub async fn seed_all(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // skills
    for (s, t, d, c) in [
        (
            "english-work",
            "English for work",
            "Work English: emails, meetings, small talk",
            "languages",
        ),
        (
            "python-auto",
            "Python automation",
            "Automate daily tasks with Python",
            "programming",
        ),
        (
            "personal-finance",
            "Personal finance",
            "Budget, savings, ETFs, taxes",
            "finance",
        ),
        (
            "productivity",
            "Productivity & psychology",
            "Focus, habits, stress, deep work",
            "psychology",
        ),
    ] {
        ins_skill(pool, s, t, d, c).await;
    }

    // only seed if tracks empty
    let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
        .fetch_one(pool)
        .await?;
    if cnt.0 > 0 {
        return Ok(());
    }

    // Track 1
    let t1 = ins_track(
        pool,
        "english-work-track",
        "Английский для работы",
        "Письма, встречи, small talk за 15 минут",
        "beginner",
        60,
        "english-work",
    )
    .await;
    ins_lesson(pool,&t1,"intro-email","Вводное письмо клиенту",1,12,"Как написать короткое профессиональное письмо.", vec![
        ("mcq","Выберите наиболее профессиональное приветствие для клиента:","[{\"id\":\"a\",\"text\":\"Hi John,\"},{\"id\":\"b\",\"text\":\"Dear John,\"},{\"id\":\"c\",\"text\":\"Hey John!\"}]","b",1),
        ("short","Напишите строку темы письма для переноса встречи (одна фраза).","","reschedule meeting",1),
    ]).await;
    ins_lesson(pool,&t1,"meeting-smalltalk","Small talk на встрече",2,10,"Темы и фразы для small talk.", vec![
        ("mcq","Что лучше спросить в начале встречи с новым клиентом?","[{\"id\":\"a\",\"text\":\"How is business going?\"},{\"id\":\"b\",\"text\":\"What's your salary?\"},{\"id\":\"c\",\"text\":\"Are you married?\"}]","a",1),
    ]).await;
    ins_lesson(pool,&t1,"follow-up","Follow-up после встречи",3,14,"Итоги и next steps.", vec![
        ("mcq","Что обязательно включить в follow-up?","[{\"id\":\"a\",\"text\":\"Summary and next steps\"},{\"id\":\"b\",\"text\":\"Jokes\"},{\"id\":\"c\",\"text\":\"Personal secrets\"}]","a",1),
    ]).await;

    // Track 2
    let t2 = ins_track(
        pool,
        "python-auto-track",
        "Python для автоматизации",
        "Скрипты для рутины за 15 минут",
        "beginner",
        75,
        "python-auto",
    )
    .await;
    ins_lesson(pool,&t2,"files-rename","Переименование файлов",1,13,"os and pathlib basics.", vec![
        ("mcq","Какой модуль лучше для путей?","[{\"id\":\"a\",\"text\":\"pathlib\"},{\"id\":\"b\",\"text\":\"socket\"},{\"id\":\"c\",\"text\":\"asyncio\"}]","a",1),
        ("short","Функция для листинга директории?","","os.listdir",1),
    ]).await;
    ins_lesson(
        pool,
        &t2,
        "csv-clean",
        "Очистка CSV",
        2,
        15,
        "pandas/csv.",
        vec![(
            "mcq",
            "Что делает dropna?",
            "[{\"id\":\"a\",\"text\":\"удаляет пропуски\"},{\"id\":\"b\",\"text\":\"сортирует\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(pool,&t2,"schedule-script","Планировщик скриптов",3,12,"cron/Task Scheduler.", vec![
        ("mcq","Где настраивается автозапуск в Windows?","[{\"id\":\"a\",\"text\":\"Task Scheduler\"},{\"id\":\"b\",\"text\":\"Registry only\"}]","a",1),
    ]).await;

    // Track 3
    let t3 = ins_track(
        pool,
        "finance-track",
        "Личные финансы",
        "Бюджет и инвестиции",
        "beginner",
        60,
        "personal-finance",
    )
    .await;
    ins_lesson(pool,&t3,"budget-50-30-20","Правило 50/30/20",1,11,"Базовый бюджет.", vec![
        ("mcq","Сколько на нужды?","[{\"id\":\"a\",\"text\":\"50%\"},{\"id\":\"b\",\"text\":\"30%\"},{\"id\":\"c\",\"text\":\"20%\"}]","a",1),
    ]).await;
    ins_lesson(
        pool,
        &t3,
        "emergency-fund",
        "Подушка безопасности",
        2,
        10,
        "3-6 месяцев расходов.",
        vec![(
            "mcq",
            "Рекомендуемый размер?",
            "[{\"id\":\"a\",\"text\":\"3-6 месяцев\"},{\"id\":\"b\",\"text\":\"1 день\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t3,
        "etf-basics",
        "ETF основы",
        3,
        14,
        "Диверсификация.",
        vec![(
            "mcq",
            "Что такое ETF?",
            "[{\"id\":\"a\",\"text\":\"фонд~корзина\"},{\"id\":\"b\",\"text\":\"одна акция\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Track 4
    let t4 = ins_track(
        pool,
        "productivity-track",
        "Продуктивность и фокус",
        "Привычки и deep work",
        "beginner",
        50,
        "productivity",
    )
    .await;
    ins_lesson(
        pool,
        &t4,
        "deep-work",
        "Deep work",
        1,
        12,
        "Фокус-блоки.",
        vec![(
            "mcq",
            "Минимальный блок фокуса?",
            "[{\"id\":\"a\",\"text\":\"25-50 мин\"},{\"id\":\"b\",\"text\":\"2 мин\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t4,
        "habit-loop",
        "Петля привычки",
        2,
        11,
        "Триггер-действие-награда.",
        vec![(
            "mcq",
            "Что формирует привычку?",
            "[{\"id\":\"a\",\"text\":\"повторение\"},{\"id\":\"b\",\"text\":\"желание\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t4,
        "stress-regulation",
        "Регуляция стресса",
        3,
        13,
        "Дыхание и паузы.",
        vec![(
            "mcq",
            "Дыхание 4-7-8 помогает?",
            "[{\"id\":\"a\",\"text\":\"да\"},{\"id\":\"b\",\"text\":\"нет\"}]",
            "a",
            1,
        )],
    )
    .await;

    Ok(())
}
