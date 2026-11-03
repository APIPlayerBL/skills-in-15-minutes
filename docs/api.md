# Навыки за 15 минут — API (v1)

**Base URL:** `/api/v1`  
**Content-Type:** `application/json; charset=utf-8`  
**Auth:** `Authorization: Bearer <access_token>` (access 15m). Refresh token — httpOnly cookie.  
**Errors:** RFC 7807 `application/problem+json`.

---

## 0. Conventions

- Идентификаторы — UUID v7 (строка).
- Время — RFC 3339 UTC.
- Пагинация: `?limit=20&offset=0` (limit ≤ 100). Ответы списков: `{ "items": [...], "total": N, "limit": 20, "offset": 0 }`.
- Идемпотентность для mutating offline-синка: заголовок `Idempotency-Key: <uuid>`.
- Версионирование контента: ресурсы контента имеют `version`, `content_hash`; клиент шлёт `If-None-Match`.

---

## 1. Auth

### POST /auth/register

Request:
```json
{ "email": "u@example.com", "password": "strong-password-123", "display_name": "Иван" }
```
Response 201:
```json
{ "user": { "id": "...", "email": "...", "display_name": "...", "role": "user" },
  "access_token": "...", "expires_in": 900 }
```
+ Set-Cookie refresh.

### POST /auth/login
Request: `{ "email", "password" }` → same response.

### POST /auth/refresh
Cookie refresh → `{ "access_token", "expires_in" }` (refresh rotates).

### POST /auth/logout
Revokes refresh; 204.

### POST /auth/password/reset/request  (optional v1.1)
### POST /auth/password/reset/confirm

**Rate limits:** register/login 5/min per IP; refresh 30/min.

---

## 2. Users / Profile

### GET /me
Auth. Response user + stats:
```json
{ "id", "email", "display_name", "role", "created_at",
  "stats": { "tracks_enrolled": 2, "lessons_completed": 14, "streak_days": 3 } }
```

### PATCH /me
`{ "display_name"?, "email"? }` (email requires re-verify in v2).

### GET /me/progress
Query `?track_id=`. Returns lessons with status/score.

---

## 3. Tracks (public)

### GET /tracks
Query: `?difficulty=beginner&skill=python&limit=&offset=&published=true`.  
Response list: `{id, slug, title, description, difficulty, estimated_minutes, skill_ids, version}`.

### GET /tracks/{id_or_slug}
Full track with lessons (summary only: id, title, order_index, duration_min).

### POST /tracks/{id}/enroll  (auth)
Creates enrollment; idempotent. Response enrollment.

### GET /tracks/{id}/certificate (auth)
Returns user's certificate if issued, else 404.

---

## 4. Skills

### GET /skills
List categories/skills.  
### GET /skills/{slug}/tracks
Tracks mapped to skill.

---

## 5. Lessons

### GET /lessons/{id}
Response:
```json
{
  "id","track_id","slug","title","order_index","duration_min","version","content_hash",
  "content": {
    "intro": "текст...",
    "blocks": [
      { "type":"text", "value":"..." },
      { "type":"example", "value":"..." },
      { "type":"check", "task_id":"..." }
    ],
    "tasks": [
      { "id","kind":"mcq","prompt":"...","options":[{"id":"a","text":"..."}],"weight":1 }
    ]
  }
}
```
Контент урока полностью; для офлайн-пакета — то же через bundle.

### POST /lessons/{id}/complete  (auth)
Request:
```json
{
  "task_answers": [
    { "task_id":"...", "answer": "a" },
    { "task_id":"...", "answer": "текст" }
  ],
  "client_started_at": "2026-...",
  "duration_sec": 780
}
```
Response 200:
```json
{
  "lesson_id": "...",
  "score": 80,                                    // 0..100
  "passed": true,                                  // score >= pass_threshold (default 60)
  "task_results": [{ "task_id","correct":true,"score":1,"feedback":"..." }],
  "progress": { "status":"completed", "attempts_count":2 },
  "next": { "lesson_id":"...", "unlock_at":"..." },
  "schedule": { "due_at":"...", "interval_days":2, "state":"review" }
}
```

Идемпотентность: повторный complete с тем же `client_started_at` возвращает кешированный результат (no double score).

### GET /lessons/{id}/progress (auth)
Per-user progress for lesson.

---

## 6. Progress & Schedule

### GET /me/schedule
Query `?from=&to=&limit=`. Returns due lessons grouped by day:
```json
{ "days": [ { "date":"2026-11-04", "items":[ { "lesson_id","track_id","title","due_at","state" } ] } ] }
```

### POST /schedule/reschedule (auth, optional manual)
`{ "lesson_id", "reason":"vacation" }` → сдвигает дедлайн (max +7 дней). Rate-limited.

### GET /me/progress/summary
Aggregated by track: `{ track_id, completed, total, percent, last_activity_at }`.

---

## 7. Offline Bundles

### POST /bundles (auth)
Request:
```json
{ "track_ids": ["..."], "days": 7 }
```
Response 201:
```json
{ "id":"...", "expires_at":"...", "manifest": { "content_version":42,
  "files": [ { "path":"lessons/abc.json","hash":"sha256:...","size":1234 } ] } }
```

### GET /bundles/{id}  (auth, owner)
Manifest + status.

### GET /bundles/{id}/download  (auth, owner)
`application/zip` (или `multipart/mixed`). Заголовки: `ETag`, `Content-Disposition`. Проверка целостности на клиенте.

### POST /bundles/{id}/consume  (auth)
Помечает consumed после успешной загрузки.

---

## 8. Sync (offline → online)

### POST /sync (auth)
Request:
```json
{
  "events": [
    { "id":"uuid", "type":"attempt.submitted", "created_at":"...", "payload":{ "task_id":"...","answer":"...","duration_sec":60 } },
    { "id":"uuid", "type":"lesson.completed", "created_at":"...", "payload":{ "lesson_id":"...","client_started_at":"...","duration_sec":780 } },
    { "id":"uuid", "type":"schedule.missed", "created_at":"...", "payload":{ "lesson_id":"..." } }
  ]
}
```
Response 200:
```json
{ "applied": ["uuid",...], "conflicts": [ { "event_id":"...", "reason":"stale_version", "server_state":{...} } ],
  "server_time":"..." }
```
Семантика: идемпотентно по `events[].id`; события обрабатываются в порядке `created_at`; конфликты возвращаются, клиент применяет server_state.

**Rate limits:** 10/min per user.

---

## 9. Certificates

### POST /tracks/{id}/certificate (auth)
Выпуск (если все уроки завершены и score ≥ порог). 201 или 409 (уже выдано/не завершено).  
Response: `{ "id","verify_token","issued_at","metadata":{...} }`.

### GET /certificates/{id} (auth, owner)
Метаданные + URL печатной страницы: `/certificates/{id}/print`.

### GET /certificates/{id}/print (auth or public? owner only for full, public preview blurred)
HTML printable страница с QR → `/verify/{token}`.

### GET /verify/{token} (public)
`{ "valid":true, "user_display_name":"...", "track_title":"...", "issued_at":"...", "metadata":{...} }`.  
404/410 если отозван.

---

## 10. Admin / Content (role: editor/admin)

### POST /admin/tracks
### PATCH /admin/tracks/{id}
### POST /admin/lessons  `{ "track_id","slug","title","order_index","duration_min","content":{...} }`
### PATCH /admin/lessons/{id}
### POST /admin/lessons/{id}/publish  `{ "published":true }`
### DELETE /admin/lessons/{id}  (soft delete v1: published=false)
### POST /admin/import/content  multipart: zip с JSON+assets → upsert по slug, version++.

Все админ-маршруты: RBAC middleware (role editor/admin), audit log (actor, action, target, time).

---

## 11. Health & Meta

### GET /healthz  → `{ "status":"ok" }` (200)
### GET /readyz     → checks DB connection; 503 if not ready.
### GET /metrics     → Prometheus (опционально, behind flag).
### GET /api/v1/openapi.json → OpenAPI 3.1 (utoipa, если подключён).

---

## 12. Ошибки (RFC 7807)

```json
{
  "type": "https://errors.skills15min.app/validation",
  "title": "Validation failed",
  "status": 422,
  "detail": "password must be at least 10 characters",
  "instance": "/api/v1/auth/register",
  "errors": [ { "field":"password", "message":"..." } ]
}
```

Статусы: 400 (bad request), 401 (unauth), 403 (forbidden), 404, 409 (conflict), 422 (validation), 429 (rate limit), 500, 503.

---

## 13. Rate limits (сводка)

| Эндпоинт | Лимит |
|----------|-------|
| /auth/register, /auth/login | 5/min/IP |
| /auth/refresh | 30/min/IP |
| /sync | 10/min/user |
| /lessons/*/complete | 30/min/user |
| admin/* | 60/min/user |
| остальное | 120/min/IP |

Ответ 429 с `Retry-After`.

---

## 14. Пример полного потока (клиент)

1. `POST /auth/register` → access token + refresh cookie.
2. `GET /tracks?published=true` → список.
3. `POST /tracks/{id}/enroll`.
4. `GET /tracks/{id}` → уроки.
5. `GET /lessons/{lesson_id}` → контент.
6. Проверка ответов локально? Нет — отправка на сервер: `POST /lessons/{id}/complete`.
7. `GET /me/schedule` → следующий.
8. На неделю офлайна: `POST /bundles` → `GET .../download` → SW cache.
9. После завершения трека: `POST /tracks/{id}/certificate` → печать.

---

## 15. Совместимость и эволюция

- Breaking changes → `/api/v2` (старая версия поддерживается 6 мес).
- Additive-поля допустимы в v1; клиенты должны игнорировать unknown.
- Контент-версии независимы от API-версии.

> Создано: Planner. Реализация: backend-агенты по task breakdown в architecture.md.
