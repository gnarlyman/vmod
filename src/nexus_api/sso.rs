//! Nexus Mods SSO WebSocket authentication.
//!
//! Implements the SSO flow:
//! 1. Generate UUID for session
//! 2. Connect to WebSocket at wss://sso.nexusmods.com
//! 3. Send initial handshake with protocol version 2
//! 4. Open browser for user to authorize
//! 5. Receive API key via WebSocket

use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use tungstenite::{connect, Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use uuid::Uuid;

const SSO_WEBSOCKET_URL: &str = "wss://sso.nexusmods.com";
const APP_SLUG: &str = "vmod";

/// SSO authentication result
#[derive(Debug)]
pub enum SsoResult {
    /// Successfully received API key
    Success(String),
    /// User cancelled or denied authorization
    Cancelled,
    /// Connection or protocol error
    Error(SsoError),
}

/// SSO error types
#[derive(Debug)]
pub enum SsoError {
    /// WebSocket connection failed
    Connection(String),
    /// Failed to send message
    Send(String),
    /// Failed to receive message
    Receive(String),
    /// Invalid response from server
    Protocol(String),
    /// Browser launch failed
    BrowserLaunch(String),
}

impl std::fmt::Display for SsoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsoError::Connection(e) => write!(f, "WebSocket connection failed: {}", e),
            SsoError::Send(e) => write!(f, "Failed to send message: {}", e),
            SsoError::Receive(e) => write!(f, "Failed to receive message: {}", e),
            SsoError::Protocol(e) => write!(f, "Protocol error: {}", e),
            SsoError::BrowserLaunch(e) => write!(f, "Failed to open browser: {}", e),
        }
    }
}

impl std::error::Error for SsoError {}

/// Initial SSO request message
#[derive(Debug, Serialize)]
struct SsoRequest {
    id: String,
    token: Option<String>,
    protocol: u32,
}

/// SSO response message
#[derive(Debug, Deserialize)]
struct SsoResponse {
    success: Option<bool>,
    data: Option<SsoData>,
    error: Option<String>,
    #[serde(default)]
    connection_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SsoData {
    api_key: Option<String>,
    #[serde(default)]
    connection_token: Option<String>,
}

/// SSO authentication handler
pub struct SsoAuth {
    session_id: String,
    socket: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
    connection_token: Option<String>,
}

impl SsoAuth {
    /// Create a new SSO authentication session
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            socket: None,
            connection_token: None,
        }
    }

    /// Get the authorization URL for the browser
    pub fn get_auth_url(&self) -> String {
        format!(
            "https://www.nexusmods.com/sso?id={}&application={}",
            self.session_id, APP_SLUG
        )
    }

    /// Connect to the SSO WebSocket server
    pub fn connect(&mut self) -> Result<(), SsoError> {
        log::info!("Connecting to Nexus SSO at {}", SSO_WEBSOCKET_URL);

        let (socket, response) = connect(SSO_WEBSOCKET_URL)
            .map_err(|e| SsoError::Connection(e.to_string()))?;

        log::debug!("SSO WebSocket connected, status: {}", response.status());

        self.socket = Some(socket);
        Ok(())
    }

    /// Send the initial handshake message
    pub fn send_handshake(&mut self) -> Result<(), SsoError> {
        let socket = self.socket.as_mut()
            .ok_or_else(|| SsoError::Protocol("Not connected".to_string()))?;

        let request = SsoRequest {
            id: self.session_id.clone(),
            token: self.connection_token.clone(),
            protocol: 2,
        };

        let json = serde_json::to_string(&request)
            .map_err(|e| SsoError::Protocol(e.to_string()))?;

        log::debug!("Sending SSO handshake: {}", json);

        socket.send(Message::Text(json.into()))
            .map_err(|e| SsoError::Send(e.to_string()))?;

        // Read initial response (may contain connection_token)
        if let Ok(msg) = socket.read() {
            if let Message::Text(text) = msg {
                log::debug!("Received SSO response: {}", text);
                if let Ok(response) = serde_json::from_str::<SsoResponse>(&text) {
                    // Store connection token for reconnection
                    if let Some(token) = response.connection_token {
                        self.connection_token = Some(token);
                    }
                    if let Some(data) = &response.data {
                        if let Some(token) = &data.connection_token {
                            self.connection_token = Some(token.clone());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Open the browser for user to authorize
    pub fn open_browser(&self) -> Result<(), SsoError> {
        let url = self.get_auth_url();
        log::info!("Opening browser for SSO authorization: {}", url);

        open::that(&url).map_err(|e| SsoError::BrowserLaunch(e.to_string()))?;

        Ok(())
    }

    /// Wait for the API key from the WebSocket.
    /// This blocks until an API key is received or an error occurs.
    pub fn wait_for_api_key(&mut self) -> SsoResult {
        let socket = match self.socket.as_mut() {
            Some(s) => s,
            None => return SsoResult::Error(SsoError::Protocol("Not connected".to_string())),
        };

        log::info!("Waiting for SSO authorization...");

        loop {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    log::debug!("Received SSO message: {}", text);

                    match serde_json::from_str::<SsoResponse>(&text) {
                        Ok(response) => {
                            // Check for error
                            if let Some(error) = response.error {
                                log::error!("SSO error: {}", error);
                                return SsoResult::Error(SsoError::Protocol(error));
                            }

                            // Check for success with API key
                            if response.success == Some(true) {
                                if let Some(data) = response.data {
                                    if let Some(api_key) = data.api_key {
                                        log::info!("SSO authentication successful!");
                                        return SsoResult::Success(api_key);
                                    }
                                }
                            }

                            // Check for explicit failure
                            if response.success == Some(false) {
                                log::warn!("SSO authorization denied or cancelled");
                                return SsoResult::Cancelled;
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to parse SSO response: {}", e);
                            // Continue waiting, might be a status message
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    // Respond to ping with pong
                    if let Err(e) = socket.send(Message::Pong(data)) {
                        log::warn!("Failed to send pong: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    log::info!("SSO WebSocket closed by server");
                    return SsoResult::Cancelled;
                }
                Ok(_) => {
                    // Ignore other message types
                }
                Err(e) => {
                    log::error!("SSO WebSocket error: {}", e);
                    return SsoResult::Error(SsoError::Receive(e.to_string()));
                }
            }
        }
    }

    /// Close the WebSocket connection
    pub fn close(&mut self) {
        if let Some(mut socket) = self.socket.take() {
            let _ = socket.close(None);
            log::debug!("SSO WebSocket closed");
        }
    }

    /// Run the complete SSO flow (blocking)
    ///
    /// This will:
    /// 1. Connect to the WebSocket
    /// 2. Send the handshake
    /// 3. Open the browser
    /// 4. Wait for the API key
    pub fn run_auth_flow(&mut self) -> SsoResult {
        // Connect
        if let Err(e) = self.connect() {
            return SsoResult::Error(e);
        }

        // Send handshake
        if let Err(e) = self.send_handshake() {
            return SsoResult::Error(e);
        }

        // Open browser
        if let Err(e) = self.open_browser() {
            return SsoResult::Error(e);
        }

        // Wait for API key
        let result = self.wait_for_api_key();

        // Clean up
        self.close();

        result
    }
}

impl Default for SsoAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SsoAuth {
    fn drop(&mut self) {
        self.close();
    }
}
