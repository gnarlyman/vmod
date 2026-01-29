//! Nexus Mods API integration module.
//!
//! Provides functionality for:
//! - API authentication
//! - Mod metadata retrieval
//! - Download link generation
//! - File downloading with progress tracking

mod client;
mod config;
mod types;

pub mod download;

pub use client::NexusClient;
pub use config::{downloads_dir, NexusConfig};
pub use download::{check_existing_file, delete_existing_file, DownloadManager, DownloadMetadata, DownloadProgress, DownloadState};
pub use types::{DownloadLink, ModFile};
