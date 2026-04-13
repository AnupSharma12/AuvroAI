use reqwest::blocking::{Client, Response};
use std::time::Duration;

fn supabase_base_url() -> String {
    crate::env::supabase_url()
}

#[allow(dead_code)]
pub fn signup_url() -> String {
    format!("{}/auth/v1/signup", supabase_base_url())
}

pub fn signin_url() -> String {
    format!(
        "{}/auth/v1/token?grant_type=password",
        supabase_base_url()
    )
}

#[allow(dead_code)]
pub fn signout_url() -> String {
    format!("{}/auth/v1/logout", supabase_base_url())
}

#[allow(dead_code)]
pub fn refresh_url() -> String {
    format!(
        "{}/auth/v1/token?grant_type=refresh_token",
        supabase_base_url()
    )
}

pub fn signin_with_password(email: &str, password: &str) -> Result<Response, String> {
    let url = signin_url();
    let body = serde_json::json!({
        "email": email,
        "password": password,
    });

    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Login setup failed: {err}"))?;

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("apikey", crate::env::supabase_publishable_key())
        .header(
            "Authorization",
            format!("Bearer {}", crate::env::supabase_publishable_key()),
        )
        .json(&body)
        .send()
        .map_err(|err| format!("Login request failed: {err}"))?;
    Ok(response)
}

#[allow(dead_code)]
pub fn refresh_session(refresh_token: &str) -> Result<Response, String> {
    let endpoint = refresh_url();
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Refresh setup failed: {err}"))?;

    client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("apikey", crate::env::SUPABASE_PUBLISHABLE_KEY)
        .header(
            "Authorization",
            format!("Bearer {}", crate::env::SUPABASE_PUBLISHABLE_KEY),
        )
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .map_err(|err| format!("Refresh request failed: {err}"))
}

