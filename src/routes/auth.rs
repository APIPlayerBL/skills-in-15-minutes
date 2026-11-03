use crate::error::AppError;
use crate::middleware::{hash_password, make_access_token, verify_password};
use crate::state::AppState;
use axum::http::header::SET_COOKIE;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
}

#[derive(Deserialize)]
struct RegisterReq {
    email: String,
    password: String,
    display_name: Option<String>,
}
#[derive(Deserialize)]
struct LoginReq {
    email: String,
    password: String,
}
#[derive(Deserialize)]
struct RefreshReq {
    refresh_token: Option<String>,
}

#[derive(Serialize)]
struct UserOut {
    id: String,
    email: String,
    display_name: Option<String>,
    role: String,
}
#[derive(Serialize)]
struct AuthResp {
    user: UserOut,
    access_token: String,
    expires_in: i64,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn hash_token(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    hex::encode(h.finalize())
}

async fn register(
    State(st): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<(HeaderMap, Json<AuthResp>), AppError> {
    if req.password.len() < 10 || req.password.len() > 128 {
        return Err(AppError::Validation(
            "password must be at least 10 characters".into(),
        ));
    }
    if !req.email.contains('@') {
        return Err(AppError::Validation("invalid email".into()));
    }
    let id = Uuid::new_v4().to_string();
    let hash = hash_password(&req.password).map_err(|e| AppError::Internal(e.to_string()))?;
    let ts = now();
    sqlx::query("INSERT INTO users (id,email,password_hash,display_name,role,created_at,updated_at) VALUES (?,?,?,?,?,?,?)")
        .bind(&id).bind(&req.email).bind(&hash).bind(&req.display_name).bind("user").bind(&ts).bind(&ts)
        .execute(&st.db).await.map_err(|e| {
            if e.to_string().contains("UNIQUE") { AppError::Conflict("email already registered".into()) } else { AppError::Db(e) }
        })?;
    let access = make_access_token(&id, "user", &st.config.jwt_secret);
    let cookie = issue_refresh_cookie(&st.db, &id).await?;
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.parse().unwrap());
    Ok((
        headers,
        Json(AuthResp {
            user: UserOut {
                id,
                email: req.email,
                display_name: req.display_name,
                role: "user".into(),
            },
            access_token: access,
            expires_in: 900,
        }),
    ))
}

async fn issue_refresh_cookie(db: &SqlitePool, user_id: &str) -> Result<String, AppError> {
    let raw = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        rand::random::<[u8; 32]>(),
    );
    let hash = hash_token(&raw);
    let id = Uuid::new_v4().to_string();
    let expires = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    sqlx::query("INSERT INTO refresh_tokens (id,user_id,token_hash,expires_at) VALUES (?,?,?,?)")
        .bind(&id)
        .bind(user_id)
        .bind(&hash)
        .bind(&expires)
        .execute(db)
        .await?;
    Ok(format!(
        "refresh_token={}; HttpOnly; Path=/; Max-Age=2592000; SameSite=Lax",
        raw
    ))
}

async fn login(
    State(st): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<(HeaderMap, Json<AuthResp>), AppError> {
    use sqlx::Row;
    let row = sqlx::query("SELECT id,password_hash,display_name,role FROM users WHERE email=?")
        .bind(&req.email)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::Unauthorized("invalid credentials".into()))?;
    let id: String = row.get("id");
    let password_hash: String = row.get("password_hash");
    let display_name: Option<String> = row.get("display_name");
    let role: String = row.get("role");
    if !verify_password(&req.password, &password_hash) {
        return Err(AppError::Unauthorized("invalid credentials".into()));
    }
    let access = make_access_token(&id, &role, &st.config.jwt_secret);
    let cookie = issue_refresh_cookie(&st.db, &id).await?;
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.parse().unwrap());
    Ok((
        headers,
        Json(AuthResp {
            user: UserOut {
                id,
                email: req.email,
                display_name,
                role,
            },
            access_token: access,
            expires_in: 900,
        }),
    ))
}

async fn refresh(
    State(st): State<AppState>,
    headers: HeaderMap,
    req: axum::extract::Json<Option<RefreshReq>>,
) -> Result<Json<serde_json::Value>, AppError> {
    // read cookie or body
    let mut raw: Option<String> = None;
    if let Some(c) = headers.get("cookie").and_then(|h| h.to_str().ok()) {
        for part in c.split(';') {
            let part = part.trim();
            if let Some(v) = part.strip_prefix("refresh_token=") {
                raw = Some(v.to_string());
            }
        }
    }
    if raw.is_none() {
        raw = req.0.and_then(|r| r.refresh_token);
    }
    let raw = raw.ok_or(AppError::Unauthorized("missing refresh token".into()))?;
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    let hash = hex::encode(h.finalize());
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT id,user_id,expires_at,revoked_at FROM refresh_tokens WHERE token_hash=?",
    )
    .bind(&hash)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::Unauthorized("invalid refresh token".into()))?;
    let token_id: String = row.get("id");
    let user_id: String = row.get("user_id");
    let expires_at: String = row.get("expires_at");
    let revoked: Option<String> = row.get("revoked_at");
    if revoked.is_some() {
        // reuse detected: revoke all tokens for user
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at=? WHERE user_id=? AND revoked_at IS NULL",
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&user_id)
        .execute(&st.db)
        .await
        .ok();
        return Err(AppError::Unauthorized("token reuse detected".into()));
    }
    if expires_at < chrono::Utc::now().to_rfc3339() {
        return Err(AppError::Unauthorized("token expired".into()));
    }
    // rotate: revoke old and issue a new refresh token
    let new_raw = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        rand::random::<[u8; 32]>(),
    );
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, new_raw.as_bytes());
    let new_hash = hex::encode(hasher.finalize());
    let new_id = uuid::Uuid::new_v4().to_string();
    let new_expires = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    sqlx::query("INSERT INTO refresh_tokens (id,user_id,token_hash,expires_at) VALUES (?,?,?,?)")
        .bind(&new_id)
        .bind(&user_id)
        .bind(&new_hash)
        .bind(&new_expires)
        .execute(&st.db)
        .await?;
    sqlx::query("UPDATE refresh_tokens SET revoked_at=? , replaced_by=? WHERE id=?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&new_id)
        .bind(&token_id)
        .execute(&st.db)
        .await?;
    let role: String = sqlx::query("SELECT role FROM users WHERE id=?")
        .bind(&user_id)
        .fetch_one(&st.db)
        .await?
        .get("role");
    let access = make_access_token(&user_id, &role, &st.config.jwt_secret);
    Ok(Json(
        serde_json::json!({"access_token": access, "expires_in": 900}),
    ))
}

async fn logout(State(st): State<AppState>, headers: HeaderMap) -> StatusCode {
    // revoke if cookie present
    if let Some(c) = headers.get("cookie").and_then(|h| h.to_str().ok()) {
        for part in c.split(';') {
            let part = part.trim();
            if let Some(v) = part.strip_prefix("refresh_token=") {
                use sha2::{Digest, Sha256};
                let mut h = Sha256::new();
                h.update(v.as_bytes());
                let hash = hex::encode(h.finalize());
                sqlx::query("UPDATE refresh_tokens SET revoked_at=? WHERE token_hash=?")
                    .bind(chrono::Utc::now().to_rfc3339())
                    .bind(&hash)
                    .execute(&st.db)
                    .await
                    .ok();
            }
        }
    }
    let _ = st;
    StatusCode::NO_CONTENT
}
