
pub fn get_google_client_id() -> String {
    // 1. Try compile-time injection (useful for production binaries)
    if let Some(id) = option_env!("GOOGLE_CLIENT_ID") {
        return id.to_string();
    }
    
    // 2. Fallback to runtime environment variables (useful for local development)
    dotenvy::dotenv().ok();
    std::env::var("GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| "NOT_CONFIGURED".to_string())
}

pub fn get_google_client_secret() -> String {
    // 1. Try compile-time injection
    if let Some(secret) = option_env!("GOOGLE_CLIENT_SECRET") {
        return secret.to_string();
    }

    // 2. Fallback to runtime environment variables
    dotenvy::dotenv().ok();
    std::env::var("GOOGLE_CLIENT_SECRET")
        .unwrap_or_else(|_| "NOT_CONFIGURED".to_string())
}
