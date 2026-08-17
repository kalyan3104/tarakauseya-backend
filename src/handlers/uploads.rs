use crate::{
    AppState,
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Multipart, State},
    http::header,
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
        let content_type = field
            .content_type()
            .map(|content_type| content_type.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
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
        if bytes.len() > 20 * 1024 * 1024 {
            return Err(ApiError::bad_request("Files must be 20 MB or smaller"));
        }
        let file_url = if let Some(storage) = &state.config.supabase_storage {
            let upload_url = format!("{}/storage/v1/object/{}/{}", storage.url, storage.bucket, safe);
            let response = reqwest::Client::new()
                .put(upload_url)
                .header("apikey", &storage.service_role_key)
                .bearer_auth(&storage.service_role_key)
                .header(header::CONTENT_TYPE, content_type)
                .body(bytes)
                .send()
                .await
                .map_err(ApiError::internal)?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::error!(%status, %body, "Supabase Storage upload failed");
                return Err(ApiError::internal("Supabase Storage upload failed"));
            }

            format!(
                "{}/storage/v1/object/public/{}/{}",
                storage.url, storage.bucket, safe
            )
        } else {
            let path = state.config.uploads_dir.join(&safe);
            let mut file = tokio::fs::File::create(path)
                .await
                .map_err(ApiError::internal)?;
            file.write_all(&bytes).await.map_err(ApiError::internal)?;
            format!("/uploads/{safe}")
        };
        return Ok(Json(
            serde_json::json!({"file_url": file_url, "name": original}),
        ));
    }
    Err(ApiError::bad_request("A file field is required"))
}
