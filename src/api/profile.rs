/*
create table profiles (
  id uuid primary key references auth.users(id) on delete cascade,
  display_name text,
  avatar_url text,
  theme text default 'system',
  updated_at timestamptz default now()
);

alter table profiles enable row level security;

drop policy if exists "users manage own profile" on profiles;

create policy "profiles_select_own" on profiles
    for select to authenticated
    using (auth.uid() = id);

create policy "profiles_insert_own" on profiles
    for insert to authenticated
    with check (auth.uid() = id);

create policy "profiles_update_own" on profiles
    for update to authenticated
    using (auth.uid() = id)
    with check (auth.uid() = id);

create or replace function handle_new_user()
returns trigger as $$
begin
  insert into public.profiles (id)
  values (new.id)
  on conflict do nothing;
  return new;
end;
$$ language plpgsql security definer;

create or replace trigger on_auth_user_created
  after insert on auth.users
  for each row execute procedure handle_new_user();

insert into storage.buckets (id, name, public)
values ('avatars', 'avatars', true)
on conflict do nothing;

-- For existing users, insert missing profile rows manually:
-- insert into public.profiles (id, theme)
-- select u.id, 'system'
-- from auth.users u
-- left join public.profiles p on p.id = u.id
-- where p.id is null;
*/

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profile {
    pub id: Uuid,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub theme: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Profile {
    pub fn new(user_id: Uuid) -> Self {
        Self {
            id: user_id,
            display_name: None,
            avatar_url: None,
            theme: Some("system".to_owned()),
            updated_at: None,
        }
    }
}

fn auth_request(
    builder: reqwest::blocking::RequestBuilder,
    token: &str,
) -> reqwest::blocking::RequestBuilder {
    builder
        .header("Authorization", format!("Bearer {token}"))
        .header("apikey", crate::env::SUPABASE_PUBLISHABLE_KEY)
        .header("Content-Type", "application/json")
}

fn map_profile_error(status: reqwest::StatusCode, body: String, action: &str) -> String {
    if body.contains("PGRST205") || body.contains("public.profiles") {
        return "Database not set up: profiles table is missing. Run the SQL setup in api/profile.rs first.".to_owned();
    }

    if status == reqwest::StatusCode::FORBIDDEN && body.contains("42501") {
        return "Profile write blocked by Supabase RLS. Ensure INSERT/UPDATE policies for profiles use auth.uid() = id and refresh your login session.".to_owned();
    }

    format!("Could not {action} ({status}): {body}")
}

pub fn get_profile(client: &Client, token: &str, user_id: Uuid) -> Result<Profile, String> {
    let endpoint = format!(
        "{}/profiles?id=eq.{}&select=*",
        crate::env::supabase_rest_url(),
        user_id
    );

    let response = auth_request(client.get(endpoint), token)
        .send()
        .map_err(|err| format!("Failed to fetch profile: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(map_profile_error(status, body, "fetch profile"));
    }

    let rows = response
        .json::<Vec<Profile>>()
        .map_err(|err| format!("Failed to parse profile: {err}"))?;

    Ok(rows.into_iter().next().unwrap_or_else(|| Profile::new(user_id)))
}

#[allow(dead_code)]
pub fn upsert_profile(client: &Client, token: &str, profile: &Profile) -> Result<(), String> {
    let endpoint = format!("{}/profiles", crate::env::supabase_rest_url());

    let response = auth_request(client.post(endpoint), token)
        .header("Prefer", "resolution=merge-duplicates,return=representation")
        .json(&serde_json::json!({
            "id": profile.id,
            "display_name": profile.display_name,
            "avatar_url": profile.avatar_url,
            "theme": profile.theme,
            "updated_at": Utc::now().to_rfc3339(),
        }))
        .send()
        .map_err(|err| format!("Failed to save profile: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(map_profile_error(status, body, "save profile"));
    }

    Ok(())
}

pub fn update_display_name(client: &Client, token: &str, user_id: Uuid, display_name: &str) -> Result<Profile, String> {
    let endpoint = format!(
        "{}/profiles?id=eq.{}",
        crate::env::supabase_rest_url(),
        user_id
    );

    let response = auth_request(client.patch(endpoint), token)
        .header("Prefer", "return=representation")
        .json(&serde_json::json!({
            "display_name": display_name,
            "updated_at": Utc::now().to_rfc3339(),
        }))
        .send()
        .map_err(|err| format!("Failed to update display name: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(map_profile_error(status, body, "update display name"));
    }

    let rows = response
        .json::<Vec<Profile>>()
        .map_err(|err| format!("Failed to parse updated profile: {err}"))?;

    rows.into_iter().next().ok_or_else(|| {
        "No profile row was updated. Ensure a profile row exists for this user (run backfill SQL in api/profile.rs)."
            .to_owned()
    })
}

pub fn update_avatar_url(client: &Client, token: &str, user_id: Uuid, avatar_url: &str) -> Result<Profile, String> {
    let endpoint = format!(
        "{}/profiles?id=eq.{}",
        crate::env::supabase_rest_url(),
        user_id
    );

    let response = auth_request(client.patch(endpoint), token)
        .header("Prefer", "return=representation")
        .json(&serde_json::json!({
            "avatar_url": avatar_url,
            "updated_at": Utc::now().to_rfc3339(),
        }))
        .send()
        .map_err(|err| format!("Failed to update avatar URL: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(map_profile_error(status, body, "update avatar URL"));
    }

    let rows = response
        .json::<Vec<Profile>>()
        .map_err(|err| format!("Failed to parse updated profile: {err}"))?;

    rows.into_iter().next().ok_or_else(|| {
        "No profile row was updated. Ensure a profile row exists for this user (run backfill SQL in api/profile.rs)."
            .to_owned()
    })
}

pub fn update_theme(client: &Client, token: &str, user_id: Uuid, theme: &str) -> Result<Profile, String> {
    let endpoint = format!(
        "{}/profiles?id=eq.{}",
        crate::env::supabase_rest_url(),
        user_id
    );

    let response = auth_request(client.patch(endpoint), token)
        .header("Prefer", "return=representation")
        .json(&serde_json::json!({
            "theme": theme,
            "updated_at": Utc::now().to_rfc3339(),
        }))
        .send()
        .map_err(|err| format!("Failed to update theme: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(map_profile_error(status, body, "update theme"));
    }

    let rows = response
        .json::<Vec<Profile>>()
        .map_err(|err| format!("Failed to parse updated profile: {err}"))?;

    rows.into_iter().next().ok_or_else(|| {
        "No profile row was updated. Ensure a profile row exists for this user (run backfill SQL in api/profile.rs)."
            .to_owned()
    })
}

pub fn update_email(client: &Client, token: &str, new_email: &str) -> Result<(), String> {
    let endpoint = format!("{}/user", crate::env::supabase_auth_url());

    let response = auth_request(client.put(endpoint), token)
        .json(&serde_json::json!({ "email": new_email }))
        .send()
        .map_err(|err| format!("Failed to update email: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not update email ({status}): {body}"));
    }

    Ok(())
}

pub fn update_password(client: &Client, token: &str, new_password: &str) -> Result<(), String> {
    let endpoint = format!("{}/user", crate::env::supabase_auth_url());

    let response = auth_request(client.put(endpoint), token)
        .json(&serde_json::json!({ "password": new_password }))
        .send()
        .map_err(|err| format!("Failed to update password: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not update password ({status}): {body}"));
    }

    Ok(())
}

pub fn upload_avatar(
    client: &Client,
    token: &str,
    user_id: Uuid,
    image_bytes: Vec<u8>,
    mime: &str,
) -> Result<String, String> {
    let base = crate::env::supabase_url();
    let upload_endpoint = format!(
        "{}/storage/v1/object/avatars/{}/avatar.jpg",
        base, user_id
    );

    let response = client
        .post(upload_endpoint)
        .header("Authorization", format!("Bearer {token}"))
        .header("apikey", crate::env::SUPABASE_PUBLISHABLE_KEY)
        .header("Content-Type", mime)
        .header("x-upsert", "true")
        .body(image_bytes)
        .send()
        .map_err(|err| format!("Failed to upload avatar: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not upload avatar ({status}): {body}"));
    }

    Ok(format!(
        "{}/storage/v1/object/public/avatars/{}/avatar.jpg",
        base, user_id
    ))
}

pub fn download_avatar(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|err| format!("Failed to download avatar: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not download avatar ({status}): {body}"));
    }

    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("Failed to read avatar bytes: {err}"))
}

pub fn delete_account(client: &Client, token: &str) -> Result<(), String> {
    let endpoint = format!("{}/user", crate::env::supabase_auth_url());

    let response = client
        .delete(endpoint)
        .header("Authorization", format!("Bearer {token}"))
        .header("apikey", crate::env::SUPABASE_PUBLISHABLE_KEY)
        .send()
        .map_err(|err| format!("Failed to delete account: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not delete account ({status}): {body}"));
    }

    Ok(())
}
