use reqwest::blocking::{Client, Response};

fn supabase_base_url() -> String {
    crate::env::supabase_url().to_string()
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

pub fn signin_with_password_with_client(
    client: &Client,
    email: &str,
    password: &str,
) -> Result<Response, String> {
    let url = signin_url();
    let body = serde_json::json!({
        "email": email,
        "password": password,
    });

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("apikey", crate::env::SUPABASE_PUBLISHABLE_KEY)
        .header(
            "Authorization",
            format!("Bearer {}", crate::env::SUPABASE_PUBLISHABLE_KEY),
        )
        .json(&body)
        .send()
        .map_err(|err| format!("Login request failed: {err}"))?;
    Ok(response)
}

#[allow(dead_code)]
pub fn signin_with_password(email: &str, password: &str) -> Result<Response, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .build()
        .map_err(|err| format!("Login setup failed: {err}"))?;

    signin_with_password_with_client(&client, email, password)
}

#[allow(dead_code)]
pub fn refresh_session_with_client(client: &Client, refresh_token: &str) -> Result<Response, String> {
    let endpoint = refresh_url();
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

#[allow(dead_code)]
pub fn refresh_session(refresh_token: &str) -> Result<Response, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .build()
        .map_err(|err| format!("Refresh setup failed: {err}"))?;

    refresh_session_with_client(&client, refresh_token)
}

