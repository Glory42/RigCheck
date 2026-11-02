//! The data pipeline that builds games.db (ROADMAP Phase 4). `main.rs` is the
//! command line; the logic lives here so tests can reach it.

pub mod applist;
pub mod awacy;
pub mod blender;
pub mod build;
pub mod crawl;
pub mod deck;
pub mod pci;
pub mod protondb;
pub mod publish;
pub mod report;
pub mod validate;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
