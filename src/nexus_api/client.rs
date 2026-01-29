//! Nexus Mods API HTTP client.

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};

use super::types::{ApiError, DownloadLink, ModFilesResponse, ModInfo, RateLimitInfo, UserInfo};

const API_BASE_URL: &str = "https://api.nexusmods.com";
const APP_NAME: &str = "vmod";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error types for API operations
#[derive(Debug)]
pub enum NexusApiError {
    /// HTTP request failed
    Request(reqwest::Error),
    /// API returned an error response
    Api(ApiError),
    /// Rate limit exceeded
    RateLimited(RateLimitInfo),
    /// Invalid API key
    Unauthorized,
    /// Resource not found
    NotFound(String),
    /// JSON parsing failed
    Parse(String),
}

impl std::fmt::Display for NexusApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NexusApiError::Request(e) => write!(f, "HTTP request failed: {}", e),
            NexusApiError::Api(e) => write!(f, "API error: {}", e.message),
            NexusApiError::RateLimited(info) => {
                write!(f, "Rate limited. Hourly remaining: {:?}, Daily remaining: {:?}",
                    info.hourly_remaining, info.daily_remaining)
            }
            NexusApiError::Unauthorized => write!(f, "Invalid or missing API key"),
            NexusApiError::NotFound(resource) => write!(f, "Not found: {}", resource),
            NexusApiError::Parse(msg) => write!(f, "Failed to parse response: {}", msg),
        }
    }
}

impl std::error::Error for NexusApiError {}

impl From<reqwest::Error> for NexusApiError {
    fn from(e: reqwest::Error) -> Self {
        NexusApiError::Request(e)
    }
}

/// Result type with rate limit info
pub struct ApiResponse<T> {
    /// The response data
    pub data: T,
}

/// Nexus Mods API client
pub struct NexusClient {
    client: Client,
    api_key: String,
    /// Default game domain for requests
    pub game_domain: String,
}

impl NexusClient {
    /// Create a new API client with the given API key
    pub fn new(api_key: String, game_domain: String) -> Result<Self, NexusApiError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&format!("{}/{}", APP_NAME, APP_VERSION))
                .unwrap_or_else(|_| HeaderValue::from_static(APP_NAME)),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            api_key,
            game_domain,
        })
    }

    /// Validate the API key and get user information
    pub fn validate_key(&self) -> Result<ApiResponse<UserInfo>, NexusApiError> {
        let url = format!("{}/v1/users/validate.json", API_BASE_URL);

        log::debug!("Validating API key at {}", url);

        let response = self.client
            .get(&url)
            .header("apikey", &self.api_key)
            .send()?;

        let rate_limit = RateLimitInfo::from_headers(response.headers());

        match response.status().as_u16() {
            200 => {
                let user: UserInfo = response.json()
                    .map_err(|e| NexusApiError::Parse(e.to_string()))?;
                log::debug!("API key validated for user: {} (premium: {})", user.name, user.is_premium);
                Ok(ApiResponse { data: user })
            }
            401 => Err(NexusApiError::Unauthorized),
            429 => Err(NexusApiError::RateLimited(rate_limit)),
            _ => {
                let error: ApiError = response.json()
                    .unwrap_or(ApiError { message: "Unknown error".to_string() });
                Err(NexusApiError::Api(error))
            }
        }
    }

    /// Get download links for a specific file.
    ///
    /// The `key` and `expires` parameters come from the NXM link and are required
    /// for generating download URLs.
    pub fn get_download_link(
        &self,
        mod_id: u64,
        file_id: u64,
        key: &str,
        expires: u64,
    ) -> Result<ApiResponse<Vec<DownloadLink>>, NexusApiError> {
        self.get_download_link_for_game(&self.game_domain, mod_id, file_id, key, expires)
    }

    /// Get download links for a specific file in a specific game
    pub fn get_download_link_for_game(
        &self,
        game: &str,
        mod_id: u64,
        file_id: u64,
        key: &str,
        expires: u64,
    ) -> Result<ApiResponse<Vec<DownloadLink>>, NexusApiError> {
        let url = format!(
            "{}/v1/games/{}/mods/{}/files/{}/download_link.json?key={}&expires={}",
            API_BASE_URL, game, mod_id, file_id, key, expires
        );

        log::debug!("Fetching download link: {}", url);

        let response = self.client
            .get(&url)
            .header("apikey", &self.api_key)
            .send()?;

        let rate_limit = RateLimitInfo::from_headers(response.headers());

        match response.status().as_u16() {
            200 => {
                let links: Vec<DownloadLink> = response.json()
                    .map_err(|e| NexusApiError::Parse(e.to_string()))?;
                log::info!("Got {} download link(s) for file {}", links.len(), file_id);
                Ok(ApiResponse { data: links })
            }
            401 => Err(NexusApiError::Unauthorized),
            403 => {
                // Premium required or link expired
                let error: ApiError = response.json()
                    .unwrap_or(ApiError { message: "Download link expired or premium required".to_string() });
                Err(NexusApiError::Api(error))
            }
            404 => Err(NexusApiError::NotFound(format!("file {} for mod {} in game {}", file_id, mod_id, game))),
            429 => Err(NexusApiError::RateLimited(rate_limit)),
            _ => {
                let error: ApiError = response.json()
                    .unwrap_or(ApiError { message: "Unknown error".to_string() });
                Err(NexusApiError::Api(error))
            }
        }
    }

    /// Get mod information from Nexus
    pub fn get_mod_info(&self, mod_id: u64) -> Result<ApiResponse<ModInfo>, NexusApiError> {
        self.get_mod_info_for_game(&self.game_domain, mod_id)
    }

    /// Get mod information for a specific game
    pub fn get_mod_info_for_game(&self, game: &str, mod_id: u64) -> Result<ApiResponse<ModInfo>, NexusApiError> {
        let url = format!("{}/v1/games/{}/mods/{}.json", API_BASE_URL, game, mod_id);

        log::debug!("Fetching mod info: {}", url);

        let response = self.client
            .get(&url)
            .header("apikey", &self.api_key)
            .send()?;

        let rate_limit = RateLimitInfo::from_headers(response.headers());

        match response.status().as_u16() {
            200 => {
                let info: ModInfo = response.json()
                    .map_err(|e| NexusApiError::Parse(e.to_string()))?;
                log::debug!("Got mod info: {} (v{})", info.name, info.version);
                Ok(ApiResponse { data: info })
            }
            401 => Err(NexusApiError::Unauthorized),
            404 => Err(NexusApiError::NotFound(format!("mod {} in game {}", mod_id, game))),
            429 => Err(NexusApiError::RateLimited(rate_limit)),
            _ => {
                let error: ApiError = response.json()
                    .unwrap_or(ApiError { message: "Unknown error".to_string() });
                Err(NexusApiError::Api(error))
            }
        }
    }

    /// Get all files for a mod from Nexus
    pub fn get_mod_files(&self, mod_id: u64) -> Result<ApiResponse<ModFilesResponse>, NexusApiError> {
        self.get_mod_files_for_game(&self.game_domain, mod_id)
    }

    /// Get all files for a mod in a specific game
    pub fn get_mod_files_for_game(&self, game: &str, mod_id: u64) -> Result<ApiResponse<ModFilesResponse>, NexusApiError> {
        let url = format!("{}/v1/games/{}/mods/{}/files.json", API_BASE_URL, game, mod_id);

        log::debug!("Fetching mod files: {}", url);

        let response = self.client
            .get(&url)
            .header("apikey", &self.api_key)
            .send()?;

        let rate_limit = RateLimitInfo::from_headers(response.headers());

        match response.status().as_u16() {
            200 => {
                let files_response: ModFilesResponse = response.json()
                    .map_err(|e| NexusApiError::Parse(e.to_string()))?;
                log::debug!("Got {} files for mod {}", files_response.files.len(), mod_id);
                Ok(ApiResponse { data: files_response })
            }
            401 => Err(NexusApiError::Unauthorized),
            404 => Err(NexusApiError::NotFound(format!("files for mod {} in game {}", mod_id, game))),
            429 => Err(NexusApiError::RateLimited(rate_limit)),
            _ => {
                let error: ApiError = response.json()
                    .unwrap_or(ApiError { message: "Unknown error".to_string() });
                Err(NexusApiError::Api(error))
            }
        }
    }
}
