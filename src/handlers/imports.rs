use crate::{
    AppState, db,
    error::{ApiError, ApiResult},
};
use axum::{Json, extract::State};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ImportRequest {
    pub file_url: String,
}
pub async fn bulk_import_products(
    State(state): State<AppState>,
    Json(input): Json<ImportRequest>,
) -> ApiResult<Json<Value>> {
    let filename = input
        .file_url
        .strip_prefix("/uploads/")
        .ok_or_else(|| ApiError::bad_request("Invalid upload URL"))?;
    if filename.contains('/') || filename.contains("..") {
        return Err(ApiError::bad_request("Invalid upload URL"));
    }
    let content = tokio::fs::read_to_string(state.config.uploads_dir.join(filename))
        .await
        .map_err(|_| ApiError::bad_request("Uploaded CSV could not be read"))?;
    let mut rows = content.lines();
    let headers: Vec<String> = rows
        .next()
        .ok_or_else(|| ApiError::bad_request("CSV has no header row"))?
        .split(',')
        .map(|h| h.trim().trim_matches('\u{feff}').to_string())
        .collect();
    let mut products_created = 0;
    let mut inventory_created = 0;
    let mut skipped = Vec::new();
    let mut total_rows = 0;
    for (line_number, line) in rows.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        total_rows += 1;
        let values = parse_csv_line(line);
        let mut row = serde_json::Map::new();
        for (index, header) in headers.iter().enumerate() {
            row.insert(
                header.clone(),
                json!(values.get(index).cloned().unwrap_or_default()),
            );
        }
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let sku = row
            .get("sku")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let price = row
            .get("price")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() || sku.is_empty() || price.parse::<f64>().is_err() {
            skipped.push(
                json!({"row":line_number + 2, "reason":"name, sku and numeric price are required"}),
            );
            continue;
        }
        let product_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let input_slug = row
            .get("slug")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&name)
            .to_string();
        let active = parse_bool(row.get("active").and_then(Value::as_str).unwrap_or("true"));
        row.insert("id".into(), json!(product_id));
        row.insert("name".into(), json!(name));
        row.insert("sku".into(), json!(sku));
        row.insert(
            "price".into(),
            json!(price.parse::<f64>().unwrap_or_default()),
        );
        row.insert("slug".into(), json!(slug(&input_slug)));
        row.insert("active".into(), json!(active));
        row.insert("created_date".into(), json!(now));
        row.insert("updated_date".into(), json!(Utc::now().to_rfc3339()));
        let product = Value::Object(row);
        db::save(
            &state.pool,
            "products",
            &product_id,
            &product,
            product.get("created_date").and_then(Value::as_str),
        )
        .await?;
        products_created += 1;
        let inventory_id = Uuid::new_v4().to_string();
        let inventory = json!({"id":inventory_id,"product_id":product_id,"sku":sku,"stock_quantity":number(&product,"stock_quantity"),"reserved":number(&product,"reserved"),"incoming":number(&product,"incoming"),"minimum_stock":number(&product,"minimum_stock"),"warehouse_location":product.get("warehouse_location").cloned().unwrap_or(json!("")),"created_date":Utc::now().to_rfc3339(),"updated_date":Utc::now().to_rfc3339()});
        db::save(
            &state.pool,
            "inventory",
            &inventory_id,
            &inventory,
            inventory.get("created_date").and_then(Value::as_str),
        )
        .await?;
        inventory_created += 1;
    }
    Ok(Json(
        json!({"total_rows":total_rows,"products_created":products_created,"inventory_created":inventory_created,"skipped":skipped}),
    ))
}
fn number(value: &Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}
fn parse_bool(value: &str) -> bool {
    matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}
fn slug(input: &str) -> String {
    input
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '"' if quoted && chars.get(i + 1) == Some(&'"') => {
                current.push('"');
                i += 1;
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                values.push(current.trim().to_string());
                current.clear();
            }
            c => current.push(c),
        }
        i += 1;
    }
    values.push(current.trim().to_string());
    values
}
