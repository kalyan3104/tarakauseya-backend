use crate::{
    AppState,
    error::{ApiError, ApiResult},
};
use axum::{extract::FromRequestParts, http::request::Parts};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
}
pub struct AuthUser(pub Claims);

pub fn issue_token(state: &AppState, id: &str, email: &str, role: &str) -> ApiResult<String> {
    encode(
        &Header::default(),
        &Claims {
            sub: id.into(),
            email: email.into(),
            role: role.into(),
            exp: (Utc::now() + Duration::days(7)).timestamp() as usize,
        },
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(ApiError::internal)
}
#[axum::async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("Authentication required"))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::unauthorized("Invalid authorization header"))?;
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| ApiError::unauthorized("Invalid or expired session"))?
        .claims;
        Ok(Self(claims))
    }
}
