use url::Url;

/// Parsed NXM link data from Nexus Mods
#[derive(Debug, Clone)]
pub struct NxmLink {
    /// Game domain name (e.g., "skyrimspecialedition", "daggerfallunity")
    pub game: String,
    /// Mod ID on Nexus Mods
    pub mod_id: u64,
    /// File ID for the specific download
    pub file_id: u64,
    /// Download key (for premium/authorized downloads)
    pub key: Option<String>,
    /// Expiration timestamp for the download link
    pub expires: Option<u64>,
    /// User ID associated with the download
    pub user_id: Option<u64>,
}

impl NxmLink {
    /// Parse an NXM URL string into an NxmLink struct
    ///
    /// NXM URL format: nxm://gamename/mods/modid/files/fileid?key=...&expires=...&user_id=...
    pub fn parse(nxm_url: &str) -> Result<Self, NxmParseError> {
        // Parse as URL (url crate doesn't recognize nxm:// scheme, so we help it)
        let url = Url::parse(nxm_url).map_err(|e| NxmParseError::InvalidUrl(e.to_string()))?;

        // Verify scheme
        if url.scheme() != "nxm" {
            return Err(NxmParseError::InvalidScheme(url.scheme().to_string()));
        }

        // Game name is the host
        let game = url
            .host_str()
            .ok_or(NxmParseError::MissingGame)?
            .to_string();

        // Parse path segments: /mods/{mod_id}/files/{file_id}
        let segments: Vec<&str> = url
            .path_segments()
            .ok_or(NxmParseError::InvalidPath)?
            .collect();

        if segments.len() < 4 {
            return Err(NxmParseError::InvalidPath);
        }

        if segments[0] != "mods" {
            return Err(NxmParseError::InvalidPath);
        }

        let mod_id: u64 = segments[1]
            .parse()
            .map_err(|_| NxmParseError::InvalidModId)?;

        if segments[2] != "files" {
            return Err(NxmParseError::InvalidPath);
        }

        let file_id: u64 = segments[3]
            .parse()
            .map_err(|_| NxmParseError::InvalidFileId)?;

        // Parse query parameters
        let mut key = None;
        let mut expires = None;
        let mut user_id = None;

        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "key" => key = Some(v.to_string()),
                "expires" => expires = v.parse().ok(),
                "user_id" => user_id = v.parse().ok(),
                _ => {} // Ignore unknown parameters
            }
        }

        Ok(NxmLink {
            game,
            mod_id,
            file_id,
            key,
            expires,
            user_id,
        })
    }

    /// Log the parsed NXM link data using the logging system
    pub fn log_info(&self) {
        log::info!(
            "Received NXM: game={}, mod={}, file={}",
            self.game,
            self.mod_id,
            self.file_id
        );
        if let Some(ref key) = self.key {
            log::debug!("NXM key: {}", key);
        }
        if let Some(expires) = self.expires {
            log::debug!("NXM expires: {}", expires);
        }
        if let Some(user_id) = self.user_id {
            log::debug!("NXM user_id: {}", user_id);
        }
    }
}

#[derive(Debug)]
pub enum NxmParseError {
    InvalidUrl(String),
    InvalidScheme(String),
    MissingGame,
    InvalidPath,
    InvalidModId,
    InvalidFileId,
}

impl std::fmt::Display for NxmParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NxmParseError::InvalidUrl(e) => write!(f, "Invalid URL: {}", e),
            NxmParseError::InvalidScheme(s) => write!(f, "Invalid scheme '{}', expected 'nxm'", s),
            NxmParseError::MissingGame => write!(f, "Missing game name in NXM URL"),
            NxmParseError::InvalidPath => {
                write!(f, "Invalid path, expected /mods/{{id}}/files/{{id}}")
            }
            NxmParseError::InvalidModId => write!(f, "Invalid mod ID"),
            NxmParseError::InvalidFileId => write!(f, "Invalid file ID"),
        }
    }
}

impl std::error::Error for NxmParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_nxm_link() {
        let url = "nxm://skyrimspecialedition/mods/12345/files/67890?key=abc123&expires=1706400000&user_id=999";
        let nxm = NxmLink::parse(url).unwrap();

        assert_eq!(nxm.game, "skyrimspecialedition");
        assert_eq!(nxm.mod_id, 12345);
        assert_eq!(nxm.file_id, 67890);
        assert_eq!(nxm.key, Some("abc123".to_string()));
        assert_eq!(nxm.expires, Some(1706400000));
        assert_eq!(nxm.user_id, Some(999));
    }

    #[test]
    fn test_parse_minimal_nxm_link() {
        let url = "nxm://daggerfallunity/mods/100/files/200";
        let nxm = NxmLink::parse(url).unwrap();

        assert_eq!(nxm.game, "daggerfallunity");
        assert_eq!(nxm.mod_id, 100);
        assert_eq!(nxm.file_id, 200);
        assert_eq!(nxm.key, None);
        assert_eq!(nxm.expires, None);
        assert_eq!(nxm.user_id, None);
    }
}
