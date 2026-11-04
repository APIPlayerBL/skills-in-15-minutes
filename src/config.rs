use std::env;
use std::path::PathBuf;

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
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(default_database_url),
            bind_addr: env::var("BIND_ADDR")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| {
                    env::var("PORT")
                        .ok()
                        .map(|p| format!("127.0.0.1:{}", p.trim()))
                })
                .unwrap_or_else(|| "127.0.0.1:0".to_string()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| {
                use rand::RngCore;
                let mut key = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut key);
                base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, key)
            }),
        }
    }
}

/// Resolve a default SQLite file under the per-user data directory instead of
/// scattering `data.db`/`test.db` files through the current working directory.
fn default_database_url() -> String {
    let path = default_data_dir().join("skills15.db");
    format!("sqlite:{}?mode=rwc", path.display())
}

fn default_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("SKILLS15_DATA_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Ok(dir) = env::var("XDG_DATA_HOME") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir).join("skills15");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(dir) = env::var("LOCALAPPDATA").or_else(|_| env::var("APPDATA")) {
            if !dir.trim().is_empty() {
                return PathBuf::from(dir).join("Skills15");
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Skills15");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("skills15");
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_db_is_not_in_current_dir() {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("SKILLS15_DATA_DIR");
        std::env::remove_var("XDG_DATA_HOME");
        let cfg = Config::from_env();
        assert!(!cfg.database_url.contains("./data.db"));
        assert!(!cfg.database_url.contains("test.db"));
    }
}
