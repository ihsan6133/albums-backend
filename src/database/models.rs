use std::{fmt::Display, str::FromStr};

use anyhow::bail;
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use rusqlite::{Row, types::FromSqlError};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub enum MediaType {
    PHOTO,
    VIDEO
}

#[derive(Debug)]
pub enum MediaTimestamp {
    Naive(NaiveDateTime),
    WithOffset(DateTime<FixedOffset>),
    Utc(DateTime<Utc>)
}



impl Serialize for MediaTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        
        serializer.serialize_str(&self.to_string())
    }
}

impl Display for MediaTimestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaTimestamp::Naive(dt) => {
                write!(f, "{}", dt.format("%Y-%m-%dT%H:%M:%S%.3f"))
            },

            MediaTimestamp::Utc(dt) => {
                write!(f, "{}", dt.format("%Y-%m-%dT%H:%M:%S%.3fZ"))
            },

            MediaTimestamp::WithOffset(dt) => {
                write!(f, "{}", dt.format("%Y-%m-%dT%H:%M:%S%.3f%:z"))
            }
        }
    }
}


impl MediaTimestamp {
    
    pub fn to_normalized_utc_string(&self) -> String {
        let norm = match self {
            MediaTimestamp::Naive(dt) => DateTime::from_naive_utc_and_offset(*dt, Utc),
            MediaTimestamp::Utc(dt) => *dt,
            MediaTimestamp::WithOffset(dt) => dt.with_timezone(&Utc)
        };

        format!("{}", norm.format("%Y-%m-%dT%H:%M:%S%.3fZ"))
    }
}

impl FromStr for MediaTimestamp {
    type Err = anyhow::Error;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with('Z') {
            return Ok(MediaTimestamp::Utc(s.parse()?))
        }

        if let Ok(dt) =  DateTime::parse_from_rfc3339(s) {
            return Ok(MediaTimestamp::WithOffset(dt))
        } 

        let fmts = ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M:%S%.f"];
        
        for fmt in fmts {
            if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
                return Ok(MediaTimestamp::Naive(dt))
            }
        }

        bail!("Invalid MediaTimestamp '{s}'");
    }
}

#[derive(Debug)]
pub struct LibraryStats {
    pub total_items: u64,
    pub total_size : u64,
    pub photo_count: u64,
    pub video_count: u64,
}

#[derive(Debug, Serialize)]
pub struct Metadata {
    pub width: i32,
    pub height: i32,

    pub duration: f32,
    pub taken_at: MediaTimestamp,
}


#[derive(Debug, Serialize)]
pub struct MediaItem {
    pub id: i64,
    pub path: String,
    pub size: u64,

    pub mtime: i64,
    pub indexed_at: i64,
    pub timestamp: i64,

    pub media_type: MediaType,

    pub metadata: Metadata,
    
    pub missing: bool,
}


impl MediaItem {
    pub fn from_row(row: &Row<'_>) -> Result<Self, rusqlite::Error> {
        Ok(MediaItem {
            id: row.get(0)?,
            path: row.get(1)?,
            size: row.get::<_, i64>(2)? as u64,
            mtime: row.get(3)?,
            timestamp: row.get(4)?,
            indexed_at: row.get(5)?,
            missing: row.get(10)?,
            media_type: MediaType::PHOTO,
            
            metadata: Metadata { 
                taken_at: row.get::<_, String>(6)?.parse().map_err(|_| FromSqlError::InvalidType)?, 
                width: row.get(7)?, 
                height: row.get(8)?, 
                duration: row.get(9)?, 
            }

        })
    }
}


pub trait Sizeable {
    fn total_allocated(&self) -> usize;
}

impl Sizeable for Vec<MediaItem> {
    fn total_allocated(&self,) -> usize {
        let mut size = std::mem::size_of::<Vec<MediaItem>>();
        
        size += std::mem::size_of::<MediaItem>() * self.capacity();
        
        for media in self {
            size += media.path.capacity();
        }

        return size;
    }
}


#[derive(Serialize)]
pub struct MediaLayout {
    id: i64,
    width: i32,
    height: i32,
    timestamp: i64,
}

impl MediaLayout {

    // IF QUERY IS 'SELECT id, width, height, timestamp FROM media'
    pub fn from_row(row: &Row<'_>) -> Result<Self, rusqlite::Error> {
        Ok(Self { 
            id: row.get(0)?,
            width: row.get(1)?, 
            height: row.get(2)?, 
            timestamp: row.get(3)?,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub latest_photo_at: i64,
    pub media_count: i64,
}


