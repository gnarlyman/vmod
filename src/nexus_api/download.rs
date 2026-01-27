//! Download manager for Nexus Mods files.

use reqwest::blocking::Client;
use reqwest::header::{CONTENT_LENGTH, RANGE};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::config::ensure_downloads_dir;
use super::types::DownloadLink;

const BUFFER_SIZE: usize = 8192;
const APP_NAME: &str = "vmod";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Download state
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadState {
    /// Not started
    Pending,
    /// Currently downloading
    Downloading,
    /// Download paused
    Paused,
    /// Download completed successfully
    Completed,
    /// Download failed
    Failed(String),
    /// Download was cancelled
    Cancelled,
}

/// Progress information for a download
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Current state
    pub state: DownloadState,
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Total size in bytes (if known)
    pub total_bytes: Option<u64>,
    /// Download speed in bytes/second
    pub speed: f64,
    /// File name being downloaded
    pub file_name: String,
}

impl DownloadProgress {
    fn new(file_name: String) -> Self {
        Self {
            state: DownloadState::Pending,
            bytes_downloaded: 0,
            total_bytes: None,
            speed: 0.0,
            file_name,
        }
    }

    /// Get progress as a fraction (0.0 to 1.0)
    pub fn fraction(&self) -> f64 {
        match self.total_bytes {
            Some(total) if total > 0 => self.bytes_downloaded as f64 / total as f64,
            _ => 0.0,
        }
    }

    /// Format speed as human-readable string
    pub fn speed_string(&self) -> String {
        if self.speed < 1024.0 {
            format!("{:.0} B/s", self.speed)
        } else if self.speed < 1024.0 * 1024.0 {
            format!("{:.1} KB/s", self.speed / 1024.0)
        } else {
            format!("{:.1} MB/s", self.speed / (1024.0 * 1024.0))
        }
    }

    /// Format progress as human-readable string
    pub fn progress_string(&self) -> String {
        let downloaded = format_bytes(self.bytes_downloaded);
        match self.total_bytes {
            Some(total) => format!("{} / {}", downloaded, format_bytes(total)),
            None => downloaded,
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Metadata for a downloaded file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadMetadata {
    /// Original file name
    pub file_name: String,
    /// Mod ID on Nexus
    pub mod_id: u64,
    /// File ID on Nexus
    pub file_id: u64,
    /// Game domain
    pub game: String,
    /// Mod name (if known)
    pub mod_name: Option<String>,
    /// File version
    pub version: Option<String>,
    /// Download URL used
    pub source_url: String,
    /// File size in bytes
    pub size: u64,
    /// Download timestamp
    pub downloaded_at: i64,
}

/// Download manager handles file downloads with progress tracking
pub struct DownloadManager {
    client: Client,
    progress: Arc<Mutex<DownloadProgress>>,
    cancelled: Arc<Mutex<bool>>,
}

impl DownloadManager {
    /// Create a new download manager
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .user_agent(format!("{}/{}", APP_NAME, APP_VERSION))
            .timeout(None) // No timeout for downloads
            .build()?;

        Ok(Self {
            client,
            progress: Arc::new(Mutex::new(DownloadProgress::new(String::new()))),
            cancelled: Arc::new(Mutex::new(false)),
        })
    }

    /// Get the shared progress state
    pub fn progress(&self) -> Arc<Mutex<DownloadProgress>> {
        self.progress.clone()
    }

    /// Get the cancellation flag
    pub fn cancel_flag(&self) -> Arc<Mutex<bool>> {
        self.cancelled.clone()
    }

    /// Request cancellation of the current download
    pub fn cancel(&self) {
        *self.cancelled.lock().unwrap() = true;
    }

    /// Download a file from one of the provided links
    ///
    /// Tries each link in order until one succeeds.
    /// Returns the path to the downloaded file.
    pub fn download(
        &self,
        links: &[DownloadLink],
        file_name: &str,
        metadata: DownloadMetadata,
    ) -> Result<PathBuf, DownloadError> {
        if links.is_empty() {
            return Err(DownloadError::NoLinks);
        }

        // Reset state
        *self.cancelled.lock().unwrap() = false;
        {
            let mut progress = self.progress.lock().unwrap();
            *progress = DownloadProgress::new(file_name.to_string());
            progress.state = DownloadState::Downloading;
        }

        // Get download directory
        let download_dir = ensure_downloads_dir()
            .map_err(|e| DownloadError::Io(e.to_string()))?;

        // Generate unique file path
        let file_path = download_dir.join(file_name);
        let metadata_path = download_dir.join(format!("{}.meta.json", file_name));

        // Try each download link
        let mut last_error = None;
        for link in links {
            log::info!("Trying download from {} ({})", link.name, link.short_name);

            match self.download_from_url(&link.uri, &file_path) {
                Ok(()) => {
                    // Save metadata
                    let meta_json = serde_json::to_string_pretty(&metadata)
                        .map_err(|e| DownloadError::Io(e.to_string()))?;
                    fs::write(&metadata_path, meta_json)
                        .map_err(|e| DownloadError::Io(e.to_string()))?;

                    // Update state
                    {
                        let mut progress = self.progress.lock().unwrap();
                        progress.state = DownloadState::Completed;
                    }

                    log::info!("Download completed: {:?}", file_path);
                    return Ok(file_path);
                }
                Err(e) => {
                    log::warn!("Download from {} failed: {}", link.name, e);
                    last_error = Some(e);

                    // Check for cancellation
                    if *self.cancelled.lock().unwrap() {
                        let mut progress = self.progress.lock().unwrap();
                        progress.state = DownloadState::Cancelled;
                        return Err(DownloadError::Cancelled);
                    }
                }
            }
        }

        // All links failed
        {
            let mut progress = self.progress.lock().unwrap();
            progress.state = DownloadState::Failed(
                last_error.as_ref().map(|e| e.to_string()).unwrap_or_default()
            );
        }

        Err(last_error.unwrap_or(DownloadError::AllLinksFailed))
    }

    /// Download from a specific URL
    fn download_from_url(&self, url: &str, file_path: &PathBuf) -> Result<(), DownloadError> {
        log::debug!("Starting download from: {}", url);

        // Check for existing partial download
        let existing_size = file_path.metadata().map(|m| m.len()).unwrap_or(0);

        // Build request with range header if resuming
        let mut request = self.client.get(url);
        if existing_size > 0 {
            log::info!("Resuming download from byte {}", existing_size);
            request = request.header(RANGE, format!("bytes={}-", existing_size));
        }

        let response = request.send()
            .map_err(|e| DownloadError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(DownloadError::Http(status.as_u16(), status.to_string()));
        }

        // Get total size
        let total_size = response.headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(|size| {
                if status.as_u16() == 206 {
                    size + existing_size
                } else {
                    size
                }
            });

        // Update progress with total size
        {
            let mut progress = self.progress.lock().unwrap();
            progress.total_bytes = total_size;
            progress.bytes_downloaded = existing_size;
        }

        // Open file for writing
        let mut file = if existing_size > 0 && status.as_u16() == 206 {
            fs::OpenOptions::new()
                .append(true)
                .open(file_path)
                .map_err(|e| DownloadError::Io(e.to_string()))?
        } else {
            File::create(file_path)
                .map_err(|e| DownloadError::Io(e.to_string()))?
        };

        // Read and write in chunks
        let mut reader = response;
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut bytes_downloaded = existing_size;
        let mut last_speed_update = std::time::Instant::now();
        let mut bytes_since_last_update = 0u64;

        loop {
            // Check for cancellation
            if *self.cancelled.lock().unwrap() {
                return Err(DownloadError::Cancelled);
            }

            // Read chunk
            let bytes_read = reader.read(&mut buffer)
                .map_err(|e| DownloadError::Io(e.to_string()))?;

            if bytes_read == 0 {
                break;
            }

            // Write chunk
            file.write_all(&buffer[..bytes_read])
                .map_err(|e| DownloadError::Io(e.to_string()))?;

            bytes_downloaded += bytes_read as u64;
            bytes_since_last_update += bytes_read as u64;

            // Update progress
            let elapsed = last_speed_update.elapsed();
            if elapsed.as_millis() >= 100 {
                let speed = bytes_since_last_update as f64 / elapsed.as_secs_f64();

                let mut progress = self.progress.lock().unwrap();
                progress.bytes_downloaded = bytes_downloaded;
                progress.speed = speed;

                bytes_since_last_update = 0;
                last_speed_update = std::time::Instant::now();
            }
        }

        // Final progress update
        {
            let mut progress = self.progress.lock().unwrap();
            progress.bytes_downloaded = bytes_downloaded;
            progress.speed = 0.0;
        }

        log::debug!("Downloaded {} bytes", bytes_downloaded);
        Ok(())
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new().expect("Failed to create download manager")
    }
}

/// Download error types
#[derive(Debug)]
pub enum DownloadError {
    /// No download links provided
    NoLinks,
    /// All download links failed
    AllLinksFailed,
    /// HTTP request failed
    Request(String),
    /// HTTP error status
    Http(u16, String),
    /// I/O error
    Io(String),
    /// Download was cancelled
    Cancelled,
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::NoLinks => write!(f, "No download links available"),
            DownloadError::AllLinksFailed => write!(f, "All download links failed"),
            DownloadError::Request(e) => write!(f, "Request failed: {}", e),
            DownloadError::Http(code, msg) => write!(f, "HTTP {}: {}", code, msg),
            DownloadError::Io(e) => write!(f, "I/O error: {}", e),
            DownloadError::Cancelled => write!(f, "Download cancelled"),
        }
    }
}

impl std::error::Error for DownloadError {}
