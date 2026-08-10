use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub user: PublicUser,
}
#[derive(Debug, Serialize)]
pub struct PublicUser {
    pub id: String,
    pub email: String,
    pub phone: Option<String>,
    pub name: String,
    pub role: String,
}
