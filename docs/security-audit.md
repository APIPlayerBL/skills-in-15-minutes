# Security Audit Report — Навыки за 15 минут

**Auditor:** Security Agent  
**Scope:** Current Rust/Axum scaffold in `src/`, `migrations/`, `Cargo.toml`, plus architecture/API design docs.  
**Method:** Manual code review, OWASP Top 10 mapping, grep-based secret scan, dependency/build inspection.  

> Note: `cargo check` currently **fails** (see S-0 build blocker below), so automated tests/static analysis could not run. Dependency CVE audit could not run (`cargo audit` not installed).

## Executive summary

- **Overall status:** Not production-ready. The scaffold contains several high-risk authentication/CORS/cookie and stored-XSS issues, as well as several integrity and DoS gaps. The public content/progress authorization model is incomplete.
- **Critical/HIGH issues:** 6
- **MEDIUM issues:** 8+
- **LOW/INFO issues:** several

No committed secrets were found. SQL access appears parameterized (`sqlx` binds), so classic SQL injection is not currently evident.

## S-0 — Build blocker (High, operational)

`cargo check` fails:
- `src/routes/{auth,users,tracks,lessons,certificates}.rs` use `app::error::Result<T, AppError>`, but `AppError::Result` is `Result<T, AppError>` (one generic argument).
- `src/routes/auth.rs::refresh` is rejected as an Axum handler, likely due extractor/body ordering after the type-error cascade.

This blocks verification and any security testing.

## Findings

### H-1 — Insecure fallback JWT secret
- **File:** `src/config.rs:15`
- **Severity:** High
- **Issue:** `JWT_SECRET` fallback is a publicly known string. If the variable is missing (misconfigured deploy, dev mode exposed), all access tokens are forgeable.
- **Fix:** Remove fallback or require length ≥ 32; fail startup when missing/short.

### H-2 — Permissive CORS
- **File:** `src/main.rs:42`
- **Severity:** High
- **Issue:** `allow_origin(Any).allow_methods(Any).allow_headers(Any)` allows arbitrary origins to call API with custom headers. Combined with bearer tokens or future refresh-cookie credentials, this weakens origin isolation and can expose data/actions to untrusted sites.
- **Fix:** Configure explicit allowed origins from env/config; in production do not use `Any`.

### H-3 — Refresh cookie missing `Secure`
- **File:** `src/routes/auth.rs:61`
- **Severity:** High (transport-confidentiality)
- **Issue:** Cookie string has `HttpOnly; SameSite=Lax`, but no `Secure`. Architecture requires `Secure`; implementation omits it, allowing refresh-token capture on plain HTTP/mitm paths.
- **Fix:** Add `Secure` (behind TLS/production flag as appropriate). Also set explicit `Domain` only if needed.

### H-4 — Unbounded password input / argon2 DoS
- **File:** `src/routes/auth.rs:37`
- **Severity:** High (DoS)
- **Issue:** Only minimum length is checked. Extremely large passwords are passed directly to Argon2, enabling request-based resource exhaustion.
- **Fix:** Enforce `8..=128` (or 256) bytes before hashing; reject larger payloads.

### H-5 — Stored XSS in certificate print page
- **File:** `src/routes/certificates.rs:59`
- **Severity:** High
- **Issue:** User-controlled `display_name` and track `title` are interpolated into HTML without escaping:
  `format!("<html>...<p>{}</p>...")`. An attacker can set a malicious display name, complete a track, and execute script on the public print page.
- **Fix:** HTML-escape all interpolated fields, or use a templating engine with auto-escaping. Keep public page minimal.

### H-6 — Refresh-token reuse detection not implemented
- **File:** `src/routes/auth.rs:99–114`
- **Severity:** High
- **Issue:** Architecture specifies rotation plus reuse-detection (revoke entire family on replay). Current code only marks the old token revoked and overwrites `replaced_by` with `user_id` rather than the new token id, so replay/family tracking cannot work.
- **Fix:** Insert a new refresh-token row (linked by `replaced_by` to old token), revoke old token, and on access to a revoked token revoke the whole family for that user.

### M-1 — Information disclosure: unpublished content
- **Files:** `src/routes/tracks.rs:21,42`; `src/routes/lessons.rs:15`
- **Severity:** Medium
- **Issue:** `list_tracks` ignores the `published` filter and returns all tracks; `get_track`/`get_lesson` expose content regardless of publish state (or ownership). Unpublished/administrative content may leak.
- **Fix:** Public endpoints should filter `published=1` unless an authenticated editor/admin requests otherwise. Enforce separate admin endpoints/RBAC.

### M-2 — Completion/integrity weaknesses
- **File:** `src/routes/lessons.rs:61,84`
- **Severity:** Medium (integrity)
- **Issue:** `complete_lesson` does not verify that the lesson exists, is published, or is enrolled/valid before recording progress and attempts. If a lesson has no tasks, score defaults to 100. This allows arbitrary progress records and weakens certificate eligibility.
- **Fix:** Verify lesson exists and is published; require/verify enrollment or a valid access path; reject empty-weighted lessons or treat no-task completion specially; bound answers.

### M-3 — Weak grading logic
- **File:** `src/routes/lessons.rs:130–144`
- **Severity:** Medium (integrity)
- **Issue:** `grade()` uses case-insensitive `contains` for short/code answers, allowing accidental/false positives if the expected key is a substring. `multi` is graded like single MCQ.
- **Fix:** Use constant-time exact normalization for short answers (or dedicated checker); implement proper multi-answer scoring. Avoid substring fallback in security-relevant pass/fail decisions.

### M-4 — Missing rate limiting (auth)
- **Files:** `src/main.rs`, `src/routes/auth.rs`
- **Severity:** Medium/High (auth hardening)
- **Issue:** No rate limiting on register/login/refresh, contrary to architecture/API design (`5/min/IP`, etc.).
- **Fix:** Add `tower-governor` (or reverse-proxy limit) and per-IP/per-user limits; lock accounts/retry-after handling.

### M-5 — Missing security headers/CSP
- **File:** `src/main.rs`
- **Severity:** Medium
- **Issue:** No CSP, HSTS, `X-Content-Type-Options`, Referrer-Policy, etc. XSS mitigations rely entirely on frontend behavior.
- **Fix:** Add security headers middleware; use strict CSP once frontend is implemented.

### M-6 — Error detail leakage
- **File:** `src/error.rs:38–46`
- **Severity:** Medium
- **Issue:** `AppError::Db` and validation errors propagate internal details (`self.to_string()`), which can expose schema/SQL or validation internals. `register` also string-matches `UNIQUE` to infer conflict, which is brittle.
- **Fix:** Return generic messages for DB/internal errors; distinguish conflict via SQLx error codes, not substring matching.

### M-7 — No input size/length bounds
- **Files:** `src/routes/auth.rs` DTOs; `src/routes/lessons.rs` (`submitted_answer` length), `src/routes/users.rs` (`display_name`)
- **Severity:** Medium
- **Issue:** DTOs use unbounded `String`/`Vec` with no `#[serde]` limits. Large payloads may inflate DB/storage and DoS.
- **Fix:** Add `deny_unknown_fields` where practical, max lengths, and Axum body limits. Validate email length and format more strictly.

### M-8 — Reuse detection can be client-side session-wipe risk (design)
- **Severity:** Medium/Low
- **Issue:** If reuse detection is implemented, an attacker with a stolen refresh token can force session revocation. This is generally acceptable and standard, but should be monitored/alerted. Ensure logs capture reuse events without leaking tokens.

### M-9 — Direct public access to certificate print page (`print_cert`)
- **File:** `src/routes/certificates.rs:54–64`
- **Severity:** Medium (authorization + privacy)
- **Issue:** `print_cert` takes no authenticated user and only an ID. Certificate IDs are UUIDs (unguessable) but there is no rate limiting/ownership check. Public verification endpoint already exposes partial data by design, but full printable page may expose more.
- **Fix:** Decide intended access model: owner-only print, or public preview with sanitized fields and rate limits. Current code mixes both.

### M-10 — SQL/refresh expiration compared lexicographically
- **File:** `src/routes/auth.rs:108`
- **Severity:** Low (correctness)
- **Issue:** `expires_at < chrono::Utc::now().to_rfc3339()` compares RFC3339 strings. This works only if the stored timestamp format exactly matches; trailing variants (offsets) can break.
- **Fix:** Parse both timestamps into `DateTime<Utc>` and compare.

### L-1 — Missing CSRF protection for cookie refresh flow
- **Files:** `src/main.rs`, `src/routes/auth.rs`
- **Severity:** Low/Medium depending on frontend
- **Issue:** Architecture specifies double-submit CSRF; scaffold only uses SameSite=Lax. Because cookie is readable only HttpOnly, CSRF still possible in same-site contexts if method/cookie behavior allows.
- **Fix:** Implement CSRF token for unsafe cookie-authenticated requests or keep refresh body token pattern and document.

### L-2 — Token output not URL-safe validated for certificate token
- **File:** `src/routes/certificates.rs:31`
- **Severity:** Low
- **Issue:** Token is base64 URL-safe with no padding; generally acceptable. Rate-limit `/verify`.
- **Fix:** Keep current entropy; add rate limits and maybe length checks.

### L-3 — Service Worker/offline sync trust boundary (design)
- **Source:** `docs/architecture.md §6.3`, `docs/api.md §8`
- **Severity:** High (to implement safely)
- **Issue:** If sync processes `lesson.completed` client events as authoritative, malicious clients can mark progress/awards. Server must derive completion from attempts/score and treat offline events as untrusted inputs.
- **Fix:** Sync should only apply attempts, then server recomputes progress; never trust client completion claims.

### L-4 — Admin import attack surface (future)
- **Source:** `docs/api.md §10`
- **Severity:** High (to implement safely)
- **Issue:** ZIP/JSON import can introduce path traversal, zip bombs, stored XSS, and schema injection. Controls are specified in API but must be strictly enforced during implementation.
- **Fix:** Strict schema validation, allowlisted paths, size/count caps, safe extraction (no `..`, symlinks, absolute paths), content sanitization, audit logging.

### Dependency / supply chain
- **`Cargo.toml`:** No version caps beyond semver-minor; `cargo audit` not installed. Recommend:
  - Add `cargo audit` (or OSV) to CI.
  - Pin versions via `Cargo.lock` (already present) and review updates.
  - Review `zip` crate extraction paths if offline bundle/zip features land.
  - Consider adding `tower-governor`, `headers`, HTML sanitizer/template.
  - Ensure Argon2 default parameters meet current OWASP guidance (review memory/time params).

## Recommended immediate remediations

1. Fix scaffold build (`Result` alias usage and refresh handler signature).
2. Remove known JWT default secret and enforce minimum length.
3. Restrict CORS to configured origins.
4. Add `Secure` to refresh cookie.
5. Cap password length before argon2.
6. HTML-escape certificate print output.
7. Filter unpublished content and protect certificates.
8. Add rate limiting and security headers.
9. Rewrite lesson completion to validate inputs and lesson existence.
10. Implement refresh-token rotation + reuse detection correctly.

## Conclusion

The current scaffold demonstrates the intended layered Rust/Axum design and uses parameterized SQL, which is good. However, security-critical controls (secret handling, CORS, cookie flags, XSS escaping, rate limits, token reuse detection, and content authorization) are either missing or incomplete. These should be fixed before feature code and tests proceed, and the build blocker must be resolved first.
