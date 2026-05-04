// use sqlite3::{Connection, Error, Readable, State, Statement, Type};

// use crate::{database::models::{MediaItem, MediaType}};

// fn optional_read<T: Readable>(stmt: &Statement, i: usize) -> sqlite3::Result<Option<T>> {
//     if stmt.kind(i) == Type::Null {
//         Ok(None)
//     } else {
//         Ok(Some(stmt.read(i)?))
//     }
// }

// fn read_media_type(stmt: &Statement, i: usize) -> sqlite3::Result<MediaType> {
//     let type_str = stmt.read::<String>(i)?;

//     match type_str.as_str() {
//         "photo" => Ok(MediaType::PHOTO),
//         "video" => Ok(MediaType::VIDEO),

//         _ => Err(Error {code: None, message: Some("Invalid value for media_type.".to_string()) })
//     }
// }

// fn read_bool(stmt: &Statement, i: usize) -> sqlite3::Result<bool> {
//     let b_int: i64 = stmt.read(i)?;

//     match b_int {
//         0 => Ok(false),
//         1 => Ok(true),

//         _ => Err(Error {code: None, message: Some("Boolean value not 0 or 1.".to_string()) })
//     }
// }

// fn read_media_item(stmt: &Statement) -> sqlite3::Result<MediaItem> {
//     Ok(MediaItem {
//         id         : stmt.read(0)?,
//         path       : stmt.read(1)?,
//         size       : stmt.read::<i64>(2)? as u64,
//         mtime      : stmt.read(3)?,
//         file_hash  : optional_read(&stmt, 4)?,
//         taken_at   : optional_read(&stmt, 5)?,
//         indexed_at : stmt.read(6)?,
//         media_type : read_media_type(&stmt, 7)?,
//         width      : optional_read::<i64>(&stmt, 8)?.map(|v| v as u16),
//         height     : optional_read::<i64>(&stmt, 9)?.map(|v| v as u16),
//         duration   : optional_read::<f64>(&stmt, 10)?,
//         missing    : read_bool(&stmt, 11)?
//     })
// }


// pub fn get_all_media(conn: &Connection) -> sqlite3::Result<Vec<MediaItem>> {
//     let mut stmt = conn.prepare("SELECT * FROM media;")?;

//     let mut results = Vec::new();

//     while let State::Row = stmt.next()?  {
//         results.push(read_media_item(&stmt)?);
//     }
//     Ok(results)
// }

use std::{fs::create_dir_all, path::Path};

use rusqlite::{Connection, params, types::FromSqlError};
use serde::Serialize;

use crate::database::models::{Album, MediaItem, MediaLayout};

pub fn init_media_db<P: AsRef<Path>>(path: P) -> anyhow::Result<Connection> {
    
    if let Some(parent) = path.as_ref().parent() {
        create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;

    conn.execute_batch(
    "
    CREATE TABLE IF NOT EXISTS media (
        id INTEGER PRIMARY KEY,
        
        path TEXT UNIQUE NOT NULL,
        size INTEGER NOT NULL,
        mtime INTEGER NOT NULL,

        timestamp INTEGER NOT NULL,
        indexed_at INTEGER NOT NULL,
        
        taken_at TEXT,
        width  INTEGER NOT NULL,
        height INTEGER NOT NULL,
        duration REAL,

        missing INTEGER NOT NULL DEFAULT 0
    );

    CREATE UNIQUE INDEX IF NOT EXISTS idx_media_path ON media (path);
    CREATE INDEX IF NOT EXISTS idx_media_sync_check ON media (path, mtime, size);
    CREATE INDEX IF NOT EXISTS idx_media_date ON media (timestamp DESC);
    
    CREATE TABLE IF NOT EXISTS albums (
        id INTEGER PRIMARY KEY,
        title TEXT NOT NULL,
        created_at TEXT NOT NULL,
        latest_photo_at INTEGER DEFAULT 0,
        media_count INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS album_media (
        album_id INTEGER NOT NULL,
        media_id INTEGER NOT NULL,
        added_at TEXT NOT NULL,   -- When it was added to the album,

        PRIMARY KEY (album_id, media_id), -- Prevents adding the same photo twice
        FOREIGN KEY(album_id) REFERENCES albums(id) ON DELETE CASCADE,
        FOREIGN KEY(media_id) REFERENCES media(id) ON DELETE CASCADE
    );

    -- For loading the list of albums quickly on the home screen
    CREATE INDEX IF NOT EXISTS idx_albums_created ON albums(created_at DESC);

    -- For loading all photos inside a specific album (Sorted by when they were added)
    CREATE INDEX IF NOT EXISTS idx_album_media_lookup ON album_media(album_id, added_at DESC);

    -- For finding all albums a specific photo belongs to (e.g., showing tags on the info panel)
    CREATE INDEX IF NOT EXISTS idx_media_to_albums ON album_media(media_id);
    CREATE INDEX IF NOT EXISTS idx_albums_timeline ON albums(latest_photo_at DESC);

    -- 2. Create a trigger for ADDING media
    CREATE TRIGGER IF NOT EXISTS increment_media_count
    AFTER INSERT ON album_media
    BEGIN
        UPDATE albums 
        SET media_count = media_count + 1 
        WHERE id = NEW.album_id;
    END;

    -- 3. Create a trigger for REMOVING media
    CREATE TRIGGER IF NOT EXISTS decrement_media_count
    AFTER DELETE ON album_media
    BEGIN
        UPDATE albums 
        SET media_count = media_count - 1 
        WHERE id = OLD.album_id;
    END;

    ")?;

    Ok(conn)
} 

pub fn get_media_path(conn: &Connection, id: i32) -> Result<String, rusqlite::Error> {
    let mut stmt = conn.prepare_cached("SELECT path FROM media WHERE id = ?1;").unwrap();

    stmt.query_one(params![id], |r| r.get::<_, String>(0))
}

pub fn get_albums(conn: &Connection) -> Result<Vec<Album>, rusqlite::Error> {

    let mut stmt = conn.prepare_cached("SELECT id, title, created_at, media_count, latest_photo_at FROM albums;")?;
    

    let rows = stmt.query_map([], |r| {
        Ok(Album{
            id: r.get(0)?,
            title: r.get(1)?,
            created_at: r.get::<_, String>(2)?.parse().map_err(|_| FromSqlError::InvalidType)?,
            media_count: r.get(3)?,
            latest_photo_at: r.get(4)?
        })
    })?;

    let mut albums = Vec::new();
    for album in rows {
        albums.push(album?);
    } 

    Ok(albums)
}

pub fn get_album(conn: &Connection, id: i64) -> Result<Album, rusqlite::Error> {
    let mut stmt = conn.prepare_cached("SELECT id, title, created_at, media_count, latest_photo_at FROM albums WHERE id = ?1;")?;
    
    let album = stmt.query_one(params![id], |r| {
        Ok(Album {
            id: r.get(0)?,
            title: r.get(1)?,
            created_at: r.get::<_, String>(2)?.parse().map_err(|_| FromSqlError::InvalidType)?,
            media_count: r.get(3)?,
            latest_photo_at: r.get(4)?
        })
    })?;

    Ok(album)
}


pub enum SortOrder {
    Asc,
    Desc
}
pub struct SortOptions {
    pub sort_by: String,
    pub sort_order: SortOrder,

}

#[derive(Serialize)]
#[serde(untagged)] // This hides the enum "wrapper" in the JSON

pub enum MediaData {
    Full(Vec<MediaItem>),
    LayoutOnly(Vec<MediaLayout>)
}

const FIELDS_FULL  : &str = "m.id, m.path, m.size, m.mtime, m.timestamp, m.indexed_at, m.taken_at, m.width, m.height, m.duration, m.missing";
const FIELDS_LAYOUT: &str = "m.id, m.width, m.height, m.timestamp";
const FIELDS: &[&str] = &["id", "path", "size", "mtime", "timestamp", "indexed_at", "taken_at", "width", "height", "duration", "missing"];

fn get_order_by(sort_options: &Option<SortOptions>) -> Result<String, anyhow::Error> {
    let Some(opt) = sort_options else {
        return Ok(String::new());
    };

    if !FIELDS.iter().any(|f| f.eq_ignore_ascii_case(&opt.sort_by)) {
        anyhow::bail!("Invalid sort field {}", opt.sort_by);
    }

    let order = match opt.sort_order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC"
    };


    Ok(format!(" ORDER BY m.{} {}", opt.sort_by, order))

}

pub fn get_media(conn: &Connection, limit: usize, offset: usize, album_id: Option<i64>, sort_options: Option<SortOptions>, layout_only: bool) -> Result<MediaData, anyhow::Error> {

    let join_stmt = match album_id {
        Some(_) => " INNER JOIN album_media am ON m.id = am.media_id WHERE am.album_id = ?3",
        None => ""
    };
    let fields = if layout_only { FIELDS_LAYOUT } else { FIELDS_FULL };
    let order_by = get_order_by(&sort_options)?;

    let query = format!("SELECT {fields} FROM media m{join_stmt}{order_by} LIMIT ?1 OFFSET ?2;");

    let mut stmt = conn.prepare_cached(&query)?;

    let params = match album_id {
        Some(album_id) => params![limit as i64, offset as i64, album_id as i64],
        None => params![limit as i64, offset as i64],
    };

    if layout_only {
        let mut media_layouts = Vec::new();
        for media in stmt.query_map(params, |r| MediaLayout::from_row(r))? {
            media_layouts.push(media?);
        }

        Ok(MediaData::LayoutOnly(media_layouts))
    } else {
        let mut media_items = Vec::new();
        for media in stmt.query_map(params, |r| MediaItem::from_row(r))? {
            media_items.push(media?);
        }

        Ok(MediaData::Full(media_items))

    }


}