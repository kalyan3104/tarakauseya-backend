use crate::{
    AppState, db,
    error::{ApiError, ApiResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

const ALLOWED: &[&str] = &[
    "products",
    "collections",
    "inventory",
    "inventory-logs",
    "media-assets",
    "trial-requests",
    "service-areas",
];
#[derive(Deserialize)]
pub struct FilterRequest {
    pub query: Option<Value>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
}
fn valid(entity: &str) -> ApiResult<()> {
    if ALLOWED.contains(&entity) {
        Ok(())
    } else {
        Err(ApiError::not_found("Unknown entity"))
    }
}

pub async fn list(
    State(state): State<AppState>,
    Path(entity): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    valid(&entity)?;
    let mut values = db::records(&state.pool, &entity).await?;
    sort(
        &mut values,
        params
            .get("sort")
            .map(String::as_str)
            .unwrap_or("-created_date"),
    );
    values.truncate(
        params
            .get("limit")
            .and_then(|n| n.parse().ok())
            .unwrap_or(100)
            .min(500),
    );
    Ok(Json(json!(values)))
}
pub async fn filter(
    State(state): State<AppState>,
    Path(entity): Path<String>,
    Json(input): Json<FilterRequest>,
) -> ApiResult<Json<Value>> {
    valid(&entity)?;
    let query = input.query.unwrap_or_else(|| json!({}));
    let mut values: Vec<Value> = db::records(&state.pool, &entity)
        .await?
        .into_iter()
        .filter(|v| matches(v, &query))
        .collect();
    sort(
        &mut values,
        input.sort.as_deref().unwrap_or("-created_date"),
    );
    values.truncate(input.limit.unwrap_or(100).min(500));
    Ok(Json(json!(values)))
}
pub async fn create(
    State(state): State<AppState>,
    Path(entity): Path<String>,
    Json(mut value): Json<Value>,
) -> ApiResult<Json<Value>> {
    valid(&entity)?;
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = Utc::now().to_rfc3339();
    let object = value
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("Entity must be a JSON object"))?;
    object.insert("id".into(), json!(id));
    object.entry("created_date").or_insert(json!(now));
    object.insert("updated_date".into(), json!(Utc::now().to_rfc3339()));
    db::save(
        &state.pool,
        &entity,
        &id,
        &value,
        value.get("created_date").and_then(Value::as_str),
    )
    .await?;
    Ok(Json(value))
}
pub async fn update(
    State(state): State<AppState>,
    Path((entity, id)): Path<(String, String)>,
    Json(patch): Json<Value>,
) -> ApiResult<Json<Value>> {
    valid(&entity)?;
    let mut value = db::records(&state.pool, &entity)
        .await?
        .into_iter()
        .find(|v| v.get("id").and_then(Value::as_str) == Some(&id))
        .ok_or_else(|| ApiError::not_found("Record not found"))?;
    let source = patch
        .as_object()
        .ok_or_else(|| ApiError::bad_request("Entity patch must be a JSON object"))?;
    let object = value.as_object_mut().expect("stored entity object");
    for (key, item) in source {
        if key != "id" && key != "created_date" {
            object.insert(key.clone(), item.clone());
        }
    }
    object.insert("updated_date".into(), json!(Utc::now().to_rfc3339()));
    db::save(
        &state.pool,
        &entity,
        &id,
        &value,
        value.get("created_date").and_then(Value::as_str),
    )
    .await?;
    Ok(Json(value))
}
pub async fn delete(
    State(state): State<AppState>,
    Path((entity, id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    valid(&entity)?;
    let result = sqlx::query("DELETE FROM entities WHERE entity_type=$1 AND id=$2")
        .bind(&entity)
        .bind(&id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("Record not found"));
    }
    Ok(Json(json!({"deleted":true})))
}
pub async fn bulk_create(
    State(state): State<AppState>,
    Path(entity): Path<String>,
    Json(values): Json<Vec<Value>>,
) -> ApiResult<Json<Value>> {
    valid(&entity)?;
    let mut created = 0;
    for value in values {
        let _ = create(State(state.clone()), Path(entity.clone()), Json(value)).await?;
        created += 1;
    }
    Ok(Json(json!({"created":created})))
}
fn matches(item: &Value, query: &Value) -> bool {
    query
        .as_object()
        .map(|q| {
            q.iter()
                .all(|(key, expected)| item.get(key) == Some(expected))
        })
        .unwrap_or(true)
}
fn sort(values: &mut [Value], directive: &str) {
    let desc = directive.starts_with('-');
    let field = directive.trim_start_matches('-');
    values.sort_by(|a, b| {
        let order = a
            .get(field)
            .map(Value::to_string)
            .cmp(&b.get(field).map(Value::to_string));
        if desc { order.reverse() } else { order }
    });
}
