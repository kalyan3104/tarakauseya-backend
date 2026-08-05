use axum::Json;
use serde_json::{Value, json};
pub async fn check_health() -> Json<Value> {
    Json(json!({"status":"ok", "service":"tara-kauseya-api"}))
}
