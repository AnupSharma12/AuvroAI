/*
create table conversations (
  id uuid primary key default gen_random_uuid(),
  user_id uuid references auth.users(id) on delete cascade not null,
  title text not null default 'New Chat',
  created_at timestamptz default now(),
  updated_at timestamptz default now()
);

create table messages (
  id uuid primary key default gen_random_uuid(),
  conversation_id uuid references conversations(id) on delete cascade not null,
  role text not null check (role in ('user', 'assistant', 'system')),
  content text not null,
  created_at timestamptz default now()
);

alter table conversations enable row level security;
alter table messages enable row level security;

create policy "users see own conversations" on conversations
  for all using (auth.uid() = user_id);

create policy "users see own messages" on messages
  for all using (
    conversation_id in (
      select id from conversations where user_id = auth.uid()
    )
  );

create index on messages (conversation_id, created_at);
create index on conversations (user_id, updated_at desc);
*/

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Conversation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

fn supabase_rest_base() -> String {
    crate::env::supabase_rest_url()
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

pub fn list_conversations(client: &Client, token: &str) -> Result<Vec<Conversation>, String> {
    let endpoint = format!(
        "{}/conversations?select=*&order=updated_at.desc",
        supabase_rest_base()
    );

    let response = auth_request(client.get(endpoint), token)
        .send()
        .map_err(|err| format!("Failed to list conversations: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not list conversations ({status}): {body}"));
    }

    response
        .json::<Vec<Conversation>>()
        .map_err(|err| format!("Failed to parse conversations: {err}"))
}

pub fn create_conversation(client: &Client, token: &str, title: &str, user_id: &str) -> Result<Conversation, String> {
    let endpoint = format!("{}/conversations", supabase_rest_base());

    let response = auth_request(client.post(endpoint), token)
        .header("Prefer", "return=representation")
        .json(&serde_json::json!({
            "title": title,
            "user_id": user_id,
        }))
        .send()
        .map_err(|err| format!("Failed to create conversation: {err}"))?;

    let status = response.status();
    let body = response.text().unwrap_or_default();

    if !status.is_success() {
        return Err(format!("Could not create conversation ({status}): {body}"));
    }

    let rows = serde_json::from_str::<Vec<Conversation>>(&body)
        .map_err(|err| format!("Failed to parse created conversation: {err}"))?;

    rows.into_iter()
        .next()
        .ok_or_else(|| "Supabase did not return the created conversation.".to_owned())
}

pub fn rename_conversation(client: &Client, token: &str, id: Uuid, title: &str) -> Result<(), String> {
    let endpoint = format!("{}/conversations?id=eq.{}", supabase_rest_base(), id);

    let response = auth_request(client.patch(endpoint), token)
        .json(&serde_json::json!({
            "title": title,
            "updated_at": Utc::now().to_rfc3339(),
        }))
        .send()
        .map_err(|err| format!("Failed to rename conversation: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not rename conversation ({status}): {body}"));
    }

    Ok(())
}

pub fn delete_conversation(client: &Client, token: &str, id: Uuid) -> Result<(), String> {
    let endpoint = format!("{}/conversations?id=eq.{}", supabase_rest_base(), id);

    let response = auth_request(client.delete(endpoint), token)
        .send()
        .map_err(|err| format!("Failed to delete conversation: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not delete conversation ({status}): {body}"));
    }

    Ok(())
}

pub fn list_messages(client: &Client, token: &str, conversation_id: Uuid) -> Result<Vec<Message>, String> {
    let endpoint = format!(
        "{}/messages?conversation_id=eq.{}&order=created_at.desc&limit=50&select=*",
        supabase_rest_base(),
        conversation_id
    );

    let response = auth_request(client.get(endpoint), token)
        .send()
        .map_err(|err| format!("Failed to list messages: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not list messages ({status}): {body}"));
    }

    let mut messages = response
        .json::<Vec<Message>>()
        .map_err(|err| format!("Failed to parse messages: {err}"))?;

    messages.reverse();
    Ok(messages)
}

pub fn append_message(
    client: &Client,
    token: &str,
    conversation_id: Uuid,
    role: &str,
    content: &str,
) -> Result<Message, String> {
    let endpoint = format!("{}/messages", supabase_rest_base());

    let response = auth_request(client.post(endpoint), token)
        .header("Prefer", "return=representation")
        .json(&serde_json::json!({
            "conversation_id": conversation_id,
            "role": role,
            "content": content,
        }))
        .send()
        .map_err(|err| format!("Failed to append message: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Could not append message ({status}): {body}"));
    }

    let rows = response
        .json::<Vec<Message>>()
        .map_err(|err| format!("Failed to parse appended message: {err}"))?;

    rows.into_iter()
        .next()
        .ok_or_else(|| "Supabase did not return the appended message.".to_owned())
}

pub fn bump_conversation_updated_at(client: &Client, token: &str, id: Uuid) -> Result<(), String> {
    let endpoint = format!("{}/conversations?id=eq.{}", supabase_rest_base(), id);

    let response = auth_request(client.patch(endpoint), token)
        .json(&serde_json::json!({
            "updated_at": Utc::now().to_rfc3339(),
        }))
        .send()
        .map_err(|err| format!("Failed to bump conversation timestamp: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "Could not update conversation timestamp ({status}): {body}"
        ));
    }

    Ok(())
}
