//! The data pipeline that builds games.db (ROADMAP Phase 4). `main.rs` is the
//! command line; the logic lives here so tests can reach it.

pub mod applist;
pub mod crawl;
pub mod ingest;
pub mod protondb;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
