use axum::{Json, Router, extract::{Path, Query, State}, http::StatusCode};

use crate::{database::{actions, models::Album}, services::{self, media::{MediaQueryParams, MediaResponse}}, state::AppState, utils::error::{HttpError, ToHttpError}};


type AlbumResponse = Vec<Album>;

async fn get_albums(State(state): State<AppState>) -> Result<Json<AlbumResponse>, HttpError> {
    let albums = tokio::task::spawn_blocking(move || -> Result<AlbumResponse, HttpError> {

        let conn = state.pool.get().map_http(StatusCode::INTERNAL_SERVER_ERROR)?;

        let albums = actions::get_albums(&conn).map_http(StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(albums)

    }).await.map_http(StatusCode::INTERNAL_SERVER_ERROR)??;

    Ok(Json(albums))
}

async fn get_album(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<Album>, HttpError> {
    let album = tokio::task::spawn_blocking(move || -> Result<Album, HttpError> {
        let conn = state.pool.get().map_http(StatusCode::INTERNAL_SERVER_ERROR)?;
        let album = actions::get_album(&conn, id).map_http(StatusCode::NOT_FOUND)?;


        Ok(album)

    }).await.map_http(StatusCode::INTERNAL_SERVER_ERROR)??;

    Ok(Json(album))
}

async fn get_album_media(State(state): State<AppState>, Query(params): Query<MediaQueryParams>, Path(id): Path<i64>) -> Result<Json<MediaResponse>, HttpError> {
    let res = services::media::get_media(state, params, Some(id)).await?;
    Ok(Json(res))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(get_albums))
        .route("/{id}", axum::routing::get(get_album))
        .route("/{id}/media", axum::routing::get(get_album_media))
}