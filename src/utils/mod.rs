use std::path::{Path, PathBuf};

pub mod extensions;
pub mod error;


pub fn get_sharded_path<P: AsRef<Path>>(base_dir: P, id: u64, ext: &str) -> PathBuf {
    let shard_value = (id & 0xFF) as u8; // Get the last two hex digits. 
    let shard_str = format!("{:02x}", shard_value);

    let filename = format!("{:08x}{}", id, ext);

    
    let mut path =PathBuf::new();

    path.push(base_dir);
    path.push(shard_str);
    path.push(filename);

    path   
}