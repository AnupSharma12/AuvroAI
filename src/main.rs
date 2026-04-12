mod chat_pipeline;
mod secrets;
mod provider;

use eframe::egui;
use provider::{create_default_provider, Provider};
use secrets::SecretStore;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TITLE_SYSTEM_PROMPT_PREFIX: &str =
    "Generate a short, descriptive chat title (max 5 words, no punctuation) based on this message:";

#[derive(Clone, Debug, Deserialize)]
struct SessionRecord {
    id: String,
    title: String,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MessageRecord {
    id: Option<String>,
    session_id: Option<String>,
    role: String,
    content: String,
    created_at: Option<String>,
}

struct ChatSession {
    id: String,
    name: String,
    messages: Vec<MessageRecord>,
}

struct AuvroApp {
    provider: Box<dyn Provider>,
    draft_message: String,
    sessions: Vec<ChatSession>,
    selected_session: Option<usize>,
    creating_new_chat: bool,
    renaming_session: bool,
    auth_mode: AuthMode,
    auth_full_name: String,
    auth_email: String,
    auth_password: String,
    auth_confirm_password: String,
    auth_notice: Option<String>,
    is_authenticated: bool,
    session_access_token: Option<String>,
    user_id: Option<String>,
    user_email: Option<String>,
    user_full_name: Option<String>,
    supabase_url: String,
    supabase_publishable_key: String,
    auth_error: Option<String>,
    profile_menu_open: bool,
    settings_open: bool,
    settings_notice: Option<String>,
    settings_name_draft: String,
    settings_email_draft: String,
    settings_password_draft: String,
    is_loading: bool,
    error_message: Option<String>,
    pending_response: Option<Vec<char>>,
    streamed_chars: usize,
    stream_session_index: Option<usize>,
    stream_line_index: Option<usize>,
    last_stream_tick: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthMode {
    Login,
    SignUp,
}

impl Default for AuvroApp {
    fn default() -> Self {
        let secret_store = SecretStore::new("AuvroAI");
        let supabase_url = std::env::var("SUPABASE_URL").unwrap_or_default();

        let mut supabase_publishable_key = std::env::var("SUPABASE_PUBLISHABLE_KEY").unwrap_or_default();
        if supabase_publishable_key.trim().is_empty() {
            if let Ok(stored_key) = secret_store.get("SUPABASE_PUBLISHABLE_KEY") {
                supabase_publishable_key = stored_key;
            }
        } else {
            let _ = secret_store.set("SUPABASE_PUBLISHABLE_KEY", &supabase_publishable_key);
        }

        let mut missing = Vec::new();
        if supabase_url.trim().is_empty() {
            missing.push("SUPABASE_URL");
        }
        if supabase_publishable_key.trim().is_empty() {
            missing.push("SUPABASE_PUBLISHABLE_KEY");
        }

        let auth_error = if missing.is_empty() {
            None
        } else {
            Some(format!(
                "Missing required environment variables: {}",
                missing.join(", ")
            ))
        };

        let (is_authenticated, user_id, user_email, user_full_name, auth_notice) =
            if auth_error.is_none() && !supabase_url.is_empty() && !supabase_publishable_key.is_empty() {
                Self::restore_auth_session(&supabase_url, &supabase_publishable_key)
            } else {
                (false, None, None, None, None)
            };

        let mut sessions = Vec::new();
        let mut selected_session = None;
        let mut session_access_token = None;
        if is_authenticated {
            if let Ok(access_token) = SecretStore::new("AuvroAI").get("SUPABASE_ACCESS_TOKEN") {
                session_access_token = Some(access_token.clone());
                if let Some(uid) = user_id.as_deref() {
                    if let Ok(rows) = Self::fetch_sessions(
                        &supabase_url,
                        &supabase_publishable_key,
                        &access_token,
                        uid,
                    ) {
                        sessions = rows
                            .into_iter()
                            .map(|row| ChatSession {
                                id: row.id,
                                name: row.title,
                                messages: Vec::new(),
                            })
                            .collect();

                        if !sessions.is_empty() {
                            selected_session = Some(0);
                            let first_session_id = sessions[0].id.clone();
                            if let Ok(messages) = Self::fetch_messages(
                                &supabase_url,
                                &supabase_publishable_key,
                                &access_token,
                                &first_session_id,
                            ) {
                                sessions[0].messages = messages;
                            }
                        }
                    }
                }
            }
        }

        let settings_name_draft = user_full_name.clone().unwrap_or_default();
        let settings_email_draft = user_email.clone().unwrap_or_default();

        Self {
            provider: create_default_provider(),
            draft_message: String::new(),
            sessions,
            selected_session,
            creating_new_chat: false,
            renaming_session: false,
            auth_mode: AuthMode::Login,
            auth_full_name: String::new(),
            auth_email: String::new(),
            auth_password: String::new(),
            auth_confirm_password: String::new(),
            auth_notice,
            is_authenticated,
            session_access_token,
            user_id,
            user_email,
            user_full_name,
            supabase_url,
            supabase_publishable_key,
            auth_error,
            profile_menu_open: false,
            settings_open: false,
            settings_notice: None,
            settings_name_draft,
            settings_email_draft,
            settings_password_draft: String::new(),
            is_loading: false,
            error_message: None,
            pending_response: None,
            streamed_chars: 0,
            stream_session_index: None,
            stream_line_index: None,
            last_stream_tick: Instant::now(),
        }
    }
}

impl AuvroApp {
    fn restore_auth_session(
        url: &str,
        key: &str,
    ) -> (bool, Option<String>, Option<String>, Option<String>, Option<String>) {
        let secret_store = SecretStore::new("AuvroAI");
        let Ok(token) = secret_store.get("SUPABASE_ACCESS_TOKEN") else {
            return (false, None, None, None, None);
        };

        match Self::fetch_user_profile(url, key, &token) {
            Ok((user_id, email, full_name)) => (
                true,
                Some(user_id),
                Some(email),
                full_name,
                Some("Session restored. Redirecting to chat.".to_owned()),
            ),
            Err(_) => {
                let _ = secret_store.delete("SUPABASE_ACCESS_TOKEN");
                (
                    false,
                    None,
                    None,
                    None,
                    Some("Previous session expired. Please log in again.".to_owned()),
                )
            }
        }
    }

    fn fetch_user_profile(
        url: &str,
        key: &str,
        access_token: &str,
    ) -> Result<(String, String, Option<String>), String> {
        let endpoint = format!("{}/auth/v1/user", url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|e| e.to_string())?;

        let response = client
            .get(endpoint)
            .header("apikey", key)
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        let body: serde_json::Value = response.json().map_err(|e| e.to_string())?;
        let user_id = body
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| "User id missing in Supabase response".to_owned())?;
        let email = body
            .get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| "User email missing in Supabase response".to_owned())?;

        let full_name = Self::extract_full_name(&body);
        Ok((user_id, email, full_name))
    }

    fn extract_full_name(body: &serde_json::Value) -> Option<String> {
        body.get("user_metadata")
            .and_then(|m| m.get("full_name"))
            .and_then(|v| v.as_str())
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
    }

    fn access_token(&self) -> Result<String, String> {
        if let Some(token) = &self.session_access_token {
            if !token.trim().is_empty() {
                return Ok(token.clone());
            }
        }

        SecretStore::new("AuvroAI")
            .get("SUPABASE_ACCESS_TOKEN")
            .map_err(|_| "Missing auth session token. Please log in again.".to_owned())
    }

    fn fetch_sessions(
        url: &str,
        key: &str,
        access_token: &str,
        user_id: &str,
    ) -> Result<Vec<SessionRecord>, String> {
        let endpoint = format!(
            "{}/rest/v1/sessions?select=id,title,created_at,updated_at&user_id=eq.{}&order=updated_at.desc",
            url.trim_end_matches('/'),
            user_id
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|e| e.to_string())?;

        let response = client
            .get(endpoint)
            .header("apikey", key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Accept", "application/json")
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Fetch sessions failed ({status}): {text}"));
        }

        response.json().map_err(|e| e.to_string())
    }

    fn fetch_messages(
        url: &str,
        key: &str,
        access_token: &str,
        session_id: &str,
    ) -> Result<Vec<MessageRecord>, String> {
        let endpoint = format!(
            "{}/rest/v1/messages?select=id,session_id,role,content,created_at&session_id=eq.{}&order=created_at.asc",
            url.trim_end_matches('/'),
            session_id
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|e| e.to_string())?;

        let response = client
            .get(endpoint)
            .header("apikey", key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Accept", "application/json")
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Fetch messages failed ({status}): {text}"));
        }

        response.json().map_err(|e| e.to_string())
    }

    fn insert_session(
        url: &str,
        key: &str,
        access_token: &str,
        user_id: &str,
        title: &str,
    ) -> Result<SessionRecord, String> {
        let endpoint = format!(
            "{}/rest/v1/sessions",
            url.trim_end_matches('/')
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|e| e.to_string())?;

        let response = client
            .post(endpoint)
            .header("apikey", key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .header("Prefer", "return=representation")
            .json(&serde_json::json!({ "user_id": user_id, "title": title }))
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Create session failed ({status}): {text}"));
        }

        let rows: Vec<SessionRecord> = response.json().map_err(|e| e.to_string())?;
        rows.into_iter()
            .next()
            .ok_or_else(|| "Supabase did not return created session".to_owned())
    }

    fn insert_message(
        url: &str,
        key: &str,
        access_token: &str,
        session_id: &str,
        role: &str,
        content: &str,
    ) -> Result<(), String> {
        let endpoint = format!(
            "{}/rest/v1/messages",
            url.trim_end_matches('/')
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|e| e.to_string())?;

        let response = client
            .post(endpoint)
            .header("apikey", key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "session_id": session_id,
                "role": role,
                "content": content
            }))
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Insert message failed ({status}): {text}"));
        }

        Ok(())
    }

    fn touch_session(
        url: &str,
        key: &str,
        access_token: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let endpoint = format!(
            "{}/rest/v1/sessions?id=eq.{}",
            url.trim_end_matches('/'),
            session_id
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|e| e.to_string())?;

        let now = chrono::Utc::now().to_rfc3339();
        let response = client
            .patch(endpoint)
            .header("apikey", key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "updated_at": now }))
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Update session timestamp failed ({status}): {text}"));
        }

        Ok(())
    }

    fn update_supabase_user(
        &self,
        access_token: &str,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let endpoint = format!("{}/auth/v1/user", self.supabase_url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|err| err.to_string())?;

        let response = client
            .put(endpoint)
            .header("apikey", &self.supabase_publishable_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .map_err(|err| err.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Update failed ({status}): {text}"));
        }

        response.json().map_err(|err| err.to_string())
    }

    fn save_profile_name(&mut self) {
        let full_name = self.settings_name_draft.trim().to_owned();
        if full_name.is_empty() {
            self.settings_notice = Some("Name cannot be empty.".to_owned());
            return;
        }

        let access_token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_notice = Some(err);
                return;
            }
        };

        let payload = serde_json::json!({
            "data": { "full_name": full_name },
        });

        match self.update_supabase_user(&access_token, &payload) {
            Ok(body) => {
                self.user_full_name = Self::extract_full_name(&body);
                if self.user_full_name.is_none() {
                    self.user_full_name = Some(self.settings_name_draft.trim().to_owned());
                }
                self.settings_notice = Some("Profile name updated.".to_owned());
            }
            Err(err) => self.settings_notice = Some(err),
        }
    }

    fn change_account_email(&mut self) {
        let email = self.settings_email_draft.trim().to_owned();
        if email.is_empty() || !email.contains('@') {
            self.settings_notice = Some("Enter a valid email address.".to_owned());
            return;
        }

        let access_token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_notice = Some(err);
                return;
            }
        };

        let payload = serde_json::json!({ "email": email });

        match self.update_supabase_user(&access_token, &payload) {
            Ok(body) => {
                self.user_email = body
                    .get("email")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_owned())
                    .or_else(|| Some(self.settings_email_draft.trim().to_owned()));
                self.settings_notice = Some(
                    "Email update requested. Supabase may require email verification.".to_owned(),
                );
            }
            Err(err) => self.settings_notice = Some(err),
        }
    }

    fn change_account_password(&mut self) {
        let password = self.settings_password_draft.clone();
        if password.trim().len() < 6 {
            self.settings_notice = Some("Password must be at least 6 characters.".to_owned());
            return;
        }

        let access_token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_notice = Some(err);
                return;
            }
        };

        let payload = serde_json::json!({ "password": password });

        match self.update_supabase_user(&access_token, &payload) {
            Ok(_) => {
                self.settings_password_draft.clear();
                self.settings_notice = Some("Password changed successfully.".to_owned());
            }
            Err(err) => self.settings_notice = Some(err),
        }
    }

    fn profile_initials(&self) -> String {
        if let Some(full_name) = &self.user_full_name {
            let mut parts = full_name.split_whitespace();
            if let Some(first) = parts.next() {
                let first_char = first.chars().next().unwrap_or('U');
                if let Some(last) = parts.last() {
                    let second_char = last.chars().next().unwrap_or(first_char);
                    return format!(
                        "{}{}",
                        first_char.to_ascii_uppercase(),
                        second_char.to_ascii_uppercase()
                    );
                }

                return first_char.to_ascii_uppercase().to_string();
            }
        }

        self.user_email
            .as_deref()
            .and_then(|email| email.chars().next())
            .map(|ch| ch.to_ascii_uppercase().to_string())
            .unwrap_or_else(|| "U".to_owned())
    }

    fn render_profile_avatar(ui: &mut egui::Ui, initials: &str, radius: f32) {
        let size = egui::vec2(radius * 2.0, radius * 2.0);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter();
        painter.circle_filled(rect.center(), radius, egui::Color32::from_rgb(58, 86, 124));
        painter.circle_stroke(
            rect.center(),
            radius,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(24, 44, 78)),
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            initials,
            egui::FontId::proportional(radius * 0.72),
            egui::Color32::WHITE,
        );
    }

    fn render_account_menu(&mut self, ctx: &egui::Context) {
        if !self.profile_menu_open {
            return;
        }

        egui::Window::new("Account")
            .collapsible(false)
            .resizable(false)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.label(self.user_full_name.as_deref().unwrap_or("AuvroAI User"));
                if let Some(email) = &self.user_email {
                    ui.small(email);
                }
                ui.separator();

                if ui.button("Settings").clicked() {
                    self.settings_open = true;
                    self.profile_menu_open = false;
                    self.settings_notice = None;
                }

                if ui.button("Log Out").clicked() {
                    self.profile_menu_open = false;
                    self.logout();
                }

                if ui.button("Close").clicked() {
                    self.profile_menu_open = false;
                }
            });
    }

    fn render_settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        let initials = self.profile_initials();
        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    Self::render_profile_avatar(ui, &initials, 28.0);
                    ui.vertical(|ui| {
                        ui.label(self.user_full_name.as_deref().unwrap_or("AuvroAI User"));
                        if let Some(email) = &self.user_email {
                            ui.small(email);
                        }
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label("Name");
                ui.add_sized(
                    [420.0, 30.0],
                    egui::TextEdit::singleline(&mut self.settings_name_draft)
                        .hint_text("Your name"),
                );
                if ui.button("Save Name").clicked() {
                    self.save_profile_name();
                }

                ui.add_space(10.0);
                ui.label("Email");
                ui.add_sized(
                    [420.0, 30.0],
                    egui::TextEdit::singleline(&mut self.settings_email_draft)
                        .hint_text("you@example.com"),
                );
                if ui.button("Change Email").clicked() {
                    self.change_account_email();
                }

                ui.add_space(10.0);
                ui.label("New Password");
                ui.add_sized(
                    [420.0, 30.0],
                    egui::TextEdit::singleline(&mut self.settings_password_draft)
                        .password(true)
                        .hint_text("Enter new password"),
                );
                if ui.button("Change Password").clicked() {
                    self.change_account_password();
                }

                if let Some(notice) = &self.settings_notice {
                    ui.add_space(8.0);
                    let is_error = notice.to_ascii_lowercase().contains("failed")
                        || notice.to_ascii_lowercase().contains("missing")
                        || notice.to_ascii_lowercase().contains("valid")
                        || notice.to_ascii_lowercase().contains("least");
                    let color = if is_error {
                        egui::Color32::from_rgb(255, 99, 99)
                    } else {
                        egui::Color32::from_rgb(48, 146, 85)
                    };
                    ui.colored_label(color, notice);
                }

                ui.add_space(10.0);
                if ui.button("Close Settings").clicked() {
                    self.settings_open = false;
                }
            });
    }

    fn login_with_email(&mut self) {
        let email = self.auth_email.trim().to_owned();
        let password = self.auth_password.clone();

        if email.is_empty() || password.is_empty() {
            self.auth_notice = Some("Email and password are required.".to_owned());
            return;
        }

        let endpoint = format!(
            "{}/auth/v1/token?grant_type=password",
            self.supabase_url.trim_end_matches('/')
        );

        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                self.auth_notice = Some(format!("Login setup failed: {err}"));
                return;
            }
        };

        let response = client
            .post(endpoint)
            .header("apikey", &self.supabase_publishable_key)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "email": email, "password": password }))
            .send();

        let response = match response {
            Ok(resp) => resp,
            Err(err) => {
                self.auth_notice = Some(format!("Login request failed: {err}"));
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            self.auth_notice = Some(format!("Login failed ({status}): {text}"));
            return;
        }

        let payload: serde_json::Value = match response.json() {
            Ok(v) => v,
            Err(err) => {
                self.auth_notice = Some(format!("Could not parse login response: {err}"));
                return;
            }
        };

        let access_token = payload
            .get("access_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();

        if access_token.is_empty() {
            self.auth_notice = Some("Login succeeded but no access token was returned.".to_owned());
            return;
        }

        let user_email = payload
            .get("user")
            .and_then(|u| u.get("email"))
            .and_then(|v| v.as_str())
            .unwrap_or(self.auth_email.trim())
            .to_owned();
        let user_full_name = payload
            .get("user")
            .and_then(Self::extract_full_name)
            .or_else(|| {
                let name = self.auth_full_name.trim().to_owned();
                if name.is_empty() {
                    None
                } else {
                    Some(name)
                }
            });
        let user_id = payload
            .get("user")
            .and_then(|u| u.get("id"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let secret_store = SecretStore::new("AuvroAI");
        let _ = secret_store.set("SUPABASE_ACCESS_TOKEN", &access_token);

        self.is_authenticated = true;
        self.session_access_token = Some(access_token.clone());
        self.user_id = user_id.clone();
        self.user_email = Some(user_email.clone());
        self.user_full_name = user_full_name;
        self.settings_email_draft = user_email.clone();
        self.settings_name_draft = self.user_full_name.clone().unwrap_or_default();
        self.sessions.clear();
        self.selected_session = None;
        if let Some(uid) = user_id {
            if let Ok(rows) = Self::fetch_sessions(
                &self.supabase_url,
                &self.supabase_publishable_key,
                &access_token,
                &uid,
            ) {
                self.sessions = rows
                    .into_iter()
                    .map(|row| ChatSession {
                        id: row.id,
                        name: row.title,
                        messages: Vec::new(),
                    })
                    .collect();
                if !self.sessions.is_empty() {
                    self.selected_session = Some(0);
                    self.load_selected_session_messages();
                }
            }
        }
        self.auth_password.clear();
        self.auth_notice = Some(format!("Logged in as {user_email}"));
    }

    fn signup_with_email(&mut self) {
        let full_name = self.auth_full_name.trim().to_owned();
        let email = self.auth_email.trim().to_owned();
        let password = self.auth_password.clone();
        let confirm_password = self.auth_confirm_password.clone();

        if full_name.is_empty() || email.is_empty() || password.is_empty() || confirm_password.is_empty() {
            self.auth_notice = Some("Full name, email, password, and confirm password are required.".to_owned());
            return;
        }

        if password != confirm_password {
            self.auth_notice = Some("Passwords do not match.".to_owned());
            return;
        }

        let endpoint = format!("{}/auth/v1/signup", self.supabase_url.trim_end_matches('/'));
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                self.auth_notice = Some(format!("Signup setup failed: {err}"));
                return;
            }
        };

        let response = client
            .post(endpoint)
            .header("apikey", &self.supabase_publishable_key)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "email": email,
                "password": password,
                "data": { "full_name": full_name },
            }))
            .send();

        let response = match response {
            Ok(resp) => resp,
            Err(err) => {
                self.auth_notice = Some(format!("Signup request failed: {err}"));
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            self.auth_notice = Some(format!("Signup failed ({status}): {text}"));
            return;
        }

        let payload: serde_json::Value = match response.json() {
            Ok(v) => v,
            Err(err) => {
                self.auth_notice = Some(format!("Could not parse signup response: {err}"));
                return;
            }
        };

        let access_token = payload
            .get("access_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();

        if !access_token.is_empty() {
            let user_email = payload
                .get("user")
                .and_then(|u| u.get("email"))
                .and_then(|v| v.as_str())
                .unwrap_or(self.auth_email.trim())
                .to_owned();
            let user_full_name = payload
                .get("user")
                .and_then(Self::extract_full_name)
                .or_else(|| Some(full_name.clone()));
            let user_id = payload
                .get("user")
                .and_then(|u| u.get("id"))
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let secret_store = SecretStore::new("AuvroAI");
            let _ = secret_store.set("SUPABASE_ACCESS_TOKEN", &access_token);

            self.is_authenticated = true;
            self.session_access_token = Some(access_token.clone());
            self.user_id = user_id;
            self.user_email = Some(user_email.clone());
            self.user_full_name = user_full_name;
            self.settings_email_draft = user_email.clone();
            self.settings_name_draft = self.user_full_name.clone().unwrap_or_default();
            self.sessions.clear();
            self.selected_session = None;
            self.creating_new_chat = true;
            self.auth_password.clear();
            self.auth_confirm_password.clear();
            self.auth_notice = Some(format!("Signup successful. Logged in as {user_email}"));
        } else {
            self.auth_notice = Some(
                "Signup successful. Please verify your email, then log in.".to_owned(),
            );
        }
    }

    fn logout(&mut self) {
        let secret_store = SecretStore::new("AuvroAI");
        let _ = secret_store.delete("SUPABASE_ACCESS_TOKEN");
        self.is_authenticated = false;
        self.session_access_token = None;
        self.user_id = None;
        self.user_email = None;
        self.user_full_name = None;
        self.sessions.clear();
        self.selected_session = None;
        self.creating_new_chat = false;
        self.profile_menu_open = false;
        self.settings_open = false;
        self.settings_notice = None;
        self.settings_name_draft.clear();
        self.settings_email_draft.clear();
        self.settings_password_draft.clear();
        self.auth_password.clear();
        self.auth_confirm_password.clear();
        self.auth_notice = Some("Logged out.".to_owned());
    }

    fn render_auth_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.heading("Welcome to AuvroAI");
            ui.label("Log in or sign up with your email account.");
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                let login_selected = self.auth_mode == AuthMode::Login;
                if ui.selectable_label(login_selected, "Log In").clicked() {
                    self.auth_mode = AuthMode::Login;
                    self.auth_notice = None;
                }

                let signup_selected = self.auth_mode == AuthMode::SignUp;
                if ui.selectable_label(signup_selected, "Sign Up").clicked() {
                    self.auth_mode = AuthMode::SignUp;
                    self.auth_notice = None;
                }
            });

            ui.add_space(12.0);

            if self.auth_mode == AuthMode::SignUp {
                ui.label("Full Name");
                ui.add_sized(
                    [380.0, 30.0],
                    egui::TextEdit::singleline(&mut self.auth_full_name).hint_text("Your full name"),
                );
                ui.add_space(8.0);
            }

            ui.label("Email");
            ui.add_sized(
                [380.0, 30.0],
                egui::TextEdit::singleline(&mut self.auth_email).hint_text("you@example.com"),
            );

            ui.add_space(8.0);
            ui.label("Password");
            ui.add_sized(
                [380.0, 30.0],
                egui::TextEdit::singleline(&mut self.auth_password)
                    .password(true)
                    .hint_text("Enter your password"),
            );

            if self.auth_mode == AuthMode::SignUp {
                ui.add_space(8.0);
                ui.label("Confirm Password");
                ui.add_sized(
                    [380.0, 30.0],
                    egui::TextEdit::singleline(&mut self.auth_confirm_password)
                        .password(true)
                        .hint_text("Re-enter your password"),
                );
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let auth_ready = self.auth_error.is_none()
                    && !self.supabase_url.trim().is_empty()
                    && !self.supabase_publishable_key.trim().is_empty();

                match self.auth_mode {
                    AuthMode::Login => {
                        if ui.button("Log In").clicked() {
                            if auth_ready {
                                self.login_with_email();
                            } else {
                                self.auth_notice = Some(
                                    self.auth_error.clone().unwrap_or_else(|| {
                                        "Supabase is not configured. Set SUPABASE_URL and SUPABASE_PUBLISHABLE_KEY in .env."
                                            .to_owned()
                                    }),
                                );
                            }
                        }
                    }
                    AuthMode::SignUp => {
                        if ui.button("Sign Up").clicked() {
                            if auth_ready {
                                self.signup_with_email();
                            } else {
                                self.auth_notice = Some(
                                    self.auth_error.clone().unwrap_or_else(|| {
                                        "Supabase is not configured. Set SUPABASE_URL and SUPABASE_PUBLISHABLE_KEY in .env."
                                            .to_owned()
                                    }),
                                );
                            }
                        }
                    }
                }
            });

            if let Some(notice) = &self.auth_notice {
                ui.add_space(10.0);
                let color = if notice.to_ascii_lowercase().contains("failed")
                    || notice.to_ascii_lowercase().contains("required")
                {
                    egui::Color32::from_rgb(255, 99, 99)
                } else {
                    egui::Color32::from_rgb(48, 146, 85)
                };
                ui.colored_label(color, notice);
            }
        });
    }

    fn active_session(&self) -> Option<&ChatSession> {
        self.selected_session.and_then(|idx| self.sessions.get(idx))
    }

    fn active_session_mut(&mut self) -> Option<&mut ChatSession> {
        self.selected_session
            .and_then(|idx| self.sessions.get_mut(idx))
    }

    fn as_conversation_lines(messages: &[MessageRecord]) -> Vec<String> {
        messages
            .iter()
            .map(|m| {
                if m.role == "user" {
                    format!("You: {}", m.content)
                } else {
                    format!("Auvro: {}", m.content)
                }
            })
            .collect()
    }

    fn normalize_title(title: &str) -> String {
        let cleaned: String = title
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace() {
                    ch
                } else {
                    ' '
                }
            })
            .collect();
        let words = cleaned
            .split_whitespace()
            .take(5)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let joined = words.join(" ").trim().to_owned();
        if joined.is_empty() {
            "New Chat".to_owned()
        } else {
            joined
        }
    }

    fn is_missing_table_error(error: &str) -> bool {
        let lower = error.to_ascii_lowercase();
        lower.contains("pgrst205")
            || lower.contains("could not find the table")
            || lower.contains("schema cache")
    }

    fn local_session_id() -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("local-{millis}")
    }

    fn sidebar_title(title: &str) -> String {
        const MAX_CHARS: usize = 28;
        if title.chars().count() <= MAX_CHARS {
            return title.to_owned();
        }
        let trimmed: String = title.chars().take(MAX_CHARS.saturating_sub(3)).collect();
        format!("{trimmed}...")
    }

    fn load_selected_session_messages(&mut self) {
        let Some(idx) = self.selected_session else {
            return;
        };
        let Some(session_id) = self.sessions.get(idx).map(|s| s.id.clone()) else {
            return;
        };

        if session_id.starts_with("local-") {
            return;
        }

        let access_token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.error_message = Some(err);
                return;
            }
        };

        match Self::fetch_messages(
            &self.supabase_url,
            &self.supabase_publishable_key,
            &access_token,
            &session_id,
        ) {
            Ok(messages) => {
                if let Some(session) = self.sessions.get_mut(idx) {
                    session.messages = messages;
                }
            }
            Err(err) => self.error_message = Some(err),
        }
    }

    fn create_session_for_first_message(&mut self, first_message: &str) -> Result<usize, String> {
        let uid = self
            .user_id
            .clone()
            .ok_or_else(|| "Missing user id. Please log in again.".to_owned())?;
        let access_token = self.access_token()?;

        let system_prompt = format!("{} `{}`", TITLE_SYSTEM_PROMPT_PREFIX, first_message);
        let raw_title = self
            .provider
            .generate_reply_with_system_prompt(&system_prompt, first_message, &[])
            .unwrap_or_else(|_| "New Chat".to_owned());
        let title = Self::normalize_title(&raw_title);

        match Self::insert_session(
            &self.supabase_url,
            &self.supabase_publishable_key,
            &access_token,
            &uid,
            &title,
        ) {
            Ok(row) => {
                self.sessions.insert(
                    0,
                    ChatSession {
                        id: row.id,
                        name: row.title,
                        messages: Vec::new(),
                    },
                );
                self.selected_session = Some(0);
                self.creating_new_chat = false;
                Ok(0)
            }
            Err(err) => {
                if Self::is_missing_table_error(&err) {
                    self.sessions.insert(
                        0,
                        ChatSession {
                            id: Self::local_session_id(),
                            name: title,
                            messages: Vec::new(),
                        },
                    );
                    self.selected_session = Some(0);
                    self.creating_new_chat = false;
                    self.auth_notice = Some(
                        "Supabase sessions/messages tables are missing. Running in local-only chat mode."
                            .to_owned(),
                    );
                    Ok(0)
                } else {
                    Err(err)
                }
            }
        }
    }

    fn push_message_to_db(&self, session_id: &str, role: &str, content: &str) -> Result<(), String> {
        if session_id.starts_with("local-") {
            return Ok(());
        }

        let access_token = self.access_token()?;
        Self::insert_message(
            &self.supabase_url,
            &self.supabase_publishable_key,
            &access_token,
            session_id,
            role,
            content,
        )?;
        Self::touch_session(
            &self.supabase_url,
            &self.supabase_publishable_key,
            &access_token,
            session_id,
        )
    }

    fn reorder_session_to_top(&mut self, index: usize) {
        if index == 0 || index >= self.sessions.len() {
            self.selected_session = Some(index.min(self.sessions.len().saturating_sub(1)));
            return;
        }
        let moved = self.sessions.remove(index);
        self.sessions.insert(0, moved);
        self.selected_session = Some(0);
    }

    fn send_message(&mut self) {
        if self.is_loading {
            return;
        }

        let prompt = self.draft_message.trim().to_owned();
        if prompt.is_empty() {
            return;
        }

        self.draft_message.clear();
        self.error_message = None;

        let session_index = match self.selected_session {
            Some(idx) => idx,
            None => match self.create_session_for_first_message(&prompt) {
                Ok(idx) => idx,
                Err(err) => {
                    self.error_message = Some(err);
                    return;
                }
            },
        };

        let session_id = self.sessions[session_index].id.clone();
        if let Err(err) = self.push_message_to_db(&session_id, "user", &prompt) {
            if Self::is_missing_table_error(&err) {
                self.auth_notice = Some(
                    "Supabase sessions/messages tables are missing. Messages are stored locally only."
                        .to_owned(),
                );
            } else {
                self.error_message = Some(err);
                return;
            }
        }

        self.sessions[session_index].messages.push(MessageRecord {
            id: None,
            session_id: Some(session_id.clone()),
            role: "user".to_owned(),
            content: prompt.clone(),
            created_at: None,
        });

        let conversation = Self::as_conversation_lines(&self.sessions[session_index].messages);
        let full_response = match self.provider.generate_reply(&prompt, &conversation) {
            Ok(reply) => reply,
            Err(err) => {
                self.error_message = Some(format!("Provider error: {err}"));
                return;
            }
        };

        self.sessions[session_index].messages.push(MessageRecord {
            id: None,
            session_id: Some(session_id),
            role: "assistant".to_owned(),
            content: String::new(),
            created_at: None,
        });

        self.pending_response = Some(full_response.chars().collect());
        self.streamed_chars = 0;
        self.stream_session_index = Some(session_index);
        self.stream_line_index = Some(self.sessions[session_index].messages.len() - 1);
        self.is_loading = true;
        self.last_stream_tick = Instant::now();
    }

    fn tick_streaming(&mut self, ctx: &egui::Context) {
        if !self.is_loading {
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(16));

        if self.last_stream_tick.elapsed() < Duration::from_millis(24) {
            return;
        }
        self.last_stream_tick = Instant::now();

        let Some(chars) = self.pending_response.as_ref() else {
            self.is_loading = false;
            return;
        };

        let chars_len = chars.len();
        self.streamed_chars = (self.streamed_chars + 3).min(chars_len);
        let streamed_text: String = chars.iter().take(self.streamed_chars).collect();

        if let (Some(session_idx), Some(line_idx)) = (self.stream_session_index, self.stream_line_index)
        {
            if let Some(session) = self.sessions.get_mut(session_idx) {
                if let Some(message) = session.messages.get_mut(line_idx) {
                    message.content = streamed_text;
                }
            }
        }

        if self.streamed_chars >= chars_len {
            self.is_loading = false;
            self.pending_response = None;

            if let (Some(session_idx), Some(line_idx)) =
                (self.stream_session_index.take(), self.stream_line_index.take())
            {
                if let Some(session) = self.sessions.get(session_idx) {
                    if let Some(message) = session.messages.get(line_idx) {
                        let _ = self.push_message_to_db(&session.id, "assistant", &message.content);
                    }
                }
                self.reorder_session_to_top(session_idx);
            }
        }
    }

    fn render_sessions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("+ New Chat").clicked() {
                self.selected_session = None;
                self.creating_new_chat = true;
                self.error_message = None;
                self.renaming_session = false;
            }
        });

        ui.add_space(8.0);
        ui.heading("Chats");
        ui.add_space(8.0);

        for idx in 0..self.sessions.len() {
            let selected = self.selected_session == Some(idx);
            let label = Self::sidebar_title(&self.sessions[idx].name);

            if ui
                .add_sized(
                    [ui.available_width(), 30.0],
                    egui::Button::new(label).selected(selected),
                )
                .clicked()
            {
                self.selected_session = Some(idx);
                self.creating_new_chat = false;
                self.load_selected_session_messages();
            }
            ui.add_space(4.0);
        }
    }

    fn render_empty_state(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(70.0);
            ui.heading("AuvroAI");
            ui.label("What can I help you with today?");
            ui.add_space(16.0);

            if ui.button("New Chat").clicked() {
                self.creating_new_chat = true;
                self.selected_session = None;
            }

            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                for suggestion in [
                    "Summarize a long article",
                    "Draft a professional email",
                    "Plan my learning roadmap",
                ] {
                    if ui.button(suggestion).clicked() {
                        self.creating_new_chat = true;
                        self.selected_session = None;
                        self.draft_message = suggestion.to_owned();
                    }
                }
            });
        });
    }

    fn render_chat_panel(&mut self, ui: &mut egui::Ui) {
        if self.sessions.is_empty() && !self.creating_new_chat {
            self.render_empty_state(ui);
            return;
        }

        let session_name = self
            .active_session()
            .map(|s| s.name.as_str())
            .unwrap_or("New Chat");

        ui.heading(format!("Chat - {session_name}"));
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .id_salt("chat_scroll")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if let Some(session) = self.active_session() {
                    for message in &session.messages {
                        let prefix = if message.role == "user" { "You" } else { "Auvro" };
                        ui.label(format!("{prefix}: {}", message.content));
                    }
                }
            });

        ui.add_space(8.0);
        if self.is_loading {
            ui.colored_label(egui::Color32::from_rgb(255, 196, 61), "Auvro is typing...");
        }

        if let Some(error) = &self.error_message {
            ui.colored_label(
                egui::Color32::from_rgb(255, 99, 99),
                format!("error: {error}"),
            );
            if ui.button("Clear Error").clicked() {
                self.error_message = None;
            }
        }

        ui.add_space(12.0);
        ui.label("Message");
        let editor = ui.add(
            egui::TextEdit::multiline(&mut self.draft_message)
                .desired_rows(4)
                .hint_text("Type your message..."),
        );

        let (enter_pressed, shift_held) =
            ui.input(|i| (i.key_pressed(egui::Key::Enter), i.modifiers.shift));
        if editor.has_focus() && enter_pressed && !shift_held {
            self.draft_message = self.draft_message.trim_end_matches('\n').to_owned();
            self.send_message();
        }

        let send_clicked = ui
            .add_enabled(
                !self.is_loading
                    && self.auth_error.is_none()
                    && !self.supabase_publishable_key.trim().is_empty(),
                egui::Button::new("Send"),
            )
            .clicked();
        if send_clicked {
            self.send_message();
        }
    }
}

impl eframe::App for AuvroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick_streaming(ctx);

        ctx.set_visuals(egui::Visuals {
            panel_fill: egui::Color32::from_rgb(245, 247, 250),
            window_fill: egui::Color32::from_rgb(250, 251, 253),
            override_text_color: Some(egui::Color32::from_rgb(28, 31, 35)),
            widgets: egui::style::Widgets {
                noninteractive: egui::style::WidgetVisuals {
                    bg_fill: egui::Color32::from_rgb(235, 239, 245),
                    weak_bg_fill: egui::Color32::from_rgb(235, 239, 245),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(195, 203, 214)),
                    corner_radius: egui::CornerRadius::same(6),
                    fg_stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(28, 31, 35)),
                    expansion: 0.0,
                },
                inactive: egui::style::WidgetVisuals {
                    bg_fill: egui::Color32::from_rgb(255, 255, 255),
                    weak_bg_fill: egui::Color32::from_rgb(255, 255, 255),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(195, 203, 214)),
                    corner_radius: egui::CornerRadius::same(6),
                    fg_stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(28, 31, 35)),
                    expansion: 0.0,
                },
                hovered: egui::style::WidgetVisuals {
                    bg_fill: egui::Color32::from_rgb(225, 232, 241),
                    weak_bg_fill: egui::Color32::from_rgb(225, 232, 241),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(145, 155, 168)),
                    corner_radius: egui::CornerRadius::same(6),
                    fg_stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(28, 31, 35)),
                    expansion: 0.0,
                },
                active: egui::style::WidgetVisuals {
                    bg_fill: egui::Color32::from_rgb(208, 219, 231),
                    weak_bg_fill: egui::Color32::from_rgb(208, 219, 231),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(114, 126, 142)),
                    corner_radius: egui::CornerRadius::same(6),
                    fg_stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(28, 31, 35)),
                    expansion: 0.0,
                },
                open: egui::style::WidgetVisuals {
                    bg_fill: egui::Color32::from_rgb(245, 247, 250),
                    weak_bg_fill: egui::Color32::from_rgb(245, 247, 250),
                    bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(195, 203, 214)),
                    corner_radius: egui::CornerRadius::same(6),
                    fg_stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(28, 31, 35)),
                    expansion: 0.0,
                },
            },
            ..egui::Visuals::light()
        });

        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 10.0);
            style.spacing.button_padding = egui::vec2(12.0, 8.0);
            style.spacing.indent = 16.0;
            style.spacing.interact_size = egui::vec2(44.0, 28.0);
            style.text_styles = [
                (
                    egui::TextStyle::Heading,
                    egui::FontId::proportional(26.0),
                ),
                (
                    egui::TextStyle::Name("PanelTitle".into()),
                    egui::FontId::proportional(20.0),
                ),
                (egui::TextStyle::Body, egui::FontId::proportional(16.0)),
                (egui::TextStyle::Button, egui::FontId::proportional(15.0)),
                (egui::TextStyle::Monospace, egui::FontId::monospace(15.0)),
                (egui::TextStyle::Small, egui::FontId::proportional(13.0)),
            ]
            .into();
        });

        let window_width = ctx.input(|i| i.content_rect().width());
        let compact_layout = window_width < 980.0;

        egui::TopBottomPanel::top("app_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("AuvroAI");
                ui.separator();
                ui.label("Your AI Chat Assistant");
                ui.separator();
                if self.is_authenticated {
                    let email = self
                        .user_email
                        .as_deref()
                        .unwrap_or("authenticated user");
                    ui.label(format!("Signed in: {email}"));

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let initials = self.profile_initials();
                            let button = egui::Button::new(initials)
                                .min_size(egui::vec2(34.0, 34.0))
                                .corner_radius(egui::CornerRadius::same(17));
                            if ui.add(button).clicked() {
                                self.profile_menu_open = true;
                            }
                        },
                    );
                }
            });
        });

        if !self.is_authenticated {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.render_auth_panel(ui);
            });
            return;
        }

        if compact_layout {
            egui::TopBottomPanel::top("compact_controls").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Chat");
                    let selected_name = self
                        .selected_session
                        .and_then(|idx| self.sessions.get(idx))
                        .map_or("New Chat", |s| s.name.as_str());

                    egui::ComboBox::from_id_salt("session_selector")
                        .selected_text(selected_name)
                        .show_ui(ui, |ui| {
                            let mut pending_selection: Option<usize> = None;
                            for (idx, session) in self.sessions.iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_session == Some(idx), &session.name)
                                    .clicked()
                                {
                                    pending_selection = Some(idx);
                                }
                            }

                            if let Some(idx) = pending_selection {
                                self.selected_session = Some(idx);
                                self.creating_new_chat = false;
                                self.load_selected_session_messages();
                            }
                        });

                    if ui.button("+ New").clicked() {
                        self.selected_session = None;
                        self.creating_new_chat = true;
                    }
                });
            });
        } else {
            egui::SidePanel::left("session_sidebar")
                .resizable(true)
                .default_width(220.0)
                .show(ctx, |ui| {
                    self.render_sessions(ui);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_chat_panel(ui);
        });

        self.render_account_menu(ctx);
        self.render_settings_window(ctx);
    }
}

fn main() -> Result<(), eframe::Error> {
    let _ = load_environment();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 680.0])
            .with_min_inner_size([360.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AuvroAI",
        options,
        Box::new(|_cc| Ok(Box::<AuvroApp>::default())),
    )
}

fn load_environment() -> bool {
    if let Ok(executable_path) = std::env::current_exe() {
        if let Some(executable_dir) = executable_path.parent() {
            let executable_env = executable_dir.join(".env");
            if executable_env.exists() && dotenvy::from_path(&executable_env).is_ok() {
                return true;
            }
        }
    }

    dotenvy::dotenv().is_ok()
}
