//! DownloadItem GObject for displaying downloaded files in the Downloads list.

use glib::Object;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;

mod imp {
    use super::*;
    use glib::Properties;
    use std::cell::Cell;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::DownloadItem)]
    pub struct DownloadItem {
        /// File name on disk (e.g., "Bestiary Linux-222-2-2-1-1708550222.zip")
        #[property(get, set)]
        pub file_name: RefCell<String>,
        /// Display name for UI (extracted from metadata or source URL)
        #[property(get, set)]
        pub display_name: RefCell<String>,
        /// Mod ID on Nexus
        #[property(get, set)]
        pub mod_id: Cell<u64>,
        /// File ID on Nexus
        #[property(get, set)]
        pub file_id: Cell<u64>,
        /// File size in bytes
        #[property(get, set)]
        pub size: Cell<u64>,
        /// Download timestamp (Unix timestamp)
        #[property(get, set)]
        pub downloaded_at: Cell<i64>,
        /// Full path to the zip file
        #[property(get, set)]
        pub full_path: RefCell<String>,
        /// Game domain (e.g., "daggerfallunity")
        #[property(get, set)]
        pub game: RefCell<String>,
        /// Version string if available
        #[property(get, set)]
        pub version: RefCell<String>,
        /// Mod name if known
        #[property(get, set)]
        pub mod_name: RefCell<String>,
        /// Source URL for the download
        #[property(get, set)]
        pub source_url: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DownloadItem {
        const NAME: &'static str = "DownloadItem";
        type Type = super::DownloadItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for DownloadItem {}
}

glib::wrapper! {
    pub struct DownloadItem(ObjectSubclass<imp::DownloadItem>);
}

impl DownloadItem {
    /// Create a new download item
    pub fn new(
        file_name: &str,
        display_name: &str,
        mod_id: u64,
        file_id: u64,
        size: u64,
        downloaded_at: i64,
        full_path: &str,
        game: &str,
        version: &str,
        mod_name: &str,
        source_url: &str,
    ) -> Self {
        Object::builder()
            .property("file-name", file_name)
            .property("display-name", display_name)
            .property("mod-id", mod_id)
            .property("file-id", file_id)
            .property("size", size)
            .property("downloaded-at", downloaded_at)
            .property("full-path", full_path)
            .property("game", game)
            .property("version", version)
            .property("mod-name", mod_name)
            .property("source-url", source_url)
            .build()
    }

    /// Get the full path as a PathBuf
    pub fn path(&self) -> PathBuf {
        PathBuf::from(self.full_path())
    }

    /// Format the file size as a human-readable string
    pub fn size_string(&self) -> String {
        let bytes = self.size();
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

    /// Format the download date as a human-readable string
    pub fn date_string(&self) -> String {
        use chrono::{Local, TimeZone};
        let timestamp = self.downloaded_at();
        if let Some(dt) = Local.timestamp_opt(timestamp, 0).single() {
            dt.format("%Y-%m-%d %H:%M").to_string()
        } else {
            "Unknown".to_string()
        }
    }
}
