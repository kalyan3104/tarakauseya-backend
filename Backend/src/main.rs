use tara_kauseya_api::{AppState, config::AppConfig, db, routes};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> MainResult {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tara_kauseya_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env()?;
    let pool = db::connect_and_migrate(&config).await?;
    db::seed_catalogue(&pool, &config.seed_data_dir).await?;
    tara_kauseya_api::handlers::auth::seed_dev_admin(&pool).await?;
    let app = routes::create_router(AppState {
        pool,
        config: config.clone(),
    });
    let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
    tracing::info!(address = %config.bind_address, "Tara Kauseya API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

type MainResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("shutdown signal received"),
        Err(error) => {
            tracing::warn!(%error, "signal handling unavailable; use process termination to stop the server");
            std::future::pending::<()>().await;
        }
    }
}
