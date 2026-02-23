use std::env;

pub fn get_google_client_id() -> String {
    dotenvy::dotenv().ok();
    env::var("GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| "NOT_CONFIGURED".to_string())
}

pub fn get_google_client_secret() -> String {
    dotenvy::dotenv().ok();
    env::var("GOOGLE_CLIENT_SECRET")
        .unwrap_or_else(|_| "NOT_CONFIGURED".to_string())
}
