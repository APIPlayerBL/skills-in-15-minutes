use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub role: String,
}

#[async_trait::async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let token = auth
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "missing bearer"))?;
        let secret = state.config.jwt_secret.as_bytes();
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.leeway = 0;
        let data = decode::<Claims>(token, &DecodingKey::from_secret(secret), &validation)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token"))?;
        Ok(AuthUser {
            user_id: data.claims.sub,
            role: data.claims.role,
        })
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Claims {
    sub: String,
    role: String,
    exp: usize,
}
pub fn make_access_token(user_id: &str, role: &str, secret: &str) -> String {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    #[derive(serde::Serialize)]
    struct C {
        sub: String,
        role: String,
        exp: usize,
        iat: usize,
    }
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = C {
        sub: user_id.to_string(),
        role: role.to_string(),
        exp: now + 900,
        iat: now,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}
pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    use argon2::Argon2;
    let parsed = match PasswordHash::new(hash) {
        Ok(p) => p,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    use argon2::Argon2;
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}
