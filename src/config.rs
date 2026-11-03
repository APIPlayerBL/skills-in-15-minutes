use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./data.db?mode=rwc".to_string()),
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| {
                use rand::RngCore;
                let mut key = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut key);
                base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, key)
            }),
        }
    }
}
