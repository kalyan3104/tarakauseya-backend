use crate::{
    AppState,
    error::{ApiError, ApiResult},
    middleware::auth::{AuthUser, issue_token},
    models::auth::{
        AuthResponse, LoginRequest, PublicUser, RegisterRequest, ResetRequest, ResetRequestStart,
        VerifyOtpRequest,
    },
};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{Json, extract::State};
use chrono::{Duration, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<RegisterRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    validate_credentials(&input.email, &input.password)?;
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE email=$1")
        .bind(input.email.trim().to_lowercase())
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if exists.is_some() {
        return Err(ApiError::bad_request(
            "An account already exists for this email",
        ));
    }
    let password_hash = hash_password(&input.password)?;
    let code = development_code();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO users (id,email,password_hash,name,role,verified,verification_code,created_at,updated_at) VALUES ($1,$2,$3,'','customer',0,$4,$5,$5)")
        .bind(Uuid::new_v4().to_string()).bind(input.email.trim().to_lowercase()).bind(password_hash).bind(&code).bind(now).execute(&state.pool).await.map_err(ApiError::internal)?;
    // Wire a transactional email provider here in production. The code keeps local development usable.
    Ok(Json(
        serde_json::json!({"ok":true, "message":"Verification code sent", "development_verification_code": code}),
    ))
}
pub async fn verify_otp(
    State(state): State<AppState>,
    Json(input): Json<VerifyOtpRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let row =
        sqlx::query("SELECT id,email,name,role FROM users WHERE email=$1 AND verification_code=$2")
            .bind(input.email.trim().to_lowercase())
            .bind(input.otp_code)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::bad_request("Invalid verification code"))?;
    let user = user_from_row(&row)?;
    sqlx::query("UPDATE users SET verified=1, verification_code=NULL, updated_at=$2 WHERE id=$1")
        .bind(&user.id)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let token = issue_token(&state, &user.id, &user.email, &user.role)?;
    Ok(Json(AuthResponse {
        access_token: token,
        user,
    }))
}
pub async fn resend_otp(
    State(state): State<AppState>,
    Json(input): Json<ResetRequestStart>,
) -> ApiResult<Json<serde_json::Value>> {
    let code = development_code();
    let updated = sqlx::query(
        "UPDATE users SET verification_code=$1, updated_at=$2 WHERE email=$3 AND verified=0",
    )
    .bind(&code)
    .bind(Utc::now().to_rfc3339())
    .bind(input.email.trim().to_lowercase())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::bad_request(
            "No unverified account exists for this email",
        ));
    }
    Ok(Json(
        serde_json::json!({"ok":true,"development_verification_code":code}),
    ))
}
pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let row =
        sqlx::query("SELECT id,email,password_hash,name,role,verified FROM users WHERE email=$1")
            .bind(input.email.trim().to_lowercase())
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::unauthorized("Invalid email or password"))?;
    let hash: String = row.try_get("password_hash").map_err(ApiError::internal)?;
    let parsed = PasswordHash::new(&hash).map_err(ApiError::internal)?;
    Argon2::default()
        .verify_password(input.password.as_bytes(), &parsed)
        .map_err(|_| ApiError::unauthorized("Invalid email or password"))?;
    // `verified` is declared as an INTEGER in the PostgreSQL migration, which
    // SQLx decodes as an i32 (not an i64).
    let verified: i32 = row.try_get("verified").map_err(ApiError::internal)?;
    if verified == 0 {
        return Err(ApiError::unauthorized(
            "Please verify your email before logging in",
        ));
    }
    let user = user_from_row(&row)?;
    let token = issue_token(&state, &user.id, &user.email, &user.role)?;
    Ok(Json(AuthResponse {
        access_token: token,
        user,
    }))
}
pub async fn me(AuthUser(claims): AuthUser) -> Json<PublicUser> {
    Json(PublicUser {
        id: claims.sub,
        email: claims.email,
        name: String::new(),
        role: claims.role,
    })
}
pub async fn logout() -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok":true}))
}
pub async fn reset_password_request(
    State(state): State<AppState>,
    Json(input): Json<ResetRequestStart>,
) -> ApiResult<Json<serde_json::Value>> {
    let token = Uuid::new_v4().to_string();
    let expiry = (Utc::now() + Duration::hours(1)).to_rfc3339();
    sqlx::query(
        "UPDATE users SET reset_token=$1, reset_expires_at=$2, updated_at=$3 WHERE email=$4",
    )
    .bind(&token)
    .bind(expiry)
    .bind(Utc::now().to_rfc3339())
    .bind(input.email.trim().to_lowercase())
    .execute(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    Ok(Json(
        serde_json::json!({"ok":true, "development_reset_token":token}),
    ))
}
pub async fn reset_password(
    State(state): State<AppState>,
    Json(input): Json<ResetRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if input.new_password.len() < 8 {
        return Err(ApiError::bad_request(
            "Password must have at least 8 characters",
        ));
    }
    let hash = hash_password(&input.new_password)?;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query("UPDATE users SET password_hash=$1, reset_token=NULL, reset_expires_at=NULL, updated_at=$2 WHERE reset_token=$3 AND reset_expires_at>$2").bind(hash).bind(now).bind(input.reset_token).execute(&state.pool).await.map_err(ApiError::internal)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::bad_request("Invalid or expired reset link"));
    }
    Ok(Json(serde_json::json!({"ok":true})))
}

#[derive(Debug, Deserialize)]
struct SupaClaims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
    pub role: Option<String>,
}

pub async fn supabase_exchange(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
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
    let email = decoded.claims.email;

    // Find or create local user
    let row_opt = sqlx::query("SELECT id,email,name,role,verified FROM users WHERE email=$1")
        .bind(email.trim().to_lowercase())
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let user = if let Some(row) = row_opt {
        user_from_row(&row)?
    } else {
        // Create a local user record for this Supabase user. Generate a random password hash.
        let random_password = Uuid::new_v4().to_string();
        let password_hash = hash_password(&random_password)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO users (id,email,password_hash,name,role,verified,created_at,updated_at) VALUES ($1,$2,$3,'', 'customer',1,$4,$4)")
            .bind(&id)
            .bind(email.trim().to_lowercase())
            .bind(password_hash)
            .bind(now)
            .execute(&state.pool)
            .await
            .map_err(ApiError::internal)?;
        // Return created user
        let row = sqlx::query("SELECT id,email,name,role,verified FROM users WHERE id=$1").bind(id).fetch_one(&state.pool).await.map_err(ApiError::internal)?;
        user_from_row(&row)?
    };

    let token = issue_token(&state, &user.id, &user.email, &user.role)?;
    Ok(Json(AuthResponse { access_token: token, user }))
}
fn validate_credentials(email: &str, password: &str) -> ApiResult<()> {
    if !email.contains('@') {
        return Err(ApiError::bad_request("Enter a valid email address"));
    }
    if password.len() < 8 {
        return Err(ApiError::bad_request(
            "Password must have at least 8 characters",
        ));
    }
    Ok(())
}
fn hash_password(password: &str) -> ApiResult<String> {
    let seed = Uuid::new_v4();
    let salt = SaltString::encode_b64(seed.as_bytes()).map_err(ApiError::internal)?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|p| p.to_string())
        .map_err(ApiError::internal)
}
fn development_code() -> String {
    "000000".to_string()
}

const DEV_ADMIN_EMAIL: &str = "kalyannchowdaryy@gmail.com";
const DEV_ADMIN_PWD: &str = "Kalyan@8899";

pub async fn seed_dev_admin(pool: &PgPool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let existing: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE email=$1")
        .bind(DEV_ADMIN_EMAIL)
        .fetch_optional(pool)
        .await?;
    let password_hash = hash_password(DEV_ADMIN_PWD)?;
    let now = Utc::now().to_rfc3339();

    if let Some(id) = existing {
        sqlx::query("UPDATE users SET password_hash=$1, role='admin', verified=1, updated_at=$2 WHERE id=$3")
            .bind(password_hash)
            .bind(&now)
            .bind(id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("INSERT INTO users (id,email,password_hash,name,role,verified,created_at,updated_at) VALUES ($1,$2,$3,'Admin','admin',1,$4,$4)")
            .bind(Uuid::new_v4().to_string())
            .bind(DEV_ADMIN_EMAIL)
            .bind(password_hash)
            .bind(&now)
            .execute(pool)
            .await?;
    }
    Ok(())
}

fn user_from_row(row: &sqlx::postgres::PgRow) -> ApiResult<PublicUser> {
    Ok(PublicUser {
        id: row.try_get("id").map_err(ApiError::internal)?,
        email: row.try_get("email").map_err(ApiError::internal)?,
        name: row.try_get("name").unwrap_or_default(),
        role: row.try_get("role").map_err(ApiError::internal)?,
    })
}
