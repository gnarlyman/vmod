//! Nexus Mods API integration module.
//!
//! Provides functionality for:
//! - API authentication (SSO WebSocket flow)
//! - Mod metadata retrieval
//! - Download link generation
//! - File downloading with progress tracking

mod client;
mod config;
mod types;

pub mod download;
pub mod sso;

pub use client::NexusClient;
pub use config::NexusConfig;
pub use download::{DownloadManager, DownloadMetadata, DownloadProgress, DownloadState};
pub use sso::SsoAuth;
pub use types::{DownloadLink, ModFile, ModInfo, UserInfo};
