use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
#[derive(Debug, Deserialize)]
pub struct VerifyOtpRequest {
    pub email: String,
    #[serde(rename = "otpCode")]
    pub otp_code: String,
}
#[derive(Debug, Deserialize)]
pub struct ResetRequest {
    #[serde(rename = "resetToken")]
    pub reset_token: String,
    #[serde(rename = "newPassword")]
    pub new_password: String,
}
#[derive(Debug, Deserialize)]
pub struct ResetRequestStart {
    pub email: String,
}
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub user: PublicUser,
}
#[derive(Debug, Serialize)]
pub struct PublicUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
}
