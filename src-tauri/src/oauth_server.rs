// OAuth HTTP server for handling Google OAuth callback

use tiny_http::{Server, Response};
use std::sync::mpsc;

// Embed icon at compile time (avoid absolute paths that break on other machines).
static ICON_PNG: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../public/icon.png"));

pub fn start_oauth_server() -> Result<(String, mpsc::Receiver<String>), String> {
    let (tx, rx) = mpsc::channel();
    
    let port = 8080;
    let server = Server::http(format!("127.0.0.1:{}", port))
        .map_err(|e| format!("Failed to start server: {}", e))?;
    
    let redirect_uri = format!("http://localhost:{}/oauth/callback", port);
    let redirect_uri_clone = redirect_uri.clone();
    
    // Spawn server thread
    std::thread::spawn(move || {
        println!("🌐 OAuth server listening on {}", redirect_uri_clone);
        
        let mut code_sent = false;
        let mut requests_served = 0;
        
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            println!("📥 Received request: {}", url);
            
            // Serve icon.png
            if url == "/icon.png" {
                println!("📷 Serving icon");
                
                let response = Response::from_data(ICON_PNG)
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap()
                    );
                request.respond(response).ok();
                requests_served += 1;
                
                // Close server after serving both callback and icon
                if code_sent && requests_served >= 1 {
                    println!("✅ OAuth server shutting down (served {} requests)", requests_served + 1);
                    break;
                }
                continue;
            }
            
            // Parse authorization code from URL
            if url.starts_with("/oauth/callback") {
                if let Some(code) = extract_code_from_url(&url) {
                    println!("✅ Got authorization code");
                    
                    // Send success HTML response with local icon
                    let html = r#"
                        <!DOCTYPE html>
                        <html>
                        <head>
                            <title>Authentication Successful</title>
                            <style>
                                body {
                                    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
                                    display: flex;
                                    justify-content: center;
                                    align-items: center;
                                    height: 100vh;
                                    margin: 0;
                                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                                }
                                .container {
                                    background: white;
                                    padding: 50px 60px;
                                    border-radius: 16px;
                                    text-align: center;
                                    box-shadow: 0 20px 60px rgba(0,0,0,0.3);
                                    max-width: 400px;
                                }
                                .icon {
                                    width: 80px;
                                    height: 80px;
                                    margin: 0 auto 20px;
                                    border-radius: 16px;
                                    box-shadow: 0 4px 12px rgba(0,0,0,0.1);
                                }
                                h1 { 
                                    color: #4CAF50; 
                                    margin-bottom: 20px;
                                    font-size: 28px;
                                }
                                p { 
                                    color: #666;
                                    font-size: 16px;
                                    line-height: 1.6;
                                }
                            </style>
                            <script>
                                // Clear URL from address bar for security
                                if (window.history.replaceState) {
                                    window.history.replaceState(null, '', '/oauth/callback');
                                }
                                // Auto-close after 3 seconds
                                setTimeout(() => {
                                    window.close();
                                }, 3000);
                            </script>
                        </head>
                        <body>
                            <div class="container">
                                <img class="icon" src="/icon.png" alt="CrackingInterview">
                                <h1>Authentication Successful!</h1>
                                <p>You can close this window and return to CrackingInterview.</p>
                                <p style="font-size: 14px; color: #999; margin-top: 20px;">
                                    This window will close automatically in 3 seconds...
                                </p>
                            </div>
                        </body>
                        </html>
                    "#;
                    
                    let response = Response::from_string(html)
                        .with_header(
                            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap()
                        );
                    
                    request.respond(response).ok();
                    
                    // Send code to receiver
                    tx.send(code).ok();
                    code_sent = true;
                    
                    // Don't break yet - need to serve icon request
                    println!("✅ Callback served, waiting for icon request...");
                } else {
                    // Error response
                    let error_html = r#"
                        <!DOCTYPE html>
                        <html>
                        <body style="font-family: sans-serif; text-align: center; padding: 50px;">
                            <h1 style="color: #f44336;">❌ Authentication Failed</h1>
                            <p>No authorization code received. Please try again.</p>
                        </body>
                        </html>
                    "#;
                    
                    let response = Response::from_string(error_html);
                    request.respond(response).ok();
                    break;
                }
            }
        }
    });
    
    Ok((redirect_uri, rx))
}

fn extract_code_from_url(url: &str) -> Option<String> {
    // URL format: /oauth/callback?code=CODE&scope=...
    url.split('?')
        .nth(1)?
        .split('&')
        .find(|param| param.starts_with("code="))?
        .strip_prefix("code=")?
        .to_string()
        .into()
}
