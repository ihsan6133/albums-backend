use std::io;

use axum::{Json, Router, body::Body, debug_handler, extract::{Path, Query, State}, http::{Response, StatusCode, header}, response::IntoResponse};
use mime_guess::MimeGuess;
use tokio::task;
use rusqlite::params;
use tokio_util::io::ReaderStream;
use tower_http::{compression::CompressionLayer};

use crate::{database::{actions, models::MediaItem}, services::{self, media::{MediaQueryParams, MediaResponse}}, state::AppState, utils::{error::{HttpError, ToHttpError}, get_sharded_path}};



async fn serve_file<P: AsRef<std::path::Path>>(path: P) -> Result<Response<Body>, io::Error>{
    let file = tokio::fs::File::open(&path).await?;
    // let metadata = file.metadata().await?;
    // let content_length = metadata.len();

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = MimeGuess::from_path(&path).first_raw();
    
    let mut builder = Response::builder()
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        // .header(header::CONTENT_LENGTH, content_length);
    ;
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }

    Ok(builder.body(body).unwrap())
}


#[debug_handler]
async fn get_media(State(state): State<AppState>, Query(params): Query<MediaQueryParams>) -> Result<Json<MediaResponse>, HttpError> {
    let res = services::media::get_media(state, params, None).await?;

    Ok(Json(res))
}

#[debug_handler]
async fn get_media_item(State(state): State<AppState>, Path(id): Path<i32>) -> Result<Json<MediaItem>, HttpError> {  
    let item = task::spawn_blocking(move || -> Result<MediaItem, HttpError> {
        let conn = state.pool.get().map_http(StatusCode::INTERNAL_SERVER_ERROR)?;

        let query = "SELECT * FROM media WHERE id = ?1";
        let mut stmt = conn.prepare_cached(query).map_http(StatusCode::BAD_GATEWAY)?;
        let res = stmt.query_one(params![id], |row| MediaItem::from_row(row));   
        res.map_http(StatusCode::NOT_FOUND)

    }).await.map_err(|_| HttpError::from_code(StatusCode::INTERNAL_SERVER_ERROR))??;


    Ok(Json(item))

}

#[debug_handler]
async fn get_media_file(State(state): State<AppState>, Path(id): Path<i32>) -> Result<impl IntoResponse, HttpError> {
    let path = task::spawn_blocking(move || -> Result<String, HttpError>{
        let conn = state.pool.get().map_http(StatusCode::INTERNAL_SERVER_ERROR)?;

        actions::get_media_path(&conn, id).map_http(StatusCode::NOT_FOUND)
    }).await.map_http(StatusCode::INTERNAL_SERVER_ERROR)??;

    serve_file(path).await.map_http(StatusCode::NOT_FOUND)
}

#[debug_handler]
async fn get_media_thumb(State(state): State<AppState>, Path(id): Path<i32>) -> Result<impl IntoResponse, HttpError> {
    let thumb_path = get_sharded_path(&state.config.thumbs_dir, id as u64, ".jpg");


    let res = serve_file(&thumb_path).await;

    match res {
        Ok(res) => Ok(res),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(HttpError::new(StatusCode::NOT_FOUND, "Thumbnail not found".to_owned()))
        },
        Err(_) => Err(HttpError::new(StatusCode::INTERNAL_SERVER_ERROR, "Error accessing thumbnail".to_owned()))
    }

}

pub fn router() -> axum::Router<AppState> {

    let compressed = Router::new()
        .route("/", axum::routing::get(get_media))
        .layer(CompressionLayer::new().gzip(true).zstd(true).quality(tower_http::CompressionLevel::Fastest));


    Router::new()
        .merge(compressed)
        .route("/{id}", axum::routing::get(get_media_item))
        .route("/{id}/file", axum::routing::get(get_media_file))
        .route("/{id}/thumb", axum::routing::get(get_media_thumb))

}