mod api;
mod chat_pipeline;
mod cache;
mod env;
mod secrets;
mod provider;
mod ui;

use api::conversations::{self, Conversation, Message};
use api::profile::{self, Profile};
use eframe::egui;
use egui_commonmark::CommonMarkCache;
use cache::model_metadata::ModelMetadataCache;
use provider::{create_default_provider, Provider};
use secrets::SecretStore;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone, Debug)]
enum SupabaseEvent {
    ConversationsLoaded(Result<Vec<Conversation>, String>),
    ConversationCreated(Result<Conversation, String>),
    MessagesLoaded {
        conversation_id: Uuid,
        result: Result<Vec<Message>, String>,
    },
    ConversationDeleted {
        conversation_id: Uuid,
        result: Result<(), String>,
    },
    MessageAppended(Result<Message, String>),
    ConversationBumped {
        conversation_id: Uuid,
        result: Result<(), String>,
    },
    ConversationRetitled {
        conversation_id: Uuid,
        title: String,
    },
    ProfileLoaded(Result<Profile, String>),
    DisplayNameUpdated(Result<Profile, String>),
    ThemeUpdated(Result<Profile, String>),
    EmailUpdated(Result<String, String>),
    PasswordUpdated(Result<(), String>),
    AvatarUploaded(Result<String, String>),
    AvatarUrlUpdated(Result<Profile, String>),
    AvatarDownloaded(Result<(String, Vec<u8>), String>),
    AccountDeleted(Result<(), String>),
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsStatus {
    pub(crate) message: String,
    pub(crate) is_error: bool,
    pub(crate) expires_at: Option<Instant>,
}

impl SettingsStatus {
    fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_error: false,
            expires_at: None,
        }
    }

    fn success_for(message: impl Into<String>, duration: Duration) -> Self {
        Self {
            message: message.into(),
            is_error: false,
            expires_at: Some(Instant::now() + duration),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_error: true,
            expires_at: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn from_profile(value: Option<&str>) -> Self {
        match value.unwrap_or("system").to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }
}

#[derive(Clone, Debug)]
struct PendingTitleGeneration {
    conversation_id: Uuid,
    first_user_message: String,
}

pub(crate) struct AuvroApp {
    pub(crate) provider: Arc<dyn Provider>,
    pub(crate) draft_message: String,
    pub(crate) conversations: Vec<Conversation>,
    pub(crate) active_conversation_id: Option<Uuid>,
    pub(crate) messages: Vec<Message>,
    pub(crate) messages_loading: bool,
    pub(crate) pending_delete_conversation_id: Option<Uuid>,
    pub(crate) creating_new_chat: bool,
    pub(crate) auth_mode: AuthMode,
    pub(crate) auth_full_name: String,
    pub(crate) auth_email: String,
    pub(crate) auth_password: String,
    pub(crate) auth_confirm_password: String,
    pub(crate) auth_notice: Option<String>,
    pub(crate) is_authenticated: bool,
    pub(crate) session_access_token: Option<String>,
    pub(crate) user_id: Option<String>,
    pub(crate) user_email: Option<String>,
    pub(crate) user_full_name: Option<String>,
    pub(crate) auth_error: Option<String>,
    pub(crate) profile_menu_open: bool,
    pub(crate) show_settings: bool,
    pub(crate) show_sidebar: bool,
    pub(crate) settings_name_status: Option<SettingsStatus>,
    pub(crate) settings_email_status: Option<SettingsStatus>,
    pub(crate) settings_password_status: Option<SettingsStatus>,
    pub(crate) settings_photo_status: Option<SettingsStatus>,
    pub(crate) settings_theme_status: Option<SettingsStatus>,
    pub(crate) settings_account_status: Option<SettingsStatus>,
    pub(crate) settings_name_draft: String,
    pub(crate) settings_email_draft: String,
    pub(crate) settings_password_draft: String,
    pub(crate) settings_password_confirm_draft: String,
    pub(crate) delete_account_confirmation: String,
    pub(crate) theme_preference: ThemePreference,
    pub(crate) profile: Option<Profile>,
    pub(crate) avatar_texture: Option<egui::TextureHandle>,
    #[allow(dead_code)]
    pub(crate) model_cache: ModelMetadataCache,
    pub(crate) markdown_cache: CommonMarkCache,
    pub(crate) is_loading: bool,
    pub(crate) profile_loading: bool,
    pub(crate) profile_menu_anchor: Option<egui::Pos2>,
    pub(crate) error_message: Option<String>,
    pub(crate) pending_response: Option<String>,
    pub(crate) streaming_buffer: String,
    pub(crate) streamed_chars: usize,
    pub(crate) stream_conversation_id: Option<Uuid>,
    pub(crate) stream_line_index: Option<usize>,
    pub(crate) pending_prompt_after_create: Option<String>,
    pub(crate) last_stream_tick: Instant,
    provider_response_rx: Option<mpsc::Receiver<Result<String, String>>>,
    stream_cancellation_token: Option<CancellationToken>,
    pending_title_generation: Option<PendingTitleGeneration>,
    supabase_runtime: Arc<tokio::runtime::Runtime>,
    supabase_events_tx: mpsc::Sender<SupabaseEvent>,
    supabase_events_rx: mpsc::Receiver<SupabaseEvent>,
}

pub(crate) type AppState = AuvroApp;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthMode {
    Login,
    SignUp,
}

impl Default for AuvroApp {
    fn default() -> Self {
        let supabase_runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to initialize Supabase task runtime"),
        );
        let (supabase_events_tx, supabase_events_rx) = mpsc::channel();

        let mut missing = Vec::new();
        if crate::env::SUPABASE_URL.trim().is_empty() {
            missing.push("SUPABASE_URL");
        }
        if crate::env::SUPABASE_PUBLISHABLE_KEY.trim().is_empty() {
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

        let (is_authenticated, user_id, user_email, user_full_name, auth_notice) = if auth_error
            .is_none()
            && !crate::env::SUPABASE_URL.is_empty()
            && !crate::env::SUPABASE_PUBLISHABLE_KEY.is_empty()
        {
            Self::restore_auth_session(
                crate::env::SUPABASE_URL,
                crate::env::SUPABASE_PUBLISHABLE_KEY,
            )
        } else {
            (false, None, None, None, None)
        };

        let session_access_token = if is_authenticated {
            SecretStore::new("AuvroAI").get("SUPABASE_ACCESS_TOKEN").ok()
        } else {
            None
        };

        let settings_name_draft = user_full_name.clone().unwrap_or_default();
        let settings_email_draft = user_email.clone().unwrap_or_default();

        let mut app = Self {
            provider: create_default_provider(),
            draft_message: String::new(),
            conversations: Vec::new(),
            active_conversation_id: None,
            messages: Vec::new(),
            messages_loading: false,
            pending_delete_conversation_id: None,
            creating_new_chat: false,
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
            auth_error,
            profile_menu_open: false,
            show_settings: false,
            show_sidebar: true,
            settings_name_status: None,
            settings_email_status: None,
            settings_password_status: None,
            settings_photo_status: None,
            settings_theme_status: None,
            settings_account_status: None,
            settings_name_draft,
            settings_email_draft,
            settings_password_draft: String::new(),
            settings_password_confirm_draft: String::new(),
            delete_account_confirmation: String::new(),
            theme_preference: ThemePreference::System,
            profile: None,
            avatar_texture: None,
            model_cache: ModelMetadataCache::new(Duration::from_secs(600)),
            markdown_cache: CommonMarkCache::default(),
            is_loading: false,
            profile_loading: false,
            profile_menu_anchor: None,
            error_message: None,
            pending_response: None,
            streaming_buffer: String::new(),
            streamed_chars: 0,
            stream_conversation_id: None,
            stream_line_index: None,
            pending_prompt_after_create: None,
            last_stream_tick: Instant::now(),
            provider_response_rx: None,
            stream_cancellation_token: None,
            pending_title_generation: None,
            supabase_runtime,
            supabase_events_tx,
            supabase_events_rx,
        };

        if app.is_authenticated {
            app.request_list_conversations();
            app.request_load_profile();
        }

        app
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
        let endpoint = format!(
            "{}/auth/v1/user",
            crate::env::normalized_supabase_url(url)
        );
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

    fn current_user_uuid(&self) -> Result<Uuid, String> {
        let user_id = self
            .user_id
            .as_deref()
            .ok_or_else(|| "Missing user id. Please log in again.".to_owned())?;
        Uuid::parse_str(user_id).map_err(|err| format!("Invalid user id '{user_id}': {err}"))
    }

    fn expire_settings_statuses(&mut self) {
        let now = Instant::now();
        for status in [
            &mut self.settings_name_status,
            &mut self.settings_email_status,
            &mut self.settings_password_status,
            &mut self.settings_photo_status,
            &mut self.settings_theme_status,
            &mut self.settings_account_status,
        ] {
            let expired = status
                .as_ref()
                .and_then(|s| s.expires_at)
                .is_some_and(|expires_at| expires_at <= now);
            if expired {
                *status = None;
            }
        }
    }

    fn request_load_profile(&mut self) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.error_message = Some(err);
                return;
            }
        };

        let user_id = match self.current_user_uuid() {
            Ok(user_id) => user_id,
            Err(err) => {
                self.error_message = Some(err);
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || profile::get_profile(&token, user_id))
                .await
                .map_err(|err| format!("Failed to run profile-load task: {err}"))
                .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::ProfileLoaded(result));
        });
    }

    fn request_update_display_name(&mut self, display_name: String) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_name_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        let user_id = match self.current_user_uuid() {
            Ok(user_id) => user_id,
            Err(err) => {
                self.settings_name_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                profile::update_display_name(&token, user_id, &display_name)
            })
            .await
            .map_err(|err| format!("Failed to run display-name update task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::DisplayNameUpdated(result));
        });
    }

    fn request_update_theme(&mut self, theme: ThemePreference) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_theme_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        let user_id = match self.current_user_uuid() {
            Ok(user_id) => user_id,
            Err(err) => {
                self.settings_theme_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                profile::update_theme(&token, user_id, theme.as_str())
            })
            .await
            .map_err(|err| format!("Failed to run theme-update task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::ThemeUpdated(result));
        });
    }

    fn request_update_email(&mut self, new_email: String) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_email_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let email = new_email.clone();
            let result = tokio::task::spawn_blocking(move || {
                profile::update_email(&token, &new_email)?;
                Ok::<String, String>(email)
            })
            .await
            .map_err(|err| format!("Failed to run email-update task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::EmailUpdated(result));
        });
    }

    fn request_update_password(&mut self, new_password: String) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_password_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || profile::update_password(&token, &new_password))
                .await
                .map_err(|err| format!("Failed to run password-update task: {err}"))
                .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::PasswordUpdated(result));
        });
    }

    fn request_upload_avatar(&mut self, image_bytes: Vec<u8>, mime: String) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_photo_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        let user_id = match self.current_user_uuid() {
            Ok(user_id) => user_id,
            Err(err) => {
                self.settings_photo_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                profile::upload_avatar(&token, user_id, image_bytes, &mime)
            })
            .await
            .map_err(|err| format!("Failed to run avatar-upload task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::AvatarUploaded(result));
        });
    }

    fn request_update_avatar_url(&mut self, avatar_url: String) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_photo_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        let user_id = match self.current_user_uuid() {
            Ok(user_id) => user_id,
            Err(err) => {
                self.settings_photo_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                profile::update_avatar_url(&token, user_id, &avatar_url)
            })
            .await
            .map_err(|err| format!("Failed to run avatar-url update task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::AvatarUrlUpdated(result));
        });
    }

    fn request_download_avatar(&self, avatar_url: String) {
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let url = avatar_url.clone();
            let result = tokio::task::spawn_blocking(move || {
                let bytes = profile::download_avatar(&avatar_url)?;
                Ok::<(String, Vec<u8>), String>((url, bytes))
            })
            .await
            .map_err(|err| format!("Failed to run avatar-download task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::AvatarDownloaded(result));
        });
    }

    pub(crate) fn request_delete_account(&mut self) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.settings_account_status = Some(SettingsStatus::error(err));
                return;
            }
        };

        self.profile_loading = true;
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || profile::delete_account(&token))
                .await
                .map_err(|err| format!("Failed to run account-delete task: {err}"))
                .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::AccountDeleted(result));
        });
    }

    fn decode_avatar_image(bytes: &[u8]) -> Result<egui::ColorImage, String> {
        let decoded = image::load_from_memory(bytes)
            .map_err(|err| format!("Failed to decode avatar image: {err}"))?
            .to_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let pixels = decoded.into_raw();
        Ok(egui::ColorImage::from_rgba_unmultiplied(size, &pixels))
    }

    fn request_list_conversations(&mut self) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.error_message = Some(err);
                return;
            }
        };

        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || conversations::list_conversations(&token))
                .await
                .map_err(|err| format!("Failed to run conversation-list task: {err}"))
                .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::ConversationsLoaded(result));
        });
    }

    pub(crate) fn request_create_conversation(&mut self, title: &str) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.error_message = Some(err);
                self.creating_new_chat = false;
                self.messages_loading = false;
                return;
            }
        };

        let user_id = match self.user_id.as_deref() {
            Some(user_id) if !user_id.trim().is_empty() => user_id.to_owned(),
            _ => {
                self.error_message = Some("Missing user id. Please log in again.".to_owned());
                self.creating_new_chat = false;
                self.messages_loading = false;
                return;
            }
        };

        let title = title.to_owned();
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                conversations::create_conversation(&token, &title, &user_id)
            })
                .await
                .map_err(|err| format!("Failed to run conversation-create task: {err}"))
                .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::ConversationCreated(result));
        });
    }

    pub(crate) fn request_load_messages(&mut self, conversation_id: Uuid) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.error_message = Some(err);
                self.messages_loading = false;
                return;
            }
        };

        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || conversations::list_messages(&token, conversation_id))
                .await
                .map_err(|err| format!("Failed to run messages-list task: {err}"))
                .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::MessagesLoaded {
                conversation_id,
                result,
            });
        });
    }

    pub(crate) fn request_delete_conversation(&mut self, conversation_id: Uuid) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(err) => {
                self.error_message = Some(err);
                return;
            }
        };

        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || conversations::delete_conversation(&token, conversation_id))
                .await
                .map_err(|err| format!("Failed to run conversation-delete task: {err}"))
                .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::ConversationDeleted {
                conversation_id,
                result,
            });
        });
    }

    fn request_append_message(&self, conversation_id: Uuid, role: &str, content: String) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(_) => return,
        };

        let role = role.to_owned();
        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                conversations::append_message(&token, conversation_id, &role, &content)
            })
            .await
            .map_err(|err| format!("Failed to run append-message task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::MessageAppended(result));
        });
    }

    fn request_bump_conversation_updated_at(&self, conversation_id: Uuid) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(_) => return,
        };

        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                conversations::bump_conversation_updated_at(&token, conversation_id)
            })
            .await
            .map_err(|err| format!("Failed to run bump-conversation task: {err}"))
            .and_then(|result| result);
            let _ = tx.send(SupabaseEvent::ConversationBumped {
                conversation_id,
                result,
            });
        });
    }

    fn request_generate_conversation_title(&self, conversation_id: Uuid, first_user_message: String) {
        let token = match self.access_token() {
            Ok(token) => token,
            Err(_) => return,
        };

        let runtime = Arc::clone(&self.supabase_runtime);
        let tx = self.supabase_events_tx.clone();
        runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let title = crate::api::ai::generate_title(&first_user_message)?;
                conversations::rename_conversation(&token, conversation_id, &title)?;
                Ok::<(Uuid, String), String>((conversation_id, title))
            })
            .await
            .map_err(|err| format!("Failed to run title-generation task: {err}"))
            .and_then(|result| result);

            if let Ok((conversation_id, title)) = result {
                let _ = tx.send(SupabaseEvent::ConversationRetitled {
                    conversation_id,
                    title,
                });
            }
        });
    }

    fn sort_conversations_desc(&mut self) {
        self.conversations
            .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    }

    fn process_supabase_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.supabase_events_rx.try_recv() {
            match event {
                SupabaseEvent::ConversationsLoaded(result) => match result {
                    Ok(conversations) => {
                        self.conversations = conversations;
                        self.sort_conversations_desc();
                    }
                    Err(err) => self.error_message = Some(err),
                },
                SupabaseEvent::ConversationCreated(result) => match result {
                    Ok(conversation) => {
                        self.conversations.retain(|item| item.id != conversation.id);
                        self.conversations.insert(0, conversation.clone());
                        self.active_conversation_id = Some(conversation.id);
                        self.messages.clear();
                        self.messages_loading = false;
                        self.creating_new_chat = false;

                        if let Some(prompt) = self.pending_prompt_after_create.take() {
                            self.send_prompt_to_conversation(conversation.id, prompt);
                        }
                    }
                    Err(err) => {
                        self.creating_new_chat = false;
                        self.messages_loading = false;
                        self.pending_prompt_after_create = None;
                        self.error_message = Some(err);
                    }
                },
                SupabaseEvent::MessagesLoaded {
                    conversation_id,
                    result,
                } => {
                    if self.active_conversation_id != Some(conversation_id) {
                        continue;
                    }

                    self.messages_loading = false;
                    match result {
                        Ok(messages) => self.messages = messages,
                        Err(err) => self.error_message = Some(err),
                    }
                }
                SupabaseEvent::ConversationDeleted {
                    conversation_id,
                    result,
                } => match result {
                    Ok(()) => {
                        self.conversations.retain(|item| item.id != conversation_id);
                        if self.active_conversation_id == Some(conversation_id) {
                            self.active_conversation_id = None;
                            self.messages.clear();
                            self.messages_loading = false;
                        }
                    }
                    Err(err) => self.error_message = Some(err),
                },
                SupabaseEvent::MessageAppended(result) => {
                    match result {
                        Ok(message) => {
                            if message.role == "assistant" {
                                if let Some(pending) = self.pending_title_generation.take() {
                                    if pending.conversation_id == message.conversation_id {
                                        self.request_generate_conversation_title(
                                            pending.conversation_id,
                                            pending.first_user_message,
                                        );
                                    } else {
                                        self.pending_title_generation = Some(pending);
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            self.pending_title_generation = None;
                            self.error_message = Some(err);
                        }
                    }
                }
                SupabaseEvent::ConversationBumped {
                    conversation_id,
                    result,
                } => match result {
                    Ok(()) => {
                        if let Some(conversation) =
                            self.conversations.iter_mut().find(|item| item.id == conversation_id)
                        {
                            conversation.updated_at = chrono::Utc::now();
                        }
                        self.sort_conversations_desc();
                    }
                    Err(err) => self.error_message = Some(err),
                },
                SupabaseEvent::ConversationRetitled {
                    conversation_id,
                    title,
                } => {
                    if let Some(conversation) =
                        self.conversations.iter_mut().find(|item| item.id == conversation_id)
                    {
                        conversation.title = title;
                        conversation.updated_at = chrono::Utc::now();
                        self.sort_conversations_desc();
                    }
                }
                SupabaseEvent::ProfileLoaded(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(profile) => {
                            let avatar_url = profile.avatar_url.clone();
                            let display_name = profile.display_name.clone();
                            let theme = profile.theme.clone();
                            self.profile = Some(profile);

                            if let Some(name) = display_name.filter(|name| !name.trim().is_empty()) {
                                self.settings_name_draft = name.clone();
                                self.user_full_name = Some(name);
                            }

                            self.theme_preference = ThemePreference::from_profile(theme.as_deref());

                            if let Some(url) = avatar_url.filter(|url| !url.trim().is_empty()) {
                                self.request_download_avatar(url);
                            } else {
                                self.avatar_texture = None;
                            }
                        }
                        Err(err) => self.error_message = Some(err),
                    }
                }
                SupabaseEvent::DisplayNameUpdated(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(profile) => {
                            if let Some(name) = profile.display_name.clone().filter(|name| !name.trim().is_empty()) {
                                self.user_full_name = Some(name.clone());
                                self.settings_name_draft = name;
                            }

                            self.profile = Some(profile);
                            self.settings_name_status = Some(SettingsStatus::success_for(
                                "Saved",
                                Duration::from_secs(2),
                            ));
                        }
                        Err(err) => self.settings_name_status = Some(SettingsStatus::error(err)),
                    }
                }
                SupabaseEvent::ThemeUpdated(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(profile) => {
                            self.theme_preference = ThemePreference::from_profile(profile.theme.as_deref());
                            self.profile = Some(profile);
                            self.settings_theme_status = Some(SettingsStatus::success("Theme saved."));
                        }
                        Err(err) => self.settings_theme_status = Some(SettingsStatus::error(err)),
                    }
                }
                SupabaseEvent::EmailUpdated(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(email) => {
                            self.user_email = Some(email.clone());
                            self.settings_email_draft = email;
                            self.settings_email_status = Some(SettingsStatus::success(
                                "Email update requested. Check your inbox to confirm.",
                            ));
                        }
                        Err(err) => self.settings_email_status = Some(SettingsStatus::error(err)),
                    }
                }
                SupabaseEvent::PasswordUpdated(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(()) => {
                            self.settings_password_draft.clear();
                            self.settings_password_confirm_draft.clear();
                            self.settings_password_status = Some(SettingsStatus::success(
                                "Password changed successfully.",
                            ));
                        }
                        Err(err) => {
                            self.settings_password_status = Some(SettingsStatus::error(err))
                        }
                    }
                }
                SupabaseEvent::AvatarUploaded(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(url) => self.request_update_avatar_url(url),
                        Err(err) => self.settings_photo_status = Some(SettingsStatus::error(err)),
                    }
                }
                SupabaseEvent::AvatarUrlUpdated(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(profile) => {
                            let avatar_url = profile.avatar_url.clone();
                            self.profile = Some(profile);
                            if let Some(url) = avatar_url.filter(|url| !url.trim().is_empty()) {
                                self.request_download_avatar(url);
                            }
                            self.settings_photo_status = Some(SettingsStatus::success("Photo updated."));
                        }
                        Err(err) => self.settings_photo_status = Some(SettingsStatus::error(err)),
                    }
                }
                SupabaseEvent::AvatarDownloaded(result) => match result {
                    Ok((url, bytes)) => {
                        let current_avatar = self
                            .profile
                            .as_ref()
                            .and_then(|profile| profile.avatar_url.as_deref())
                            .map(str::to_owned);

                        if current_avatar.as_deref() == Some(url.as_str()) {
                            if let Ok(color_image) = Self::decode_avatar_image(&bytes) {
                                let texture = ctx.load_texture(
                                    format!("avatar-{}", url),
                                    color_image,
                                    egui::TextureOptions::LINEAR,
                                );
                                self.avatar_texture = Some(texture);
                            }
                        }
                    }
                    Err(err) => self.settings_photo_status = Some(SettingsStatus::error(err)),
                },
                SupabaseEvent::AccountDeleted(result) => {
                    self.profile_loading = false;
                    match result {
                        Ok(()) => {
                            self.logout();
                            self.auth_notice = Some("Account deleted.".to_owned());
                        }
                        Err(err) => self.settings_account_status = Some(SettingsStatus::error(err)),
                    }
                }
            }
        }
    }

    pub(crate) fn save_profile_name(&mut self) {
        let new_name = self.settings_name_draft.trim().to_owned();
        let current_name = self
            .profile
            .as_ref()
            .and_then(|profile| profile.display_name.clone())
            .unwrap_or_default();

        if new_name.is_empty() {
            self.settings_name_status = Some(SettingsStatus::error("Display name cannot be empty."));
            return;
        }

        if new_name == current_name.trim() {
            return;
        }

        self.settings_name_status = None;
        self.request_update_display_name(new_name);
    }

    pub(crate) fn save_theme_preference(&mut self) {
        self.settings_theme_status = None;
        self.request_update_theme(self.theme_preference);
    }

    pub(crate) fn change_account_email(&mut self) {
        let email = self.settings_email_draft.trim().to_owned();
        if email.is_empty() || !email.contains('@') || !email.contains('.') {
            self.settings_email_status = Some(SettingsStatus::error("Enter a valid email address."));
            return;
        }

        let current_email = self.user_email.clone().unwrap_or_default();
        if email == current_email.trim() {
            return;
        }

        self.settings_email_status = None;
        self.request_update_email(email);
    }

    pub(crate) fn change_account_password(&mut self) {
        let password = self.settings_password_draft.clone();
        if password.trim().len() < 8 {
            self.settings_password_status =
                Some(SettingsStatus::error("Password must be at least 8 characters."));
            return;
        }

        if password != self.settings_password_confirm_draft {
            self.settings_password_status = Some(SettingsStatus::error("Passwords do not match."));
            return;
        }

        self.settings_password_status = None;
        self.request_update_password(password);
    }

    pub(crate) fn pick_and_upload_avatar(&mut self) {
        let file = rfd::FileDialog::new()
            .add_filter("image", &["png", "jpg", "jpeg"])
            .pick_file();

        let Some(file) = file else {
            return;
        };

        let bytes = match std::fs::read(&file) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.settings_photo_status =
                    Some(SettingsStatus::error(format!("Failed to read selected file: {err}")));
                return;
            }
        };

        let mime = file
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .map(|ext| {
                if ext == "png" {
                    "image/png".to_owned()
                } else {
                    "image/jpeg".to_owned()
                }
            })
            .unwrap_or_else(|| "image/jpeg".to_owned());

        self.settings_photo_status = Some(SettingsStatus::success("Uploading photo..."));
        self.request_upload_avatar(bytes, mime);
    }

    pub(crate) fn profile_initials(&self) -> String {
        self.user_email
            .as_deref()
            .and_then(|email| email.chars().next())
            .map(|ch| ch.to_ascii_uppercase().to_string())
            .unwrap_or_else(|| "U".to_owned())
    }

    pub(crate) fn render_profile_avatar(
        &self,
        ui: &mut egui::Ui,
        initials: &str,
        radius: f32,
        clickable: bool,
    ) -> egui::Response {
        let size = egui::vec2(radius * 2.0, radius * 2.0);
        let (rect, response) = ui.allocate_exact_size(
            size,
            if clickable {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );
        let painter = ui.painter();

        if let Some(texture) = &self.avatar_texture {
            let image = egui::Image::new((texture.id(), size))
                .fit_to_exact_size(size)
                .corner_radius(egui::CornerRadius::same(radius.round() as u8));
            image.paint_at(ui, rect);
            painter.circle_stroke(
                rect.center(),
                radius,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(24, 44, 78)),
            );
            return response;
        }

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
        response
    }

    pub(crate) fn login_with_email(&mut self) {
        let email = self.auth_email.trim().to_owned();
        let password = self.auth_password.clone();

        if email.is_empty() || password.is_empty() {
            self.auth_notice = Some("Email and password are required.".to_owned());
            return;
        }

        let response = match crate::api::supabase::signin_with_password(&email, &password) {
            Ok(resp) => resp,
            Err(err) => {
                self.auth_notice = Some(err);
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
        self.settings_password_confirm_draft.clear();
        self.delete_account_confirmation.clear();
        self.theme_preference = ThemePreference::System;
        self.profile = None;
        self.avatar_texture = None;
        self.profile_loading = false;
        self.profile_menu_open = false;
        self.profile_menu_anchor = None;
        self.show_settings = false;
        self.settings_name_status = None;
        self.settings_email_status = None;
        self.settings_password_status = None;
        self.settings_photo_status = None;
        self.settings_theme_status = None;
        self.settings_account_status = None;
        self.conversations.clear();
        self.active_conversation_id = None;
        self.messages.clear();
        self.messages_loading = false;
        if user_id.is_some() {
            self.request_list_conversations();
            self.request_load_profile();
        }
        self.auth_password.clear();
        self.auth_notice = Some(format!("Logged in as {user_email}"));
    }

    pub(crate) fn signup_with_email(&mut self) {
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

        let endpoint = format!("{}/signup", crate::env::supabase_auth_url());
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
            .header("apikey", crate::env::SUPABASE_PUBLISHABLE_KEY)
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
            self.settings_password_confirm_draft.clear();
            self.delete_account_confirmation.clear();
            self.theme_preference = ThemePreference::System;
            self.profile = None;
            self.avatar_texture = None;
            self.profile_loading = false;
            self.profile_menu_open = false;
            self.profile_menu_anchor = None;
            self.show_settings = false;
            self.settings_name_status = None;
            self.settings_email_status = None;
            self.settings_password_status = None;
            self.settings_photo_status = None;
            self.settings_theme_status = None;
            self.settings_account_status = None;
            self.conversations.clear();
            self.active_conversation_id = None;
            self.messages.clear();
            self.messages_loading = false;
            self.creating_new_chat = true;
            self.auth_password.clear();
            self.auth_confirm_password.clear();
            self.request_list_conversations();
            self.request_load_profile();
            self.auth_notice = Some(format!("Signup successful. Logged in as {user_email}"));
        } else {
            self.auth_notice = Some(
                "Signup successful. Please verify your email, then log in.".to_owned(),
            );
        }
    }

    pub(crate) fn logout(&mut self) {
        let secret_store = SecretStore::new("AuvroAI");
        let _ = secret_store.delete("SUPABASE_ACCESS_TOKEN");
        self.is_authenticated = false;
        self.session_access_token = None;
        self.user_id = None;
        self.user_email = None;
        self.user_full_name = None;
        self.conversations.clear();
        self.active_conversation_id = None;
        self.messages.clear();
        self.messages_loading = false;
        self.profile = None;
        self.avatar_texture = None;
        self.profile_loading = false;
        self.pending_title_generation = None;
        self.pending_delete_conversation_id = None;
        self.creating_new_chat = false;
        self.profile_menu_open = false;
        self.profile_menu_anchor = None;
        self.show_settings = false;
        self.settings_name_status = None;
        self.settings_email_status = None;
        self.settings_password_status = None;
        self.settings_photo_status = None;
        self.settings_theme_status = None;
        self.settings_account_status = None;
        self.settings_name_draft.clear();
        self.settings_email_draft.clear();
        self.settings_password_draft.clear();
        self.settings_password_confirm_draft.clear();
        self.delete_account_confirmation.clear();
        self.auth_password.clear();
        self.auth_confirm_password.clear();
        self.auth_notice = Some("Logged out.".to_owned());
    }

    fn as_conversation_lines(messages: &[Message]) -> Vec<String> {
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

    fn normalize_ai_text(text: &str) -> String {
        text.chars()
            .map(|ch| match ch {
                '\u{00A0}' | '\u{2007}' | '\u{202F}' => ' ',
                '\u{2060}' | '\u{FEFF}' => ' ',
                _ => ch,
            })
            .collect()
    }

    pub(crate) fn sidebar_title(title: &str) -> String {
        const MAX_CHARS: usize = 28;
        if title.chars().count() <= MAX_CHARS {
            return title.to_owned();
        }
        let trimmed: String = title.chars().take(MAX_CHARS.saturating_sub(3)).collect();
        format!("{trimmed}...")
    }

    fn apply_app_theme(&self, ctx: &egui::Context) {
        let emoji_fonts_id = egui::Id::new("emoji_fonts_loaded");
        let emoji_fonts_loaded = ctx.data(|data| data.get_temp::<bool>(emoji_fonts_id).unwrap_or(false));
        if !emoji_fonts_loaded {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "noto_color_emoji".to_owned(),
                egui::FontData::from_static(include_bytes!("../assets/NotoColorEmoji-Regular.ttf")).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("noto_color_emoji".to_owned());
            ctx.set_fonts(fonts);
            ctx.data_mut(|data| data.insert_temp(emoji_fonts_id, true));
        }

        let dark_mode = match self.theme_preference {
            ThemePreference::Dark => true,
            ThemePreference::Light => false,
            ThemePreference::System => matches!(ctx.system_theme(), Some(egui::Theme::Dark)),
        };

        let mut visuals = if dark_mode {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(10, 13, 18);
            visuals.window_fill = egui::Color32::from_rgb(14, 18, 24);
            visuals.faint_bg_color = egui::Color32::from_rgba_premultiplied(255, 255, 255, 6);
            visuals.extreme_bg_color = egui::Color32::from_rgb(6, 8, 11);
            visuals.override_text_color = Some(egui::Color32::from_rgb(229, 232, 238));
            visuals.selection.bg_fill = egui::Color32::from_rgb(66, 78, 104);
            visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(98, 116, 152));
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgba_premultiplied(255, 255, 255, 0);
            visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgba_premultiplied(255, 255, 255, 0);
            visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 18));
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgba_premultiplied(18, 22, 29, 220);
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgba_premultiplied(18, 22, 29, 220);
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 18));
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgba_premultiplied(26, 31, 41, 230);
            visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgba_premultiplied(26, 31, 41, 230);
            visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 28));
            visuals.widgets.active.bg_fill = egui::Color32::from_rgba_premultiplied(34, 40, 52, 240);
            visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgba_premultiplied(34, 40, 52, 240);
            visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 36));
            visuals
        } else {
            let mut visuals = egui::Visuals::light();
            visuals.panel_fill = egui::Color32::from_rgb(244, 246, 249);
            visuals.window_fill = egui::Color32::from_rgb(250, 251, 253);
            visuals.override_text_color = Some(egui::Color32::from_rgb(28, 31, 35));
            visuals.selection.bg_fill = egui::Color32::from_rgb(208, 219, 231);
            visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(114, 126, 142));
            visuals
        };

        visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
        visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);
        visuals.widgets.open.corner_radius = egui::CornerRadius::same(8);

        ctx.set_visuals(visuals);

        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(12.0, 12.0);
            style.spacing.button_padding = egui::vec2(12.0, 8.0);
            style.spacing.indent = 16.0;
            style.spacing.interact_size = egui::vec2(44.0, 30.0);
            style.text_styles = [
                (egui::TextStyle::Heading, egui::FontId::proportional(26.0)),
                (egui::TextStyle::Name("PanelTitle".into()), egui::FontId::proportional(20.0)),
                (egui::TextStyle::Body, egui::FontId::proportional(16.0)),
                (egui::TextStyle::Button, egui::FontId::proportional(15.0)),
                (egui::TextStyle::Monospace, egui::FontId::monospace(15.0)),
                (egui::TextStyle::Small, egui::FontId::proportional(13.0)),
            ]
            .into();
        });
    }

    pub(crate) fn start_new_chat(&mut self) {
        self.error_message = None;
        self.show_settings = false;
        self.profile_menu_open = false;
        self.profile_menu_anchor = None;
        self.creating_new_chat = true;
        self.active_conversation_id = None;
        self.messages.clear();
        self.messages_loading = false;
        self.pending_title_generation = None;
        self.pending_delete_conversation_id = None;
        self.request_create_conversation("New Chat");
    }

    pub(crate) fn select_conversation(&mut self, conversation_id: Uuid) {
        self.show_settings = false;
        self.profile_menu_open = false;
        self.profile_menu_anchor = None;
        self.active_conversation_id = Some(conversation_id);
        self.creating_new_chat = false;
        self.messages.clear();
        self.messages_loading = true;
        self.pending_title_generation = None;
        self.request_load_messages(conversation_id);
    }

    pub(crate) fn send_message(&mut self) {
        if self.is_loading {
            return;
        }

        let prompt = self.draft_message.trim().to_owned();
        if prompt.is_empty() {
            return;
        }

        self.draft_message.clear();
        self.error_message = None;

        let Some(conversation_id) = self.active_conversation_id else {
            self.pending_prompt_after_create = Some(prompt);
            if !self.creating_new_chat {
                self.start_new_chat();
            }
            return;
        };

        self.send_prompt_to_conversation(conversation_id, prompt);
    }

    fn send_prompt_to_conversation(&mut self, conversation_id: Uuid, prompt: String) {
        if self.is_loading {
            return;
        }

        self.messages.push(Message {
            id: Uuid::new_v4(),
            conversation_id,
            role: "user".to_owned(),
            content: prompt.clone(),
            created_at: chrono::Utc::now(),
        });

        let should_generate_title = self.messages.len() == 1
            && self
                .conversations
                .iter()
                .find(|conversation| conversation.id == conversation_id)
                .map(|conversation| conversation.title.trim() == "New Chat")
                .unwrap_or(false);
        if should_generate_title {
            self.pending_title_generation = Some(PendingTitleGeneration {
                conversation_id,
                first_user_message: prompt.clone(),
            });
        }
        self.request_append_message(conversation_id, "user", prompt.clone());
        self.request_bump_conversation_updated_at(conversation_id);

        self.messages.push(Message {
            id: Uuid::new_v4(),
            conversation_id,
            role: "assistant".to_owned(),
            content: String::new(),
            created_at: chrono::Utc::now(),
        });

        let conversation = Self::as_conversation_lines(&self.messages);
        let provider = Arc::clone(&self.provider);
        let prompt_for_task = prompt;
        let conversation_for_task = conversation;
        let cancellation_token = CancellationToken::new();
        let task_cancellation_token = cancellation_token.clone();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = provider.generate_reply_cancelable(
                &prompt_for_task,
                &conversation_for_task,
                &task_cancellation_token,
            );
            let _ = tx.send(result);
        });

        self.provider_response_rx = Some(rx);
        self.stream_cancellation_token = Some(cancellation_token);
        self.pending_response = None;
        self.streaming_buffer.clear();
        self.streamed_chars = 0;
        self.stream_conversation_id = Some(conversation_id);
        self.stream_line_index = Some(self.messages.len().saturating_sub(1));
        self.is_loading = true;
        self.last_stream_tick = Instant::now();
    }

    pub(crate) fn stop_streaming(&mut self) {
        if let Some(cancellation_token) = self.stream_cancellation_token.take() {
            cancellation_token.cancel();
        }

        let conversation_id = self.stream_conversation_id;
        let partial = self.streaming_buffer.trim().to_owned();

        if let Some(line_idx) = self.stream_line_index {
            if partial.is_empty() {
                if self
                    .messages
                    .get(line_idx)
                    .is_some_and(|message| message.role == "assistant" && message.content.trim().is_empty())
                {
                    self.messages.remove(line_idx);
                }
            } else if let Some(message) = self.messages.get_mut(line_idx) {
                message.content = partial.clone();
            }
        }

        if !partial.is_empty() {
            if let Some(conversation_id) = conversation_id {
                self.request_append_message(conversation_id, "assistant", partial);
                self.request_bump_conversation_updated_at(conversation_id);
            }
        }

        self.is_loading = false;
        self.provider_response_rx = None;
        self.pending_response = None;
        self.streaming_buffer.clear();
        self.streamed_chars = 0;
        self.stream_conversation_id = None;
        self.stream_line_index = None;
        self.last_stream_tick = Instant::now();
    }

    fn tick_streaming(&mut self, ctx: &egui::Context) {
        if !self.is_loading {
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(16));

        if self.pending_response.is_none() {
            if let Some(rx) = &self.provider_response_rx {
                match rx.try_recv() {
                    Ok(Ok(full_response)) => {
                        self.pending_response = Some(Self::normalize_ai_text(&full_response));
                        self.provider_response_rx = None;
                        self.stream_cancellation_token = None;
                        self.last_stream_tick = Instant::now();
                    }
                    Ok(Err(err)) => {
                        if let Some(line_idx) = self.stream_line_index {
                            if self
                                .messages
                                .get(line_idx)
                                .is_some_and(|message| message.role == "assistant" && message.content.trim().is_empty())
                            {
                                self.messages.remove(line_idx);
                            }
                        }

                        self.is_loading = false;
                        self.provider_response_rx = None;
                        self.stream_cancellation_token = None;
                        self.pending_response = None;
                        self.streaming_buffer.clear();
                        self.streamed_chars = 0;
                        self.stream_conversation_id = None;
                        self.stream_line_index = None;

                        if !err.to_ascii_lowercase().contains("cancelled") {
                            self.error_message = Some(format!("Provider error: {err}"));
                        }
                        return;
                    }
                    Err(mpsc::TryRecvError::Empty) => return,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.is_loading = false;
                        self.provider_response_rx = None;
                        self.stream_cancellation_token = None;
                        self.pending_response = None;
                        self.streaming_buffer.clear();
                        self.streamed_chars = 0;
                        self.stream_conversation_id = None;
                        self.stream_line_index = None;
                        self.error_message = Some("Provider task disconnected unexpectedly.".to_owned());
                        return;
                    }
                }
            }
        }

        if self.last_stream_tick.elapsed() < Duration::from_millis(24) {
            return;
        }
        self.last_stream_tick = Instant::now();

        let Some(response) = self.pending_response.as_ref() else {
            self.is_loading = false;
            return;
        };

        let chars_len = response.chars().count();
        let next_chunk: String = response
            .chars()
            .skip(self.streamed_chars)
            .take(3)
            .collect();
        self.streamed_chars = (self.streamed_chars + next_chunk.chars().count()).min(chars_len);
        self.streaming_buffer.push_str(&next_chunk);

        if let Some(line_idx) = self.stream_line_index {
            if let Some(message) = self.messages.get_mut(line_idx) {
                message.content = self.streaming_buffer.clone();
            }
        }

        if self.streamed_chars >= chars_len {
            self.is_loading = false;
            self.pending_response = None;
            self.provider_response_rx = None;
            self.stream_cancellation_token = None;

            if let (Some(conversation_id), Some(line_idx)) =
                (self.stream_conversation_id.take(), self.stream_line_index.take())
            {
                if let Some(message) = self.messages.get(line_idx) {
                    self.request_append_message(
                        conversation_id,
                        "assistant",
                        message.content.clone(),
                    );
                    self.request_bump_conversation_updated_at(conversation_id);
                }
            }
        }
    }

}

impl eframe::App for AuvroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.expire_settings_statuses();
        self.process_supabase_events(ctx);
        self.tick_streaming(ctx);

        self.apply_app_theme(ctx);

        let window_width = ctx.input(|i| i.content_rect().width());
        let compact_layout = window_width < 980.0;

        egui::TopBottomPanel::top("app_header")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_premultiplied(255, 255, 255, 18),
                    ))
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.horizontal(|ui| {
                        ui::render_app_logo(ui, 32.0);
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("AuvroAI").strong().size(18.0));
                            ui.label(
                                egui::RichText::new("Minimal, centered chat workspace")
                                    .small()
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.is_authenticated {
                            let email = self
                                .user_email
                                .as_deref()
                                .unwrap_or("authenticated user");
                            ui.label(
                                egui::RichText::new(format!("Signed in as {email}"))
                                    .small()
                                    .color(ui.visuals().weak_text_color()),
                            );
                        }
                    });
                });
            });

        if !self.is_authenticated {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui::auth::render(self, ui);
            });
            return;
        }

        if self.show_sidebar {
            egui::SidePanel::left("session_sidebar")
                .resizable(true)
                .default_width(if compact_layout { 260.0 } else { 240.0 })
                .show(ctx, |ui| {
                    ui::sidebar::render_sessions(self, ui, ctx);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_settings {
                ui::settings::render_settings_screen(self, ui);
            } else {
                ui::chat::render_chat_panel(self, ui);
            }
        });

        ui::sidebar::render_delete_confirmation(self, ctx);
    }
}

fn main() -> Result<(), eframe::Error> {
    dotenvy::dotenv().ok();
    let _ = load_environment();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 680.0])
            .with_min_inner_size([360.0, 500.0])
            .with_icon(load_app_icon()),
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

fn load_app_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/icons/Auvro.png"))
        .expect("Failed to decode Auvro.png for app icon")
        .to_rgba8();
    let (width, height) = image.dimensions();

    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}
