use std::fs;

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;
use crate::{state::AppState, utils::error::{HttpError, ToHttpError}};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")] // Apply the container attribute
pub struct LibraryStats {
    pub total_count: i32,
    pub total_size_bytes: i64,
    pub photos_count: i32,
    pub videos_count: i32,
    pub missing_files: i32,
    pub db_size_bytes: u64,
}


async fn get_stats(State(state): State<AppState>) -> Result<Json<LibraryStats>, HttpError>  {
    let stats = tokio::task::spawn_blocking(move || -> Result<LibraryStats, HttpError> {

        let conn = state.pool.get().map_http(StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut stmt = conn.prepare_cached("SELECT count(*), sum(size), sum(missing) FROM media;").map_http(StatusCode::INTERNAL_SERVER_ERROR)?;
        let (total, size, missing): (i32, i64, i32) = stmt.query_one([], |row| Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
        ))).map_http(StatusCode::INTERNAL_SERVER_ERROR)?;

        let len = fs::metadata("db/media.db").map_http(StatusCode::INTERNAL_SERVER_ERROR)?.len();

        Ok(LibraryStats {
            total_count: total,
            total_size_bytes: size,
            photos_count: 0,
            videos_count: 0,
            missing_files: missing,
            db_size_bytes: len
        })
        
    } ).await.map_err(|_| HttpError::from_code(StatusCode::INTERNAL_SERVER_ERROR))??;


    Ok(Json(stats))
}


pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stats", get(get_stats))
}