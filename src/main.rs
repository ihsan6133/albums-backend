use std::{error::Error, fs::File, sync::Arc};

use albums::{database::actions::init_media_db, logger, processor::indexer::{IndexingStatus, SharedStatus, start_media_indexing}, routes, state::{AppState, Config}};
use axum::{Router, routing};
use chrono::{DateTime, Utc};
use log::{info, error};
use tokio::{fs, sync::RwLock};
use r2d2_sqlite::SqliteConnectionManager;
use tower_http::services::{ServeDir, ServeFile};




#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    logger::init(File::create("LOG.log")?);
    log::set_max_level(log::LevelFilter::Info);
    
    let db_path = "db/media.db".into();
    let media_root = "E:\\media".into();
    let thumbs_dir = "thumbs".into();

    let config = Config { db_path, media_root, thumbs_dir };

    let _ = init_media_db(&config.db_path)?;

    let indexing_status = SharedStatus::new(RwLock::new(IndexingStatus::default()));
    // let mut indexer = start_media_indexing(&config.media_root, &config.db_path, indexing_status.clone());


    let manager = SqliteConnectionManager::file(&config.db_path);
    let pool = r2d2::Pool::builder()
        .max_size(10)
        .build(manager)?;

    let app = Router::new()
        .nest("/api/media" , routes::media::router())
        .nest("/api/system", routes::system::router())
        .nest("/api/albums", routes::albums::router())
        .fallback_service(ServeDir::new("../dist"))
        .with_state(AppState { config: Arc::new(config), indexing_status, pool });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    let server = axum::serve(listener, app);

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal, stopping program...");
        }

        res = server => {
            if let Err(e) = res {
                error!("Server error: {}", e);
            }
        }

        // res = indexer.wait_for_error() => {
        //     error!("Indexing error: {}", res);
        // }
    }

    // indexer.stop();
    // indexer.wait().await?;

    log::logger().flush();
    Ok(())
}
