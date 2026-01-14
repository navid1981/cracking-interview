// Google OAuth Service for Gemini API access
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64, // Unix timestamp
}

// OAuth configuration - loaded from environment variables
fn get_client_id() -> String {
    std::env::var("GOOGLE_CLIENT_ID")
        .expect("GOOGLE_CLIENT_ID environment variable not set. Copy .env.example to .env and configure it.")
}

fn get_client_secret() -> String {
    std::env::var("GOOGLE_CLIENT_SECRET")
        .expect("GOOGLE_CLIENT_SECRET environment variable not set. Copy .env.example to .env and configure it.")
}

const REDIRECT_URI: &str = "http://localhost:8080/oauth/callback";
const SCOPES: &str = "https://www.googleapis.com/auth/generative-language.retriever";

pub struct GoogleOAuthService {
    tokens: Arc<Mutex<Option<GoogleTokens>>>,
}

impl GoogleOAuthService {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(None)),
        }
    }

    /// Start OAuth flow - returns URL to open in browser
    pub fn get_auth_url(&self) -> String {
        let state = generate_random_state();
        let client_id = get_client_id();
        
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?\
            client_id={}&\
            redirect_uri={}&\
            response_type=code&\
            scope={}&\
            state={}&\
            access_type=offline&\
            prompt=consent",
            client_id,
            urlencoding::encode(REDIRECT_URI),
            urlencoding::encode(SCOPES),
            state
        )
    }

    /// Exchange auth code for tokens
    pub async fn exchange_code(&self, code: &str) -> Result<GoogleTokens, String> {
        let client = reqwest::Client::new();
        let client_id = get_client_id();
        let client_secret = get_client_secret();
        
        let params = [
            ("code", code),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("redirect_uri", REDIRECT_URI),
            ("grant_type", "authorization_code"),
        ];

        let response = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let access_token = token_response["access_token"]
            .as_str()
            .ok_or("No access token")?
            .to_string();

        let refresh_token = token_response["refresh_token"]
            .as_str()
            .map(|s| s.to_string());

        let expires_in = token_response["expires_in"]
            .as_i64()
            .unwrap_or(3600);

        let expires_at = chrono::Utc::now().timestamp() + expires_in;

        let tokens = GoogleTokens {
            access_token,
            refresh_token,
            expires_at,
        };

        // Store tokens
        let mut guard = self.tokens.lock().await;
        *guard = Some(tokens.clone());

        Ok(tokens)
    }

    /// Get current access token (refreshes if expired)
    pub async fn get_access_token(&self) -> Result<String, String> {
        let guard = self.tokens.lock().await;
        
        if let Some(tokens) = guard.as_ref() {
            let now = chrono::Utc::now().timestamp();
            
            // Token still valid
            if now < tokens.expires_at - 300 {
                return Ok(tokens.access_token.clone());
            }
            
            // Need to refresh
            if let Some(refresh_token) = tokens.refresh_token.clone() {
                drop(guard); // Release lock before refresh
                return self.refresh_access_token(&refresh_token).await;
            }
        }
        
        Err("No valid tokens - please sign in".to_string())
    }

    /// Refresh access token
    async fn refresh_access_token(&self, refresh_token: &str) -> Result<String, String> {
        let client = reqwest::Client::new();
        let client_id = get_client_id();
        let client_secret = get_client_secret();
        
        let params = [
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let response = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Refresh failed: {}", e))?;

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))?;

        let access_token = token_response["access_token"]
            .as_str()
            .ok_or("No access token")?
            .to_string();

        let expires_in = token_response["expires_in"]
            .as_i64()
            .unwrap_or(3600);

        let expires_at = chrono::Utc::now().timestamp() + expires_in;

        // Update stored tokens
        let mut guard = self.tokens.lock().await;
        if let Some(tokens) = guard.as_mut() {
            tokens.access_token = access_token.clone();
            tokens.expires_at = expires_at;
        }

        Ok(access_token)
    }

    /// Save tokens to file
    pub async fn save_tokens(&self, path: &str) -> Result<(), String> {
        let guard = self.tokens.lock().await;
        
        if let Some(tokens) = guard.as_ref() {
            let json = serde_json::to_string_pretty(tokens)
                .map_err(|e| format!("Serialize failed: {}", e))?;
            
            std::fs::write(path, json)
                .map_err(|e| format!("Write failed: {}", e))?;
        }
        
        Ok(())
    }

    /// Load tokens from file
    pub async fn load_tokens(&self, path: &str) -> Result<(), String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Read failed: {}", e))?;
        
        let tokens: GoogleTokens = serde_json::from_str(&json)
            .map_err(|e| format!("Parse failed: {}", e))?;
        
        let mut guard = self.tokens.lock().await;
        *guard = Some(tokens);
        
        Ok(())
    }
}

fn generate_random_state() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect()
}
