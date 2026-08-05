use crate::{config::AppConfig, error::ApiError};
use chrono::Utc;
use serde_json::Value;
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{path::Path, str::FromStr};

fn pool_options(database_url: &str) -> Result<PgConnectOptions, sqlx::Error> {
    // The configured hosted database uses PgBouncer in transaction-pooling
    // mode. Prepared-statement caching is incompatible with that mode and
    // otherwise causes `prepared statement ... already exists` on auth calls.
    Ok(PgConnectOptions::from_str(database_url)?.statement_cache_capacity(0))
}

pub async fn connect_and_migrate(
    config: &AppConfig,
) -> Result<PgPool, Box<dyn std::error::Error + Send + Sync>> {
    tokio::fs::create_dir_all(&config.uploads_dir).await?;
    let migration_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(pool_options(&config.direct_database_url)?)
        .await?;
    sqlx::migrate!("./migrations").run(&migration_pool).await?;
    migration_pool.close().await;
    // Supabase transaction poolers cannot safely retain SQLx prepared
    // statements between requests. Use the supplied direct URL when the
    // pooled URL explicitly identifies itself as PgBouncer.
    let application_url = if config.database_url.contains("pgbouncer=true") {
        &config.direct_database_url
    } else {
        &config.database_url
    };
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect_with(pool_options(application_url)?)
        .await?;
    Ok(pool)
}

pub async fn seed_catalogue(
    pool: &PgPool,
    directory: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entities WHERE entity_type = 'products'")
            .fetch_one(pool)
            .await?;
    if count > 0 {
        return Ok(());
    }
    for entity in [
        "products",
        "collections",
        "inventory",
        "media-assets",
        "trial-requests",
        "service-areas",
    ] {
        let path = directory.join(format!("{entity}.json"));
        let Ok(text) = tokio::fs::read_to_string(path).await else {
            continue;
        };
        let Ok(items) = serde_json::from_str::<Vec<Value>>(&text) else {
            continue;
        };
        for mut item in items {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let now = Utc::now().to_rfc3339();
            if let Some(object) = item.as_object_mut() {
                object.entry("id").or_insert(Value::String(id.clone()));
                object
                    .entry("created_date")
                    .or_insert(Value::String(now.clone()));
                object
                    .entry("updated_date")
                    .or_insert(Value::String(now.clone()));
            }
            sqlx::query("INSERT INTO entities (entity_type,id,data,created_at,updated_at) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (entity_type,id) DO NOTHING")
                .bind(entity).bind(id).bind(serde_json::to_string(&item)?).bind(&now).bind(&now).execute(pool).await?;
        }
    }
    Ok(())
}

pub async fn records(pool: &PgPool, entity: &str) -> Result<Vec<Value>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar("SELECT data FROM entities WHERE entity_type = $1")
        .bind(entity)
        .fetch_all(pool)
        .await
        .map_err(ApiError::internal)?;
    rows.into_iter()
        .map(|row| serde_json::from_str(&row).map_err(ApiError::internal))
        .collect()
}

pub async fn save(
    pool: &PgPool,
    entity: &str,
    id: &str,
    value: &Value,
    created_at: Option<&str>,
) -> Result<(), ApiError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO entities (entity_type,id,data,created_at,updated_at) VALUES ($1,$2,$3,$4,$5) ON CONFLICT(entity_type,id) DO UPDATE SET data=EXCLUDED.data, updated_at=EXCLUDED.updated_at")
        .bind(entity).bind(id).bind(serde_json::to_string(value).map_err(ApiError::internal)?).bind(created_at.unwrap_or(&now)).bind(now.clone()).execute(pool).await.map_err(ApiError::internal)?;
    Ok(())
}
