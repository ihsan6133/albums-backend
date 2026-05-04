use std::{path::PathBuf, sync::Arc};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::processor::indexer::SharedStatus;


pub struct Config {
    pub db_path: PathBuf,
    pub thumbs_dir: PathBuf,
    pub media_root: PathBuf,
}


#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub indexing_status: SharedStatus,
    pub pool: Pool<SqliteConnectionManager>
}
