// Google OAuth Service for Gemini API access
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use sha2::{Digest, Sha256};
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64, // Unix timestamp
    pub id_token: Option<String>,
    pub user_email: Option<String>,
}

// OAuth configuration - loaded from environment variables
fn get_client_id() -> String {
    std::env::var("GOOGLE_CLIENT_ID")
        .expect("GOOGLE_CLIENT_ID environment variable not set. Copy .env.example to .env and configure it.")
}

// Desktop apps should use OAuth Authorization Code flow with PKCE (no client_secret embedded).
// Scope note: we include OpenID scopes so we can *display* who is signed in (email) for UX clarity.
// Gemini access scope is kept as-is; adjust if you change permissions.
const SCOPES: &str = "https://www.googleapis.com/auth/generative-language.retriever openid email profile";

pub struct GoogleOAuthService {
    tokens: Arc<Mutex<Option<GoogleTokens>>>,
    pending_pkce: Arc<Mutex<HashMap<String, String>>>, // state -> code_verifier
}

impl GoogleOAuthService {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(None)),
            pending_pkce: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start OAuth flow - returns URL to open in browser
    pub async fn get_auth_url(&self, redirect_uri: &str) -> String {
        let state = generate_random_state();
        let client_id = get_client_id();

        // PKCE: generate verifier + challenge (S256)
        let code_verifier = generate_code_verifier();
        let code_challenge = pkce_challenge_s256(&code_verifier);

        {
            let mut guard = self.pending_pkce.lock().await;
            guard.insert(state.clone(), code_verifier);
        }
        
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?\
            client_id={}&\
            redirect_uri={}&\
            response_type=code&\
            scope={}&\
            state={}&\
            code_challenge={}&\
            code_challenge_method=S256&\
            access_type=offline&\
            prompt=consent",
            client_id,
            urlencoding::encode(redirect_uri),
            urlencoding::encode(SCOPES),
            state,
            urlencoding::encode(&code_challenge)
        )
    }

    /// Exchange auth code for tokens
    pub async fn exchange_code(&self, code: &str, state: &str, redirect_uri: &str) -> Result<GoogleTokens, String> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| format!("Client build failed: {}", e))?;
        let client_id = get_client_id();

        // Look up PKCE verifier for this state
        let code_verifier = {
            let mut guard = self.pending_pkce.lock().await;
            guard
                .remove(state)
                .ok_or_else(|| "Missing PKCE verifier (state mismatch). Please try signing in again.".to_string())?
        };

        // Some Google OAuth client types still accept/require client_secret.
        // For Desktop+PKCE clients, it should NOT be required; we only include it if present.
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").ok();

        let mut params: Vec<(&str, &str)> = vec![
            ("code", code),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier.as_str()),
        ];
        if let Some(secret) = client_secret.as_deref() {
            params.push(("client_secret", secret));
        }

        let response = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let status = response.status();
        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if !status.is_success() {
            let msg = token_response["error_description"]
                .as_str()
                .or_else(|| token_response["error"].as_str())
                .or_else(|| token_response["error"]["message"].as_str())
                .unwrap_or("OAuth token exchange failed");
            return Err(format!(
                "OAuth token exchange failed (HTTP {}): {}",
                status.as_u16(),
                msg
            ));
        }

        let access_token = token_response["access_token"]
            .as_str()
            .ok_or("No access token")?
            .to_string();

        let refresh_token = token_response["refresh_token"]
            .as_str()
            .map(|s| s.to_string());

        let id_token = token_response["id_token"]
            .as_str()
            .map(|s| s.to_string());

        let expires_in = token_response["expires_in"]
            .as_i64()
            .unwrap_or(3600);

        let expires_at = chrono::Utc::now().timestamp() + expires_in;

        let user_email = id_token
            .as_deref()
            .and_then(extract_email_from_id_token);

        let tokens = GoogleTokens {
            access_token,
            refresh_token,
            expires_at,
            id_token,
            user_email,
        };

        // Store tokens
        let mut guard = self.tokens.lock().await;
        *guard = Some(tokens.clone());

        Ok(tokens)
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
}

fn generate_random_state() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect()
}

fn generate_code_verifier() -> String {
    // RFC 7636: 43-128 chars, unreserved characters. We'll use a 64-char random string.
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..64)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect()
}

fn pkce_challenge_s256(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn extract_email_from_id_token(id_token: &str) -> Option<String> {
    // JWT: header.payload.signature (base64url)
    let payload_b64 = id_token.split('.').nth(1)?;
    let bytes = general_purpose::URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    json.get("email")?.as_str().map(|s| s.to_string())
}
