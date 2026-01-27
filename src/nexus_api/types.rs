//! API response types for Nexus Mods API v1.

use serde::{Deserialize, Serialize};

/// User information returned from /v1/users/validate.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserInfo {
    /// User ID on Nexus Mods
    pub user_id: u64,
    /// API key used for this request
    pub key: String,
    /// User's display name
    pub name: String,
    /// Whether user has premium membership
    pub is_premium: bool,
    /// Whether user is a supporter
    pub is_supporter: bool,
    /// User's email address
    pub email: String,
    /// URL to user's profile image
    #[serde(default)]
    pub profile_url: String,
}

/// Mod information returned from /v1/games/{game}/mods/{mod_id}.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModInfo {
    /// Mod ID
    pub mod_id: u64,
    /// Game ID this mod belongs to
    pub game_id: u64,
    /// Domain name of the game (e.g., "daggerfallunity")
    pub domain_name: String,
    /// Mod name
    pub name: String,
    /// Short summary/description
    #[serde(default)]
    pub summary: String,
    /// Mod version string
    #[serde(default)]
    pub version: String,
    /// Author name
    #[serde(default)]
    pub author: String,
    /// Uploader username
    #[serde(default)]
    pub uploaded_by: String,
    /// URL to mod page picture
    #[serde(default)]
    pub picture_url: Option<String>,
    /// Whether mod contains adult content
    #[serde(default)]
    pub contains_adult_content: bool,
    /// Mod status (e.g., "published")
    #[serde(default)]
    pub status: String,
    /// Whether mod is available for download
    #[serde(default)]
    pub available: bool,
}

/// File information returned from /v1/games/{game}/mods/{mod_id}/files.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModFile {
    /// Unique file ID
    #[serde(alias = "file_id")]
    pub id: Vec<u64>,
    /// File name
    pub name: String,
    /// File version
    pub version: String,
    /// Category ID (1=main, 2=update, 3=optional, etc.)
    pub category_id: u32,
    /// Human-readable category name
    #[serde(default)]
    pub category_name: Option<String>,
    /// Whether this is the primary/main file
    #[serde(default)]
    pub is_primary: bool,
    /// File size in kilobytes
    #[serde(default)]
    pub size_kb: u64,
    /// File size in bytes (more accurate)
    #[serde(default)]
    pub size: Option<u64>,
    /// Unix timestamp of upload
    #[serde(default)]
    pub uploaded_timestamp: u64,
    /// External virus scan URL
    #[serde(default)]
    pub external_virus_scan_url: Option<String>,
    /// Description/changelog for this file
    #[serde(default)]
    pub description: Option<String>,
    /// MD5 hash of the file
    #[serde(default)]
    pub md5: Option<String>,
}

/// Files response wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct ModFilesResponse {
    pub files: Vec<ModFileEntry>,
}

/// Individual file entry in the files response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModFileEntry {
    /// Unique file ID
    #[serde(alias = "file_id")]
    pub id: u64,
    /// File name
    pub name: String,
    /// File version
    pub version: String,
    /// Category ID (1=main, 2=update, 3=optional, etc.)
    pub category_id: u32,
    /// Human-readable category name
    #[serde(default)]
    pub category_name: Option<String>,
    /// Whether this is the primary/main file
    #[serde(default)]
    pub is_primary: bool,
    /// File size in kilobytes
    #[serde(default)]
    pub size_kb: u64,
    /// File size in bytes (more accurate)
    #[serde(default)]
    pub size: Option<u64>,
    /// Unix timestamp of upload
    #[serde(default)]
    pub uploaded_timestamp: u64,
    /// External virus scan URL
    #[serde(default)]
    pub external_virus_scan_url: Option<String>,
    /// Description/changelog for this file
    #[serde(default)]
    pub description: Option<String>,
    /// File name for download
    #[serde(default)]
    pub file_name: Option<String>,
}

/// Download link returned from /v1/games/{game}/mods/{mod_id}/files/{file_id}/download_link.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DownloadLink {
    /// Server/CDN name (e.g., "Nexus CDN")
    pub name: String,
    /// Short server name (e.g., "Paris")
    pub short_name: String,
    /// Direct download URL
    #[serde(rename = "URI")]
    pub uri: String,
}

/// Rate limit information from response headers
#[derive(Debug, Clone, Default)]
pub struct RateLimitInfo {
    /// Requests remaining in current hour
    pub hourly_remaining: Option<u32>,
    /// Total hourly limit
    pub hourly_limit: Option<u32>,
    /// Requests remaining today
    pub daily_remaining: Option<u32>,
    /// Total daily limit
    pub daily_limit: Option<u32>,
}

impl RateLimitInfo {
    /// Parse rate limit info from response headers
    pub fn from_headers(headers: &reqwest::header::HeaderMap) -> Self {
        let get_header = |name: &str| -> Option<u32> {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
        };

        Self {
            hourly_remaining: get_header("x-rl-hourly-remaining"),
            hourly_limit: get_header("x-rl-hourly-limit"),
            daily_remaining: get_header("x-rl-daily-remaining"),
            daily_limit: get_header("x-rl-daily-limit"),
        }
    }

    /// Check if rate limited (any limit at 0)
    pub fn is_rate_limited(&self) -> bool {
        self.hourly_remaining == Some(0) || self.daily_remaining == Some(0)
    }
}

/// API error response
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    /// Error code
    #[serde(default)]
    pub code: Option<u32>,
    /// Error message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_user_info() {
        let json = r#"{
            "user_id": 12345,
            "key": "test_key",
            "name": "TestUser",
            "is_premium": true,
            "is_supporter": false,
            "email": "test@example.com",
            "profile_url": "https://example.com/user"
        }"#;

        let user: UserInfo = serde_json::from_str(json).unwrap();
        assert_eq!(user.user_id, 12345);
        assert_eq!(user.name, "TestUser");
        assert!(user.is_premium);
    }

    #[test]
    fn test_deserialize_download_link() {
        let json = r#"{
            "name": "Nexus CDN",
            "short_name": "Paris",
            "URI": "https://cdn.example.com/file.zip"
        }"#;

        let link: DownloadLink = serde_json::from_str(json).unwrap();
        assert_eq!(link.name, "Nexus CDN");
        assert_eq!(link.uri, "https://cdn.example.com/file.zip");
    }
}
