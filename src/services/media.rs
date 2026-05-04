use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{database::actions::{self, MediaData, SortOptions, SortOrder}, state::AppState, utils::error::HttpError};

#[derive(Serialize, Deserialize)]
pub struct MediaQueryParams {
    limit: Option<u32>,
    offset: Option<u32>,
    sort  : Option<String>,
    layout: Option<bool>
}


#[derive(Serialize)]
pub struct Pagination {
    // total_items: u32,
    limit: u32,
    offset: u32,
    // has_more: bool
}


#[derive(Serialize)]
pub struct MediaResponse {
    data: MediaData,
    pagination: Pagination,
}


fn parse_sort_options(sort: Option<String>) -> Option<SortOptions> 
{
    sort.map(|sort| {
        let mut sort = sort;
        let dir;

        if sort.starts_with('-') {
            dir = SortOrder::Desc;
            sort.remove(0);
        } else {
            dir = SortOrder::Asc;
        }
        
        SortOptions {
            sort_by: sort,
            sort_order: dir
        }
    })
}

pub async fn get_media(state: AppState, params: MediaQueryParams, album_id: Option<i64>) -> Result<MediaResponse, HttpError> {
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    let layout = params.layout.unwrap_or(false);
    let sort_options = parse_sort_options(params.sort);

    let pagination = Pagination {
        offset,
        limit, 
    };
    
    let data = tokio::task::spawn_blocking(move || -> Result<MediaData, anyhow::Error> {
        let conn = state.pool.get()?;
        actions::get_media(&conn, limit as usize, offset as usize, album_id, sort_options, layout)

    }).await.map_err(|_| HttpError::from_code(StatusCode::INTERNAL_SERVER_ERROR))?.map_err(|e| HttpError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) )?;


    Ok(MediaResponse {pagination, data})

} 