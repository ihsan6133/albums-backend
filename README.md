# Albums-backend
A high-performance, Rust-powered media engine designed to asynchronously handle massive image libraries with sub-millisecond response times. This backend serves as the brain for the Albums web application, prioritizing database efficiency and lean API payloads.

## Tools
Language: Rust

Web Framework: Axum

Database: SQLite via rusqlite

## Performance Features
Transactional Bulk Writes: Adding 1,000+ images to an album happens in a single ACID transaction, reducing disk I/O overhead from seconds to milliseconds.
DB-Level Integrity: Uses SQLite Triggers to maintain media_count and date ranges within albums, ensuring the application layer remains "dumb" and fast.

## Project Structure
```
src/
├── main.rs          # Entry point, Router setup, and DB initialization
├── models.rs        # Shared Data Structures & Row Mapping (Traits)
├── services/        # Business Logic & SQL Query Builders
│   ├── mod.rs
│   ├── media.rs     # Core layout logic and tree reconstruction
│   └── album.rs     # Album management and bulk operations
└── routes/          # Axum Handlers (Request/Response boundary)
    ├── mod.rs
    ├── media.rs     # /api/media endpoints
    └── albums.rs    # /api/albums endpoints
```
## Database Schema
The core logic relies on three primary tables:

media: Stores file paths, dimensions, and extracted metadata.

albums: Stores user-created collections with denormalized metadata for fast browsing.

album_media: A junction table facilitating the many-to-many relationship between media and albums.

Essential Indexes
To ensure O(logN) performance at scale:

idx_media_timestamp: Sorting the main library timeline.

idx_album_media_lookup: Fast retrieval of photos within a specific album.

idx_media_path: Optimizing the folder-tree prefix queries.

## API Overview
### Media
`GET /api/media` - Returns the full library in Columnar format.

`GET /api/folders` - Returns a recursive JSON tree of the physical file structure.

### Albums
`GET /api/albums` - List all albums with media_count and cover IDs.

`POST /api/albums` - Create a new album.

`GET /api/albums/:id/media` - Fetch layout data for a specific album.

`POST /api/albums/:id/media` - Bulk add media IDs to an album (JSON body).

## Development
Setup
Ensure you have the latest stable Rust toolchain installed.

Clone the repo.

Configure your media library path in the environment or config.

Running
```bash
cargo run
```

Building for Release
```bash
cargo build --release
```
