use std::{os::windows::fs::MetadataExt, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}}, thread};

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use indicatif::{ProgressBar, ProgressStyle};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use tokio::sync::{RwLock, oneshot};
use walkdir::{DirEntry, WalkDir};

use log::{debug, error, trace};
use crate::{processor::metadata::get_media_metadata, utils::extensions::{PHOTO_EXTENSIONS}};

#[derive(Default, Serialize, Clone)]
pub struct IndexingStatus {
    pub total_files: usize,
    pub indexed_files: usize,
    pub last_indexed: Option<String>,
    pub is_indexing: bool,
    pub last_error: Option<String>
}

pub type SharedStatus = Arc<RwLock<IndexingStatus>>;

pub struct IndexerHandle {
    stop_signal: Arc<AtomicBool>,
    pub result_rx: oneshot::Receiver<anyhow::Result<()>>
}

impl IndexerHandle {
    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }

    pub async fn wait(&mut self) -> anyhow::Result<()> {
        if self.result_rx.is_terminated() {
            return Ok(());
        } else {
            (&mut self.result_rx).await?
        }
    }

    pub async fn wait_for_error(&mut self) -> anyhow::Error {
        let result = (&mut self.result_rx).await;
        match result {
            Ok(Err(e)) => anyhow::anyhow!("{}", e),
            Err(_) => anyhow!("Indexer thread panicked or was cancelled"),
            Ok(Ok(())) => std::future::pending().await, 
        }
    }
}


fn entry_filter(e: &DirEntry) -> bool {

    if !e.file_type().is_file() {return true};

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        if let Ok(meta) = e.metadata() {
            let attrs = meta.file_attributes();
            if attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
                return false
            }
        }
    }
    true
}

fn count_files<P: AsRef<Path>>(dir: P) -> anyhow::Result<usize> {
    let mut count = 0;
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template(
        "{spinner:.green} {msg}"
    )?);
    pb.set_message("Calculating files");

    for entry in WalkDir::new(dir).into_iter().filter_entry(entry_filter) {
        let Ok(entry) = entry else {continue};

        if entry.file_type().is_dir() {
            pb.set_message(format!("Calculating files {}", entry.path().display()));
        }
        let Some(ext) = entry.path().extension().and_then(|e| e.to_str()).and_then(|e| Some( e.to_ascii_lowercase()) ) else { 
            continue;
        };

        if PHOTO_EXTENSIONS.contains(&ext.as_ref()) {
            count += 1;
            pb.tick();
        }
    }

    pb.finish_with_message(format!("Found {} files.", count));
    Ok(count)
}


pub fn run_indexer(root: PathBuf, db: PathBuf, stop_signal: Arc<AtomicBool>, status: SharedStatus) 
    -> anyhow::Result<()> 
{
    trace!("Started Indexing");
    let total = count_files(&root)?;
    trace!("Total files to index: {total}");

    let mut conn = Connection::open(db)?;
    
    conn.pragma_update(None, "journal_mode", "WAL")?;

    let mut tx = conn.transaction()?;
    let batch_size = 2000;

    let pb = ProgressBar::new(total as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green}a [{elapsed_precise}] {bar:40.cyan/white} {pos}/{len} ({percent_precise}%) {msg}")?
        .progress_chars("━─")
    );

    {
        let mut status = status.blocking_write();
        status.total_files = total;
        status.is_indexing = true;
        status.indexed_files = 0;
        status.last_indexed = None;
    }

    let mut count = 0;        
    {
        let mut check_stmt = tx.prepare("SELECT mtime, size FROM media WHERE path = ?")?;
        let mut insert_stmt = tx.prepare("INSERT OR REPLACE INTO media (path, size, mtime, timestamp, indexed_at, taken_at, width, height, duration, missing) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?; 
        

        for entry in WalkDir::new(root).into_iter().filter_entry(entry_filter) {

            if stop_signal.load(Ordering::Relaxed) { break; }
            
            let entry = entry?;
            if entry.file_type().is_dir() {
                pb.set_message(format!("Indexing {}", entry.path().display()));
            }
            if ! entry.file_type().is_file() { continue }

            let Ok(file_meta) = entry.metadata().map_err(|e: walkdir::Error| {
                error!("{e}");
            }) else { continue };

            let Some(ext) = entry.path().extension().and_then(|e| e.to_str()).and_then(|e| Some( e.to_ascii_lowercase()) ) else { 
                continue;
            };
            
            
            let path = entry.path().to_str().ok_or_else(|| anyhow!("Non UTF-8 path"))?;
            let size = file_meta.file_size() as i64;
            let mtime: DateTime<Utc> = file_meta.modified()?.into();
            let mtime = format!("{}", mtime.format("%Y-%m-%dT%H:%M:%S.%3fZ"));
                    
            
            if  !PHOTO_EXTENSIONS.contains(&ext.as_str()) { continue };  

            pb.inc(1);
            // We check if it already exists.

            let res: Option<(String, i64)> = check_stmt.query_row(params![path], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;

            if let Some((r_mtime, r_size)) = res && r_mtime == mtime && r_size == size {
                // debug!("Already Exists");
                continue;
            }



            let Ok(meta) = get_media_metadata(entry.path(), &ext, &file_meta).map_err(|e| {
                debug!("{}: {}", entry.path().display(),  e);
            }) else {continue};

            // we have metadata
            
            let indexed_at = format!("{}", Utc::now().format("%Y-%m-%dT%H:%M:%S.%3fZ"));
            let timestamp = meta.taken_at.to_normalized_utc_string();
            let taken_at = meta.taken_at.to_string();
            let width = meta.width;
            let height = meta.height;
            let duration = meta.duration;
            let missing = 0;

            insert_stmt.execute(params![path, size, mtime, timestamp, indexed_at, taken_at, width, height, duration, missing])?;
            count += 1;

            {
                let mut status = status.blocking_write();
                status.indexed_files += 1;
                status.last_indexed = Some(path.to_string());
            }


            if count % batch_size == 0 {
                drop(check_stmt);
                drop(insert_stmt);

                tx.commit()?;

                tx = conn.transaction()?;
                check_stmt = tx.prepare("SELECT mtime, size FROM media WHERE path = ?")?;
                insert_stmt = tx.prepare("INSERT OR REPLACE INTO media (path, size, mtime, timestamp, indexed_at, taken_at, width, height, duration, missing) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?; 

                pb.reset_eta();
            }

        }    
    }

    pb.finish_with_message(format!("Indexed {} new files", count));
    
    tx.commit()?;

    {
        let mut status = status.blocking_write();
        status.is_indexing = false;
    }
    Ok(())

}


pub fn start_media_indexing<P: AsRef<Path>>(root: P, db: P, status: SharedStatus) -> IndexerHandle { 
    let stop_signal = Arc::new(AtomicBool::new(false));
    let thread_stop_signal = stop_signal.clone();

    let root = root.as_ref().to_path_buf();
    let db   = db.as_ref().to_path_buf();

    let (result_tx, result_rx) = oneshot::channel();

    thread::spawn(move || {
        trace!("Indexer thread started");
        let outcome = run_indexer(root, db, thread_stop_signal, status);
        trace!("Indexer thread finished with outcome: {:?}", outcome);
        let _ = result_tx.send(outcome);
    });

    IndexerHandle {stop_signal, result_rx }
} 

