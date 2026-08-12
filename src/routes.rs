use crate::{
    AppState,
    handlers::{auth, entities, health, imports, uploads},
};
use axum::{
    Router,
    http::{HeaderValue, Method, header},
    routing::{get, patch, post},
};
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, limit::RequestBodyLimitLayer,
    services::ServeDir, trace::TraceLayer,
};

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .config
                .frontend_origin
                .parse::<HeaderValue>()
                .unwrap_or(HeaderValue::from_static("http://localhost:5173")),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    Router::new()
        .route("/api/health", get(health::check_health))
        .route("/api/auth/supabase-exchange", post(auth::supabase_exchange))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/logout", post(auth::logout))
        .route(
            "/api/entities/:entity",
            get(entities::list).post(entities::create),
        )
        .route("/api/entities/:entity/filter", post(entities::filter))
        .route("/api/entities/:entity/bulk", post(entities::bulk_create))
        .route(
            "/api/entities/:entity/:id",
            patch(entities::update).delete(entities::delete),
        )
        .route("/api/integrations/core/upload-file", post(uploads::upload))
        .route(
            "/api/functions/BulkImportProducts",
            post(imports::bulk_import_products),
        )
        .nest_service("/uploads", ServeDir::new(state.config.uploads_dir.clone()))
        .layer(RequestBodyLimitLayer::new(25 * 1024 * 1024))
        .layer(CompressionLayer::new().gzip(true))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
