use std::{env, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct AppConfig {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub direct_database_url: String,
    pub jwt_secret: String,
        pub supabase_jwt_secret: Option<String>,
    pub uploads_dir: PathBuf,
    pub seed_data_dir: PathBuf,
    pub frontend_origin: String,
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
        Ok(Self {
            bind_address: format!(
                "{}:{}",
                env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
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
