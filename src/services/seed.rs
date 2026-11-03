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
        (
            "computer-basics",
            "Компьютер и интернет",
            "Устройство компьютера, файлы, браузер, безопасность для новичков",
            "digital-literacy",
        ),
        (
            "internet-safety",
            "Безопасность в интернете",
            "Пароли, фишинг, личные данные, мошенники",
            "digital-literacy",
        ),
        (
            "math-basics",
            "Математика для жизни",
            "Проценты, скидки, бюджет, простая статистика",
            "math",
        ),
        (
            "study-skills",
            "Как учиться",
            "Память, повторение, фокус, постановка целей",
            "psychology",
        ),
        (
            "english-zero",
            "Английский с нуля",
            "Алфавит, числа, базовые фразы и приветствия",
            "languages",
        ),
        (
            "python-zero",
            "Основы Python",
            "Переменные, условия, циклы, функции",
            "programming",
        ),
        (
            "spreadsheets",
            "Таблицы Excel/Google Sheets",
            "Ячейки, формулы, сортировка, фильтры",
            "productivity",
        ),
    ] {
        ins_skill(pool, s, t, d, c).await;
    }

    // Seed is idempotent: INSERT OR IGNORE keeps existing rows intact while
    // allowing new tracks/lessons to be added in later versions.

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

    // Track 5: computer basics from zero
    let t5 = ins_track(
        pool,
        "computer-basics-track",
        "Компьютер и интернет с нуля",
        "Устройство ПК, файлы, браузер, безопасные привычки",
        "beginner",
        45,
        "computer-basics",
    )
    .await;
    ins_lesson(
        pool,
        &t5,
        "computer-parts",
        "Из чего состоит компьютер",
        1,
        10,
        "Основные части компьютера и их роли.",
        vec![
            (
                "mcq",
                "Какая часть выполняет программы?",
                "[{\"id\":\"a\",\"text\":\"Процессор\"},{\"id\":\"b\",\"text\":\"Монитор\"}]",
                "a",
                1,
            ),
            (
                "mcq",
                "Для хранения файлов нужен...",
                "[{\"id\":\"a\",\"text\":\"Накопитель\"},{\"id\":\"b\",\"text\":\"Колонки\"}]",
                "a",
                1,
            ),
        ],
    )
    .await;
    ins_lesson(
        pool,
        &t5,
        "files-folders",
        "Файлы и папки",
        2,
        8,
        "Как организовать файлы в папках.",
        vec![(
            "mcq",
            "Что помогает найти файл быстрее?",
            "[{\"id\":\"a\",\"text\":\"Понятные имена и папки\"},{\"id\":\"b\",\"text\":\"Случайные названия\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t5,
        "browser-basics",
        "Браузер и адресная строка",
        3,
        9,
        "Как открывать сайты и проверять адрес.",
        vec![(
            "mcq",
            "Где вводить адрес сайта?",
            "[{\"id\":\"a\",\"text\":\"В адресной строке\"},{\"id\":\"b\",\"text\":\"В поисковой выдаче всегда\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Track 6: internet safety
    let t6 = ins_track(
        pool,
        "internet-safety-track",
        "Безопасность в интернете с нуля",
        "Пароли, фишинг, личные данные",
        "beginner",
        40,
        "internet-safety",
    )
    .await;
    ins_lesson(
        pool,
        &t6,
        "passwords",
        "Надёжные пароли",
        1,
        9,
        "Как придумывать и хранить пароли.",
        vec![(
            "mcq",
            "Что делает пароль надёжнее?",
            "[{\"id\":\"a\",\"text\":\"Длина и разные символы\"},{\"id\":\"b\",\"text\":\"Имя и дата рождения\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t6,
        "phishing",
        "Фишинг и поддельные сайты",
        2,
        10,
        "Как распознать обман.",
        vec![(
            "mcq",
            "Вы получили письмо с просьбой срочно ввести пароль. Что делать?",
            "[{\"id\":\"a\",\"text\":\"Проверить адрес отправителя и не торопиться\"},{\"id\":\"b\",\"text\":\"Сразу ввести пароль\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t6,
        "personal-data",
        "Личные данные",
        3,
        8,
        "Что можно публиковать, а что нет.",
        vec![(
            "mcq",
            "Можно ли публиковать фото паспорта?",
            "[{\"id\":\"a\",\"text\":\"Нет\"},{\"id\":\"b\",\"text\":\"Да, если красивое\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Track 7: math basics
    let t7 = ins_track(
        pool,
        "math-everyday-track",
        "Математика для жизни с нуля",
        "Проценты, скидки, бюджет",
        "beginner",
        42,
        "math-basics",
    )
    .await;
    ins_lesson(
        pool,
        &t7,
        "percent",
        "Проценты",
        1,
        10,
        "Как считать процент от числа.",
        vec![(
            "mcq",
            "10% от 200 равно...",
            "[{\"id\":\"a\",\"text\":\"20\"},{\"id\":\"b\",\"text\":\"2000\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t7,
        "discount",
        "Скидки",
        2,
        9,
        "Как посчитать цену со скидкой.",
        vec![(
            "mcq",
            "Цена 1000, скидка 20%. Итоговая цена...",
            "[{\"id\":\"a\",\"text\":\"800\"},{\"id\":\"b\",\"text\":\"1200\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t7,
        "budget",
        "Простой бюджет",
        3,
        10,
        "Доходы и расходы.",
        vec![(
            "mcq",
            "Если расходов больше доходов, что нужно сделать?",
            "[{\"id\":\"a\",\"text\":\"Сократить расходы или увеличить доход\"},{\"id\":\"b\",\"text\":\"Игнорировать\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Track 8: study skills
    let t8 = ins_track(
        pool,
        "study-skills-track",
        "Как учиться эффективно с нуля",
        "Память, повторение, фокус",
        "beginner",
        39,
        "study-skills",
    )
    .await;
    ins_lesson(
        pool,
        &t8,
        "pomodoro",
        "Техника Помодоро",
        1,
        8,
        "Короткие блоки фокуса и перерывы.",
        vec![(
            "mcq",
            "Типичная длина блока фокуса по Помодоро...",
            "[{\"id\":\"a\",\"text\":\"25 минут\"},{\"id\":\"b\",\"text\":\"2 часа\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t8,
        "spaced-repetition",
        "Интервальные повторения",
        2,
        9,
        "Как повторять, чтобы запоминать.",
        vec![(
            "mcq",
            "Что эффективнее для памяти?",
            "[{\"id\":\"a\",\"text\":\"Повторять с увеличением интервалов\"},{\"id\":\"b\",\"text\":\"Прочитать один раз\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t8,
        "goals",
        "Постановка целей",
        3,
        8,
        "Конкретные и измеримые цели.",
        vec![(
            "mcq",
            "Хорошая учебная цель должна быть...",
            "[{\"id\":\"a\",\"text\":\"Конкретной и измеримой\"},{\"id\":\"b\",\"text\":\"Размытой\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Track 9: English zero
    let t9 = ins_track(
        pool,
        "english-zero-track",
        "Английский с нуля",
        "Алфавит, числа, базовые фразы",
        "beginner",
        50,
        "english-zero",
    )
    .await;
    ins_lesson(
        pool,
        &t9,
        "alphabet",
        "Алфавит и звуки",
        1,
        10,
        "Буквы и произношение.",
        vec![(
            "mcq",
            "Сколько букв в английском алфавите?",
            "[{\"id\":\"a\",\"text\":\"26\"},{\"id\":\"b\",\"text\":\"33\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t9,
        "greetings-zero",
        "Приветствия",
        2,
        9,
        "Базовые приветствия и ответы.",
        vec![(
            "mcq",
            "Как поздороваться днём?",
            "[{\"id\":\"a\",\"text\":\"Good afternoon\"},{\"id\":\"b\",\"text\":\"Good night\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t9,
        "numbers-zero",
        "Числа от 1 до 20",
        3,
        9,
        "Чтение и написание чисел.",
        vec![(
            "mcq",
            "Как пишется число 7?",
            "[{\"id\":\"a\",\"text\":\"seven\"},{\"id\":\"b\",\"text\":\"seventy\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Track 10: Python zero
    let t10 = ins_track(
        pool,
        "python-zero-track",
        "Основы Python с нуля",
        "Переменные, условия, циклы, функции",
        "beginner",
        60,
        "python-zero",
    )
    .await;
    ins_lesson(
        pool,
        &t10,
        "variables",
        "Переменные",
        1,
        10,
        "Как хранить данные в переменных.",
        vec![(
            "mcq",
            "Как создать переменную x со значением 5?",
            "[{\"id\":\"a\",\"text\":\"x = 5\"},{\"id\":\"b\",\"text\":\"5 = x\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t10,
        "conditions",
        "Условия if",
        2,
        11,
        "Принятие решений в коде.",
        vec![(
            "mcq",
            "Что проверяет выражение if x > 5?",
            "[{\"id\":\"a\",\"text\":\"Больше ли x чем 5\"},{\"id\":\"b\",\"text\":\"Равен ли x пяти\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t10,
        "loops",
        "Циклы",
        3,
        12,
        "Повторение действий.",
        vec![(
            "mcq",
            "Какой цикл повторяет, пока условие истинно?",
            "[{\"id\":\"a\",\"text\":\"while\"},{\"id\":\"b\",\"text\":\"if\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Track 11: Spreadsheets basics
    let t11 = ins_track(
        pool,
        "spreadsheets-basics-track",
        "Таблицы Excel/Google Sheets с нуля",
        "Ячейки, формулы, сортировка, фильтры",
        "beginner",
        48,
        "spreadsheets",
    )
    .await;
    ins_lesson(
        pool,
        &t11,
        "cells-refs",
        "Ячейки и ссылки",
        1,
        10,
        "Как адресовать ячейки.",
        vec![(
            "mcq",
            "Что такое A1?",
            "[{\"id\":\"a\",\"text\":\"Ячейка в столбце A, строке 1\"},{\"id\":\"b\",\"text\":\"Функция\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t11,
        "formulas",
        "Формулы SUM и AVERAGE",
        2,
        10,
        "Базовые формулы.",
        vec![(
            "mcq",
            "Как сложить диапазон A1:A5?",
            "[{\"id\":\"a\",\"text\":\"=SUM(A1:A5)\"},{\"id\":\"b\",\"text\":\"=ADD(A1:A5)\"}]",
            "a",
            1,
        )],
    )
    .await;
    ins_lesson(
        pool,
        &t11,
        "sort-filter",
        "Сортировка и фильтры",
        3,
        9,
        "Как находить нужные данные.",
        vec![(
            "mcq",
            "Фильтр позволяет...",
            "[{\"id\":\"a\",\"text\":\"Показать только нужные строки\"},{\"id\":\"b\",\"text\":\"Удалить все данные\"}]",
            "a",
            1,
        )],
    )
    .await;

    // Multi-month daily courses: ~15 minutes per day.
    let long_programs = [
        (
            "english-monthly-track",
            "Английский за 2 месяца (15 минут в день)",
            "Ежедневные мини-уроки: грамматика, слова и фразы",
            "english-zero",
            60,
            "English Day",
        ),
        (
            "python-monthly-track",
            "Python за 2 месяца (15 минут в день)",
            "Ежедневные упражнения по Python для начинающих",
            "python-zero",
            60,
            "Python Day",
        ),
        (
            "computer-monthly-track",
            "Компьютерная грамотность за 2 месяца (15 минут в день)",
            "Ежедневные практические советы для уверенной работы",
            "computer-basics",
            60,
            "Computer Day",
        ),
    ];

    for (slug, title, desc, skill_slug, days, prefix) in long_programs {
        let tid = ins_track(
            pool,
            slug,
            title,
            desc,
            "beginner",
            (days * 15) as i64,
            skill_slug,
        )
        .await;
        for day in 1..=days {
            let lesson_title = format!("{} {}", prefix, day);
            let lesson_slug = format!("day-{:02}", day);
            ins_lesson(
                pool,
                &tid,
                &lesson_slug,
                &lesson_title,
                day as i64,
                15,
                "Короткий урок на 15 минут. Выполните задание и переходите к следующему дню.",
                vec![(
                    "mcq",
                    "Готовы продолжить обучение завтра?",
                    "[{\"id\":\"a\",\"text\":\"Да, продолжу\"},{\"id\":\"b\",\"text\":\"Нет\"}]",
                    "a",
                    1,
                )],
            )
            .await;
        }
    }

    Ok(())
}
