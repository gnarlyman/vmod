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
    /// Requests remaining today
    pub daily_remaining: Option<u32>,
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
            daily_remaining: get_header("x-rl-daily-remaining"),
        }
    }
}

/// API error response
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
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
