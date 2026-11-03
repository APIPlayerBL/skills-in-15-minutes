# Навыки за 15 минут — Architecture

**Project:** Microlearning platform (Rust/Axum backend, SQLite storage, vanilla JS frontend)  
**Owner:** Planner (Planner/Architect Agent)  
**Status:** Draft v0.1 — ready for review  
**Working dir:** `C:\Users\ivan\Desktop\гит\портфолио\Навыки за 15 минут` (greenfield; codebase-analyst confirmed empty)

---

## 1. Обзор и цели

### 1.1 Видение

Платформа **«Навыки за 15 минут»** доставляет короткие (10–15 минут) микроуроки по востребованным навыкам с интерактивными заданиями, адаптивным расписанием, офлайн-доступом и сертификатами по завершении трека.

### 1.2 Функциональные цели (MVP)

- Каталог **треков** и **уроков** (10–15 мин) с последовательностью.
- Регистрация/вход пользователя, профиль, прогресс.
- Интерактивные задания (тест/множественный выбор/короткий ответ/код-сниппет) с проверкой.
- Адаптивное расписание: пропуск → урок переносится, нагрузка пересчитывается, без «долга».
- Офлайн-пакет на 7 дней (загрузка уроков и ассетов) с последующей синхронизацией.
- Сертификат (запись + печатная страница) по завершении трека.
- Админ-контент: CRUD уроков/треков (ролевая модель).
- (Опционально) монетизация-мок: тарифы/корпоративные лицензии как поля.

### 1.3 Не-цели (v1)

- Видеостриминг/живые вебинары.
- Нативные мобильные приложения (только PWA/веб).
- Real-time multiplayer / leaderboard сервер push.
- Сложный drag-and-drop редактор контента (только JSON/Markdown-импорт).

### 1.4 Критерии успеха (MVP)

- Время ответа API p95 < 200 ms при 50 RPS на SQLite (одна нода).
- Полный цикл «урок → проверка → следующий урок» < 3 кликов.
- Офлайн-пакет скачивается одной транзакцией, проверяется по хешам.
- 80%+ покрытие доменной логики тестами.

---

## 2. ADR (Architecture Decision Records)

### ADR-001: Rust + Axum как backend

- **Решение:** Rust 1.75+, Axum 0.7, Tokio.
- **Причины:** производительность, безопасность памяти, один бинарник, статическая отдача фронта, экосистема (serde, sqlx, tower, tracing).
- **Альтернативы:** Node/Express (быстрее старт, но ниже throughput и больше деплой-complexity), Python/FastAPI (скорость разработки, но GIL и упаковка).
- **Последствия:** выше порог входа; нужна строгая структура модулей; время сборки ~20–40s.

### ADR-002: Хранение — SQLite (sqlx) + JSON-сида контента

- **Решение:** SQLite (WAL mode) через sqlx (runtime queries или макросы). Контент уроков может жить в таблицах `lessons.content_json`, но авторинг — JSON-файлы в `content/`, импортируемые миграцией/CLI.
- **Причины:** один файл БД, простой бэкап, достаточно для MVP; будущее — Postgres (sqlx абстрагирует).
- **Альтернативы:** чистый JSON на диске (просто, но нет транзакций/индексов progress), Postgres (overkill для портфолио/локального).
- **Последствия:** SQLite единственный writer (WAL допускает читателей); горизонтальное масштабирование ограничено. Миграции через `sqlx migrate`.

### ADR-003: Фронтенд — ванильный JS, статика из Axum

- **Решение:** HTML/CSS/JS без сборщика (или Vite только для dev, прод — статические файлы в `static/` → `tower_http::ServeDir`). PWA через Service Worker + Cache API.
- **Причины:** простота, нет Node в проде, согласуется с портфолио-стилем; офлайн-стратегия явная.
- **Альтернативы:** React/Vue (лучший DX, но сборка и бандл), SPA-роутинг на сервере.
- **Последствия:** ручное управление состоянием; дизайн-компоненты пишем сами.

### ADR-004: REST JSON API, версия `/api/v1`

- **Решение:** ресурсные эндпоинты, `snake_case` JSON, пагинация `?limit=&offset=`, ошибки в формате RFC 7807 (`application/problem+json`).
- **Причины:** простота, кеширование HTTP, совместимость с фронтом и возможным будущим клиентом.
- **Альтернативы:** GraphQL (overkill), gRPC (нет для браузера без прокси).

### ADR-005: Аутентификация — JWT (access 15m) + refresh в httpOnly cookie

- **Решение:** access token в памяти фронта (Bearer), refresh token в `httpOnly, Secure, SameSite=Lax` cookie с ротацией и reuse-detection. Дополнительно `Authorization: Bearer` для офлайн-компаньона.
- **Причины:** защита от XSS-утечки refresh; простая инвалидация.
- **Последствия:** CSRF-mitigation нужен (SameSite + CSRF-токен для unsafe методов при cookie-потоке). Офлайн-синк использует access token.

### ADR-006: Офлайн-доступ

- **Решение:** сервер формирует `bundle manifest` (JSON + список файлов + content-hash) на неделю; клиент (Service Worker) скачивает в Cache Storage и IndexedDB (прогресс/попытки). Синхронизация при появлении сети: queue of pending attempts → `POST /sync`.
- **Причины:** явный контроль версий контента, безопасная повторная загрузка.
- **Альтернативы:** Workbox (удобнее, но зависимость); полная IndexedDB-реплика БД (тяжело).

### ADR-007: Адаптивное расписание

- **Решение:** сервер хранит `schedule` (due-date per lesson) и алгоритм SM-2-lite (ease, interval, lapses). При пропуске дедлайна — пересчёт окна и понижение difficulty; без «долга» больше N в день. Логика — чистая функция в домене (тестируется property-based).
- **Причины:** ценность продукта, централизованная консистентность.
- **Альтернативы:** расписание только на клиенте (риск рассинхрона между устройствами).

### ADR-008: Сертификат

- **Решение:** v1 — запись `certificates` + печатная HTML-страница (QR-код с verify-URL). PDF — Phase 2 (printpdf или внешний рендер).
- **Причины:** меньше зависимостей, быстрая проверка.
- **Альтернативы:** PDF сразу через headless Chrome (тяжёлая инфра).

### ADR-009: Наблюдаемость

- **Решение:** `tracing` + `tracing-subscriber` (JSON в prod), метрики Prometheus через `metrics` (опционально), health/readiness эндпоинты.
- **Причины:** отладка и эксплуатация.

---

## 3. Контекст системы

```text
┌─────────────┐     HTTPS     ┌──────────────────────┐
│  Browser /  │◀─────────────▶│  Axum (single binary) │
│  PWA (SW)   │   REST JSON   │  ┌────────────────┐  │
└─────────────┘               │  │ Auth / JWT     │  │
       │                      │  │ Lessons/Progress│  │
       │ Cache/IndexedDB      │  │ Scheduler       │  │
       ▼                      │  │ Certificates    │  │
  offline bundle             │  └────────┬───────┘  │
                              │           │          │
                              │  ┌────────▼───────┐  │
                              │  │ SQLite (WAL)    │  │
                              │  │ + content/ JSON  │  │
                              │  └────────────────┘  │
                              └──────────────────────┘
```

---

## 4. Компоненты и слои

```text
src/
├── main.rs              # bootstrap, config, router
├── config.rs            # env/config struct
├── state.rs             # AppState (db pool, jwt keys, content)
├── error.rs             # AppError → HTTP (RFC7807)
├── routes/              # HTTP handlers (thin)
│   ├── auth.rs
│   ├── users.rs
│   ├── tracks.rs
│   ├── lessons.rs
│   ├── progress.rs
│   ├── schedule.rs
│   ├── bundles.rs
│   ├── certificates.rs
│   └── admin.rs
├── services/            # use-cases / application layer
│   ├── auth_service.rs
│   ├── progress_service.rs
│   ├── scheduler.rs     # pure-ish domain logic
│   ├── bundle_service.rs
│   └── certificate_service.rs
├── domain/              # entities, value objects, pure logic
│   ├── user.rs
│   ├── lesson.rs
│   ├── attempt.rs
│   ├── schedule.rs      # SM-2-lite
│   └── certificate.rs
├── repo/                # sqlx queries
│   ├── users.rs
│   ├── content.rs
│   ├── progress.rs
│   ├── schedules.rs
│   └── certificates.rs
├── middleware/          # auth, rate limit, tracing
└── static assets served via ServeDir

migrations/              # sqlx migrations
content/                 # seed JSON lessons/tracks
static/                  # frontend (html/css/js/sw.js)
tests/                   # integration tests
docs/                    # this file + api.md
```

**Правила зависимостей:** routes → services → domain; repo зависит от domain; domain не зависит от инфраструктуры. Services могут использовать repo traits для тестируемости.

---

## 5. Модель данных (SQLite)

```sql
-- Пользователи
CREATE TABLE users (
  id TEXT PRIMARY KEY,            -- uuid v7
  email TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,     -- argon2id
  display_name TEXT,
  role TEXT NOT NULL DEFAULT 'user', -- user | editor | admin
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE refresh_tokens (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT,
  replaced_by TEXT
);

-- Контент
CREATE TABLE tracks (
  id TEXT PRIMARY KEY,
  slug TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  difficulty TEXT NOT NULL,        -- beginner/intermediate/advanced
  estimated_minutes INTEGER NOT NULL,
  published INTEGER NOT NULL DEFAULT 0,
  version INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE skills (
  id TEXT PRIMARY KEY,
  slug TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  category TEXT NOT NULL
);

CREATE TABLE track_skills (
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  PRIMARY KEY (track_id, skill_id)
);

CREATE TABLE lessons (
  id TEXT PRIMARY KEY,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  order_index INTEGER NOT NULL,
  duration_min INTEGER NOT NULL CHECK (duration_min BETWEEN 5 AND 20),
  content_json TEXT NOT NULL,     -- {blocks:[...], tasks:[...]}
  content_hash TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  published INTEGER NOT NULL DEFAULT 0,
  UNIQUE (track_id, slug)
);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  lesson_id TEXT NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,            -- mcq | multi | short | code
  prompt TEXT NOT NULL,
  options_json TEXT,             -- for mcq/multi
  answer_key TEXT NOT NULL,      -- hashed/normalized expected
  weight INTEGER NOT NULL DEFAULT 1
);

-- Прогресс
CREATE TABLE enrollments (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  PRIMARY KEY (user_id, track_id)
);

CREATE TABLE lesson_progress (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  lesson_id TEXT NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
  status TEXT NOT NULL,          -- not_started | in_progress | completed | skipped
  score INTEGER,                 -- 0..100
  attempts_count INTEGER NOT NULL DEFAULT 0,
  first_completed_at TEXT,
  last_activity_at TEXT NOT NULL,
  PRIMARY KEY (user_id, lesson_id)
);

CREATE TABLE attempts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  submitted_answer TEXT NOT NULL,
  correct INTEGER NOT NULL,
  score INTEGER NOT NULL,
  created_at TEXT NOT NULL
);

-- Расписание
CREATE TABLE schedules (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  lesson_id TEXT NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
  due_at TEXT NOT NULL,
  interval_days INTEGER NOT NULL DEFAULT 1,
  ease REAL NOT NULL DEFAULT 2.5,
  lapses INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL,           -- new | learning | review | relearn
  updated_at TEXT NOT NULL,
  UNIQUE (user_id, lesson_id)
);

-- Офлайн-пакеты
CREATE TABLE bundles (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  manifest_json TEXT NOT NULL,   -- {files:[{path,hash,size}], content_version}
  status TEXT NOT NULL            -- pending | ready | consumed
);

-- Сертификаты
CREATE TABLE certificates (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  issued_at TEXT NOT NULL,
  verify_token TEXT UNIQUE NOT NULL,
  metadata_json TEXT NOT NULL,    -- name, score, hours
  UNIQUE (user_id, track_id)
);

-- Версии контента
CREATE TABLE content_versions (
  name TEXT PRIMARY KEY,
  version INTEGER NOT NULL,
  hash TEXT NOT NULL,
  applied_at TEXT NOT NULL
);
```

Индексы: `schedules(user_id, due_at)`, `lesson_progress(user_id,status)`, `lessons(track_id,order_index)`, `attempts(user_id,task_id)`.

---

## 6. Потоки

### 6.1 Завершение урока

1. `POST /api/v1/lessons/{id}/complete` с `{task_answers:[{task_id,answer}]}`.
2. Проверка JWT, загрузка урока/заданий, оценка ответов (нормализация + hash compare).
3. Обновление `attempts`, пересчёт `score`, `lesson_progress.status=completed`, `first_completed_at`.
4. Обновление `schedule` (SM-2-lite: успех → interval↑, ease+; провал → relearn).
5. Возврат `{score, next_lesson_id, next_due_at}`.

### 6.2 Адаптивное расписание

- Планировщик (background? on-demand): для пользователя выбрать уроки с `due_at <= today`, ограничить N/день (настройка, напр. 3), недопускать «долга»: просроченные → redistribute на ближайшие дни с cap.
- Если пропуск > 3 дней → снижение сложности: interval сброс, ease-0.1, статус relearn.

### 6.3 Офлайн

1. `POST /api/v1/bundles` → создание манифеста на 7 дней для выбранных треков.
2. `GET /api/v1/bundles/{id}/download` → zip (или JSON+assets) со всеми уроками + хеши.
3. SW сохраняет в Cache Storage; прогресс/попытки в IndexedDB.
4. При сети: `POST /api/v1/sync` с пачкой событий (idempotency-key по event id) → сервер применяет, возвращает мердж.

### 6.4 Сертификат

1. Проверка `enrollments.completed_at` и все уроки track завершены.
2. Создание `certificates` с `verify_token`.
3. `GET /api/v1/certificates/{id}` → HTML printable + QR.
4. `GET /api/v1/verify/{token}` → публичная проверка.

---

## 7. Производительность

- SQLite WAL; один writer, пул читателей (sqlx). Busy timeout 5s.
- Индексы (см. выше). Все списки — пагинация; дефолт 20, max 100.
- Статика: gzip/brotli через `tower-http::CompressionLayer`, ETag/If-None-Match, long cache для hashed assets.
- Контент уроков: lazy load, `content_hash` для клиента.
- Кеш: in-memory (moka) для каталога и контента по версии; инвалидация по `content_version`.
- Избегаем N+1: JOIN в repo, проекции.

---

## 8. Безопасность

- Пароли: argon2id; минимальная длина 10.
- JWT: RS256 (ключи из env) или HS256 с секретом ≥32 bytes; access 15m.
- Refresh: httpOnly cookie, Secure, SameSite=Lax, ротация, reuse-detection (отзыв всех токенов пользователя при reuse).
- CSRF: double-submit CSRF token для cookie-потоков; SameSite.
- RBAC: `user/editor/admin`; admin-маршруты отдельный router с middleware.
- Валидация: строгие DTO (serde deny_unknown_fields где уместно), длина, enum-проверки.
- Rate limit: tower-governor на auth и sync.
- Заголовки: HSTS, X-Content-Type-Options, CSP (script-src self; style self 'unsafe-inline' только dev), Referrer-Policy.
- SQL: только параметризованные запросы (sqlx). Нет eval пользовательского кода.
- Секреты: `.env` не коммитится; prod через env/secret manager.
- Зависимости: `cargo audit` в CI.

---

## 9. Тестируемость

- **Unit:** домен (`schedule::sm2_update`, scoring, normalization) — чистые функции; property-based (proptest) для интервалов/дат.
- **Integration:** Axum router + временная SQLite (tempfile); тестирование маршрутов с auth фикстурами; миграции применяются в test setup.
- **Contract:** snapshot OpenAPI? v1 может использовать `utoipa` для генерации OpenAPI из аннотаций; тест схемы.
- **E2E:** Playwright (опционально, Phase 3) по критическим путям: регистрация → урок → офлайн.
- **Lint/CI:** clippy `-D warnings`, rustfmt, `cargo test`, `cargo audit`.

---

## 10. Задачи (task breakdown для команды)

| # | Задача | Владелец | Блокеры |
|---|--------|----------|---------|
| 1 | Architecture & planning (docs) | planner | — (выполнено этим документом) |
| 2 | Инициализация Rust-проекта (Cargo, миграции, CI) | backend (lead) | 1 |
| 3 | Аутентификация (register/login/refresh/RBAC) | backend | 2 |
| 4 | Контент CRUD + seed (tracks/lessons/tasks) | backend + content | 2 |
| 5 | Прогресс и попытки + scoring | backend | 3,4 |
| 6 | Планировщик SM-2-lite + адаптация | planner → backend | 5 |
| 7 | Офлайн-бандлы + sync | backend + frontend | 5 |
| 8 | Фронтенд (каталог, урок, прогресс, PWA) | frontend | 4,7 |
| 9 | Сертификаты (запись + verify + printable) | backend | 5 |
| 10 | Тесты (unit/integration/e2e) | test-engineer | 3–9 |
| 11 | Безопасность-ревью + audit | security-agent | 10 |
| 12 | Документация пользователю / README | documentation | 8,9 |
| 13 | Проверка сборки/тестов | verifier | 10,11 |

---

## 11. Сид-контент: востребованные навыки (research-based seed list)

Кураторский список (обновлять раз в квартал по рынку). Организован по категориям; каждый трек состоит из 4–8 микроуроков по 10–15 мин.

**Языки и коммуникация**
- Английский для работы (письма, встречи, small talk)
- Публичные выступления и презентации
- Business writing (email, memo, report)
- Переговоры и активное слушание

**Программирование и данные**
- Python для автоматизации задач
- SQL и проектирование запросов
- Git/GitHub workflow и code review
- Prompt engineering и работа с LLM API
- Основы кибербезопасности и гигиена паролей
- Rust basics (ownership, borrowing) — для заинтересованных
- TypeScript/React основы

**Финансы и карьера**
- Личный бюджет и подушка безопасности
- Инвестиции: ETF, диверсификация, риски
- Налоги и отчётность (базовый уровень)
- Резюме, LinkedIn и собеседования

**Психология и продуктивность**
- Управление вниманием и привычки
- Эмоциональная регуляция и стресс
- Тайм-менеджмент / deep work
- Основы переговоров о границах

**Инструменты и AI-workflows**
- Автоматизация в Excel/Google Sheets
- Notion/Obsidian для базы знаний
- AI-ассистенты в повседневных задачах
- No-code/low-code (Make, n8n)

**Soft skills**
- Лидерство и делегирование
- Обратная связь и 1:1
- Работа в распределённой команде

---

## 12. Риски и митигации

| Риск | Митигация |
|------|-----------|
| SQLite узкое место записи | WAL + batched writes + разнос чтения; план миграции на Postgres |
| Сложность офлайн-синка | Idempotency keys, event-sourcing для attempts, last-write-wins с версией |
| Scope creep (монетизация) | Монетизация только полями/моками в v1 |
| Устаревание контента | content_version + hash; квартальный ревью списка навыков |
| XSS во фронте | CSP, escaping, JWT в памяти, refresh в httpOnly |
| Долгое время сборки Rust | cargo caching в CI; крейт-workspace только при росте |

---

## 13. Открытые вопросы (для lead/пользователя)

1. Нужен ли мультиязычный контент (RU/EN) в v1 или только RU?
2. Монетизация: реальный платёжный провайдер (CloudPayments/YooKassa) или мок?
3. Нужна ли интеграция с Telegram-ботом для напоминаний?
4. Порог «сертификата» — 100% уроков или ≥80% баллов?

> Создано: Planner. Отправить на review: reviewer/security-agent. API-детали — `docs/api.md`.
