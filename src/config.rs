use std::{env, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct AppConfig {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub direct_database_url: String,
    pub jwt_secret: String,
    pub supabase_jwt_secret: Option<String>,
    pub supabase_storage: Option<SupabaseStorageConfig>,
    pub production: bool,
    pub uploads_dir: PathBuf,
    pub seed_data_dir: PathBuf,
    pub frontend_origin: String,
}

#[derive(Clone)]
pub struct SupabaseStorageConfig {
    pub url: String,
    pub service_role_key: String,
    pub bucket: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        dotenvy::dotenv().ok();
        let root = env::current_dir()?;
        if env::var("DATABASE_URL").is_err() {
            let backend_env = root.join("Backend").join(".env");
            if backend_env.exists() {
                dotenvy::from_filename(backend_env).ok();
            }
        }
        let port: u16 = env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;
        let supabase_url = env::var("SUPABASE_URL").ok();
        let supabase_service_role_key = env::var("SUPABASE_SERVICE_ROLE_KEY").ok();
        let supabase_storage = match (supabase_url, supabase_service_role_key) {
            (Some(url), Some(service_role_key)) => Some(SupabaseStorageConfig {
                url: url.trim_end_matches('/').to_string(),
                service_role_key,
                bucket: env::var("SUPABASE_STORAGE_BUCKET")
                    .unwrap_or_else(|_| "product-images".to_string()),
            }),
            (None, None) => None,
            _ => {
                return Err(
                    "SUPABASE_URL and SUPABASE_SERVICE_ROLE_KEY must be set together".into(),
                );
            }
        };

        Ok(Self {
            bind_address: format!(
                "{}:{}",
                // A hosted web service must listen on every interface so that
                // Render's proxy and health checks can reach it. Developers can
                // still set HOST=127.0.0.1 in .env to restrict local access.
                env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port
            )
            .parse()?,
            database_url: env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL must be set in Backend/.env")?,
            direct_database_url: env::var("DIRECT_URL")
                .unwrap_or_else(|_| env::var("DATABASE_URL").expect("DATABASE_URL checked above")),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| {
                "change-this-local-development-secret-before-production".to_string()
            }),
            supabase_jwt_secret: env::var("SUPABASE_JWT_SECRET").ok(),
            supabase_storage,
            production: env::var("APP_ENV")
                .map(|value| value.eq_ignore_ascii_case("production"))
                .unwrap_or(false),
            uploads_dir: PathBuf::from(
                env::var("UPLOADS_DIR")
                    .unwrap_or_else(|_| root.join("uploads").display().to_string()),
            ),
            seed_data_dir: PathBuf::from(env::var("SEED_DATA_DIR").unwrap_or_else(|_| {
                root.parent()
                    .unwrap_or(&root)
                    .join("data")
                    .display()
                    .to_string()
            })),
            frontend_origin: env::var("FRONTEND_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
        })
    }
}
