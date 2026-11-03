-- 0001_init.sql
CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  display_name TEXT,
  role TEXT NOT NULL DEFAULT 'user',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS skills (
  id TEXT PRIMARY KEY,
  slug TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  category TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tracks (
  id TEXT PRIMARY KEY,
  slug TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  difficulty TEXT NOT NULL,
  estimated_minutes INTEGER NOT NULL,
  published INTEGER NOT NULL DEFAULT 0,
  version INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS track_skills (
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  PRIMARY KEY (track_id, skill_id)
);
CREATE TABLE IF NOT EXISTS lessons (
  id TEXT PRIMARY KEY,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  order_index INTEGER NOT NULL,
  duration_min INTEGER NOT NULL CHECK (duration_min BETWEEN 5 AND 20),
  content_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  published INTEGER NOT NULL DEFAULT 0,
  UNIQUE (track_id, slug)
);
CREATE TABLE IF NOT EXISTS lesson_tasks (
  id TEXT PRIMARY KEY,
  lesson_id TEXT NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  prompt TEXT NOT NULL,
  options_json TEXT,
  answer_key TEXT NOT NULL,
  weight INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS refresh_tokens (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT,
  replaced_by TEXT
);
CREATE TABLE IF NOT EXISTS enrollments (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  PRIMARY KEY (user_id, track_id)
);
CREATE TABLE IF NOT EXISTS lesson_progress (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  lesson_id TEXT NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  score INTEGER,
  attempts_count INTEGER NOT NULL DEFAULT 0,
  first_completed_at TEXT,
  last_activity_at TEXT NOT NULL,
  idempotency_key TEXT,
  PRIMARY KEY (user_id, lesson_id)
);
CREATE TABLE IF NOT EXISTS attempts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  task_id TEXT NOT NULL REFERENCES lesson_tasks(id) ON DELETE CASCADE,
  submitted_answer TEXT NOT NULL,
  correct INTEGER NOT NULL,
  score INTEGER NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS schedules (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  lesson_id TEXT NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
  due_at TEXT NOT NULL,
  interval_days INTEGER NOT NULL DEFAULT 1,
  ease REAL NOT NULL DEFAULT 2.5,
  lapses INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (user_id, lesson_id)
);
CREATE TABLE IF NOT EXISTS bundles (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  manifest_json TEXT NOT NULL,
  status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS certificates (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  issued_at TEXT NOT NULL,
  verify_token TEXT UNIQUE NOT NULL,
  metadata_json TEXT NOT NULL,
  UNIQUE (user_id, track_id)
);

CREATE INDEX IF NOT EXISTS idx_schedules_user_due ON schedules(user_id, due_at);
CREATE INDEX IF NOT EXISTS idx_lesson_progress_user_status ON lesson_progress(user_id, status);
CREATE INDEX IF NOT EXISTS idx_lessons_track_order ON lessons(track_id, order_index);
CREATE INDEX IF NOT EXISTS idx_attempts_user_task ON attempts(user_id, task_id);
