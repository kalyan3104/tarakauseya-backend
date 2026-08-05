use crate::{
    AppState,
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Multipart, State},
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    while let Some(field) = multipart.next_field().await.map_err(ApiError::internal)? {
        if field.name() != Some("file") {
            continue;
        }
        let original = field.file_name().unwrap_or("upload").to_string();
        let extension = std::path::Path::new(&original)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("bin")
            .to_string();
        let safe = format!(
            "{}.{}",
            Uuid::new_v4(),
            extension
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
        );
        let bytes = field.bytes().await.map_err(ApiError::internal)?;
        if bytes.len() > 25 * 1024 * 1024 {
            return Err(ApiError::bad_request("Files must be 25 MB or smaller"));
        }
        let path = state.config.uploads_dir.join(&safe);
        let mut file = tokio::fs::File::create(path)
            .await
            .map_err(ApiError::internal)?;
        file.write_all(&bytes).await.map_err(ApiError::internal)?;
        return Ok(Json(
            serde_json::json!({"file_url": format!("/uploads/{safe}"), "name": original}),
        ));
    }
    Err(ApiError::bad_request("A file field is required"))
}
