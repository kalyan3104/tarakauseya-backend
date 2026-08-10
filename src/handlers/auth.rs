use crate::{
    AppState,
    error::{ApiError, ApiResult},
    middleware::auth::{AuthUser, issue_token},
    models::auth::{AuthResponse, PublicUser},
};
use axum::{Json, extract::State};
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SupaClaims {
    pub _sub: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub _exp: usize,
    pub _role: Option<String>,
}

pub async fn supabase_exchange(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(_input): Json<serde_json::Value>,
) -> ApiResult<Json<AuthResponse>> {
    let supa_secret = state
        .config
        .supabase_jwt_secret
        .as_ref()
        .ok_or_else(|| ApiError::internal("SUPABASE_JWT_SECRET not configured"))?;

    let header_str = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("Authorization header required"))?;

    let token = header_str
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("Invalid authorization header"))?;

    let decoded = decode::<SupaClaims>(
        token,
        &DecodingKey::from_secret(supa_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| ApiError::unauthorized("Invalid Supabase token"))?;

    let phone_claim = decoded.claims.phone.map(|value| value.trim().to_string());
    let email_claim = decoded.claims.email.map(|value| value.trim().to_lowercase());

    let row_opt = if let Some(phone) = phone_claim.as_deref() {
        sqlx::query("SELECT id,email,phone,name,role,verified FROM users WHERE phone=$1")
            .bind(phone)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?
    } else if let Some(email) = email_claim.as_deref() {
        sqlx::query("SELECT id,email,phone,name,role,verified FROM users WHERE email=$1")
            .bind(email)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?
    } else {
        return Err(ApiError::unauthorized("Invalid Supabase token claims"));
    };

    let user = if let Some(row) = row_opt {
        let user = user_from_row(&row)?;
        let verified: i32 = row.try_get("verified").map_err(ApiError::internal)?;
        if verified == 0 {
            sqlx::query("UPDATE users SET verified=1, updated_at=$2 WHERE id=$1")
                .bind(&user.id)
                .bind(Utc::now().to_rfc3339())
                .execute(&state.pool)
                .await
                .map_err(ApiError::internal)?;
        }
        user
    } else {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO users (id,email,phone,name,role,verified,created_at,updated_at) VALUES ($1,$2,$3,'', 'customer',1,$4,$4)")
            .bind(&id)
            .bind(email_claim.clone())
            .bind(phone_claim.as_deref())
            .bind(&now)
            .execute(&state.pool)
            .await
            .map_err(ApiError::internal)?;
        let row = if let Some(phone) = phone_claim.as_deref() {
            sqlx::query("SELECT id,email,phone,name,role,verified FROM users WHERE phone=$1")
                .bind(phone)
                .fetch_one(&state.pool)
                .await
                .map_err(ApiError::internal)?
        } else {
            sqlx::query("SELECT id,email,phone,name,role,verified FROM users WHERE email=$1")
                .bind(email_claim.as_deref().unwrap())
                .fetch_one(&state.pool)
                .await
                .map_err(ApiError::internal)?
        };
        user_from_row(&row)?
    };

    let token = issue_token(&state, &user.id, &user.email, user.phone.as_deref(), &user.role)?;
    Ok(Json(AuthResponse { access_token: token, user }))
}

pub async fn me(AuthUser(claims): AuthUser) -> Json<PublicUser> {
    Json(PublicUser {
        id: claims.sub,
        email: claims.email,
        phone: claims.phone,
        name: String::new(),
        role: claims.role,
    })
}

pub async fn logout() -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true}))
}

const DEV_ADMIN_PHONE: &str = "+919999999999";

pub async fn seed_dev_admin(pool: &PgPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let existing: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE phone=$1")
        .bind(DEV_ADMIN_PHONE)
        .fetch_optional(pool)
        .await?;
    let now = Utc::now().to_rfc3339();

    if let Some(id) = existing {
        sqlx::query("UPDATE users SET role='admin', verified=1, updated_at=$2 WHERE id=$1")
            .bind(id)
            .bind(&now)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("INSERT INTO users (id,email,phone,name,role,verified,created_at,updated_at) VALUES ($1,$2,$3,'Admin','admin',1,$4,$4)")
            .bind(Uuid::new_v4().to_string())
            .bind(Some("admin@local".to_string()))
            .bind(DEV_ADMIN_PHONE)
            .bind(&now)
            .execute(pool)
            .await?;
    }
    Ok(())
}

fn user_from_row(row: &sqlx::postgres::PgRow) -> ApiResult<PublicUser> {
    let email: Option<String> = row.try_get("email").map_err(ApiError::internal)?;
    let phone: Option<String> = row.try_get("phone").map_err(ApiError::internal)?;
    let display_email = email.clone().or_else(|| phone.clone()).unwrap_or_default();
    Ok(PublicUser {
        id: row.try_get("id").map_err(ApiError::internal)?,
        email: display_email,
        phone,
        name: row.try_get("name").unwrap_or_default(),
        role: row.try_get("role").map_err(ApiError::internal)?,
    })
}
