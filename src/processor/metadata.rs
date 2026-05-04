use std::{fs::{self, File}, io::{BufReader, Seek}, path::Path, str::from_utf8};

use anyhow::anyhow;
use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc, offset::LocalResult};
use exif::{Exif, Field, In, Tag, Value};
use imagesize::{ImageSize, ImageType};
use crate::{database::models::{MediaTimestamp, Metadata}, utils::extensions::PHOTO_EXTENSIONS};

fn parse_offset(s: &str) -> Option<FixedOffset>{
    if s.len() != 6 {return None};

    let ascii = s.as_bytes();

    let sign = match  ascii[0] as char {
        '+' => 1,
        '-' => -1,
        _   => {return None} 
    };
    if ascii[3] as char != ':' {return None;}


    let hh = from_utf8(&ascii[1..3]).ok()?.parse::<i32>().ok()?;
    let mm = from_utf8(&ascii[4..6]).ok()?.parse::<i32>().ok()?;
    
    FixedOffset::east_opt(sign * (hh * 3600 + mm * 60))
}

fn get_exif_timestamp(exif: &Exif) -> Option<MediaTimestamp> {

    let timestamp = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY)
        .or_else(|| exif.get_field(Tag::DateTimeDigitized, In::PRIMARY));

    let Some(Field  {value: Value::Ascii(v), ..}) = timestamp else {return None};
    let Some(str) = v.get(0) else { return None };

    let str = from_utf8(str).ok()?;

    let timestamp = NaiveDateTime::parse_from_str(str, "%Y:%m:%d %H:%M:%S").ok()?;

    let timezone = exif.get_field(Tag::OffsetTimeOriginal, In::PRIMARY)
        .or_else(|| exif.get_field(Tag::OffsetTimeDigitized, In::PRIMARY));

    if let Some(Field { value: Value::Ascii(v), .. }) =  timezone 
    && let Some(Ok(str)) = v.get(0).and_then(|v| Some(from_utf8(v)))
    && let Some(timezone) = parse_offset(str)
    {
        let LocalResult::Single(t) = timezone.from_local_datetime(&timestamp) else {
            panic!("Error with timestamp");
        };

        return Some(MediaTimestamp::WithOffset(t));
        
    }

    return Some(MediaTimestamp::Naive(timestamp));

}

fn is_exif_sideways(exif: &Exif) -> bool {
    let Some(Field {  value: Value::Short(v), .. }) = exif.get_field(Tag::Orientation, In::PRIMARY) else { return false };
    let Some(v) = v.get(0) else {return false};

    return *v >= 5 && *v <= 8;
}

pub fn get_media_metadata(path: &Path, ext: &str, file_meta: &fs::Metadata) -> Result<Metadata, anyhow::Error>{
    
    if !PHOTO_EXTENSIONS.contains(&ext) {return Err(anyhow!("Unsupported file type {}", ext))};

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let img_type = imagesize::reader_type(&mut reader)?;

    let ImageSize { width, height} = img_type.reader_size(&mut reader)?;
    let mut width= width.try_into()?;
    let mut height = height.try_into()?;
    
    reader.rewind()?;

    match ext {
        "jpg" | "jpeg" | "tiff" | "heif" | "heic" | "png" | "webp" => {

            let exifreader = exif::Reader::new();
            if let Ok(exif) = exifreader.read_from_container(&mut reader) {

                // Imagesize already accounts for exif orientation in Heif files because the orientation
                // is also stored in the header
                if !matches!(img_type, ImageType::Heif(_)) && is_exif_sideways(&exif) {
                    std::mem::swap(&mut width, &mut height);
                }

                if let Some(timestamp) = get_exif_timestamp(&exif) {
                    return Ok(Metadata { width, height, taken_at: timestamp, duration: 0.0 });
                }
                
            }

            let mtime = file_meta.modified()?;
            let timestamp: DateTime<Utc> = mtime.into();
            return Ok(Metadata {width, height, taken_at: MediaTimestamp::Utc(timestamp), duration: 0.0});

        },

        _ => Err(anyhow!("Unsupported media extension '.{}'", ext))
    }
}