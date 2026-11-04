# Навыки за 15 минут — Skills in 15 Minutes

Microlearning platform: 10–15 minute interactive lessons for busy professionals, parents, and students.
Topics: languages, programming, finance, psychology, productivity, and in-demand skills.

## Stack
- Backend: Rust (Axum, Tokio, SQLx, SQLite)
- Auth: JWT access + httpOnly refresh cookie
- Frontend: static PWA (vanilla JS), served by Axum
- Tests: 35 passing (13 unit, 22 integration)

## Run locally
```bash
# database and bind address can be overridden with DATABASE_URL / BIND_ADDR / PORT
cargo run
```
The desktop window opens automatically; the local HTTP origin is chosen at startup and shown in the console log.

## Quality gates
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## API
See docs/api.md. Base URL `/api/v1`.

## Seed content
- English for work
- Python automation
- Personal finance
- Productivity & psychology

Full in-demand skill list in docs/architecture.md §11.
