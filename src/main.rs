mod secrets;
mod provider;

use eframe::egui;
use provider::{create_default_provider, Provider};
use secrets::SecretStore;
use std::time::{Duration, Instant};

struct ChatSession {
    name: String,
    messages: Vec<String>,
}

struct AuvroApp {
    provider: Box<dyn Provider>,
    draft_message: String,
    sessions: Vec<ChatSession>,
    selected_session: usize,
    renaming_session: bool,
    auth_mode: AuthMode,
    auth_full_name: String,
    auth_email: String,
    auth_password: String,
    auth_confirm_password: String,
    auth_notice: Option<String>,
    is_authenticated: bool,
    user_email: Option<String>,
    supabase_url: String,
    supabase_publishable_key: String,
    auth_error: Option<String>,
    is_loading: bool,
    error_message: Option<String>,
    pending_response: Option<Vec<char>>,
    streamed_chars: usize,
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

        let (is_authenticated, user_email, auth_notice) =
            if auth_error.is_none() && !supabase_url.is_empty() && !supabase_publishable_key.is_empty() {
                Self::restore_auth_session(&supabase_url, &supabase_publishable_key)
            } else {
                (false, None, None)
            };

        Self {
            provider: create_default_provider(),
            draft_message: String::new(),
            sessions: vec![ChatSession {
                name: "General".to_owned(),
                messages: vec!["Auvro: Welcome to Auvro AI. How can I help you today?".to_owned()],
            }],
            selected_session: 0,
            renaming_session: false,
            auth_mode: AuthMode::Login,
            auth_full_name: String::new(),
            auth_email: String::new(),
            auth_password: String::new(),
            auth_confirm_password: String::new(),
            auth_notice,
            is_authenticated,
            user_email,
            supabase_url,
            supabase_publishable_key,
            auth_error,
            is_loading: false,
            error_message: None,
            pending_response: None,
            streamed_chars: 0,
            stream_line_index: None,
            last_stream_tick: Instant::now(),
        }
    }
}

impl AuvroApp {
    fn restore_auth_session(url: &str, key: &str) -> (bool, Option<String>, Option<String>) {
        let secret_store = SecretStore::new("AuvroAI");
        let Ok(token) = secret_store.get("SUPABASE_ACCESS_TOKEN") else {
            return (false, None, None);
        };

        match Self::fetch_user_email(url, key, &token) {
            Ok(email) => (
                true,
                Some(email),
                Some("Session restored. Redirecting to chat.".to_owned()),
            ),
            Err(_) => {
                let _ = secret_store.delete("SUPABASE_ACCESS_TOKEN");
                (false, None, Some("Previous session expired. Please log in again.".to_owned()))
            }
        }
    }

    fn fetch_user_email(url: &str, key: &str, access_token: &str) -> Result<String, String> {
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
        body.get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| "User email missing in Supabase response".to_owned())
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

        let secret_store = SecretStore::new("AuvroAI");
        let _ = secret_store.set("SUPABASE_ACCESS_TOKEN", &access_token);

        self.is_authenticated = true;
        self.user_email = Some(user_email.clone());
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
            let secret_store = SecretStore::new("AuvroAI");
            let _ = secret_store.set("SUPABASE_ACCESS_TOKEN", &access_token);

            self.is_authenticated = true;
            self.user_email = Some(user_email.clone());
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
        self.user_email = None;
        self.auth_password.clear();
        self.auth_confirm_password.clear();
        self.auth_notice = Some("Logged out.".to_owned());
    }

    fn render_auth_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.heading("Welcome to AuvroAI");
            ui.label("Log in or sign up with your Supabase email account.");
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
                        if ui
                            .add_enabled(auth_ready, egui::Button::new("Log In"))
                            .clicked()
                        {
                            self.login_with_email();
                        }
                    }
                    AuthMode::SignUp => {
                        if ui
                            .add_enabled(auth_ready, egui::Button::new("Sign Up"))
                            .clicked()
                        {
                            self.signup_with_email();
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

    fn active_session(&self) -> &ChatSession {
        &self.sessions[self.selected_session]
    }

    fn active_session_mut(&mut self) -> &mut ChatSession {
        &mut self.sessions[self.selected_session]
    }

    fn new_session_name(&self) -> String {
        format!("Session {}", self.sessions.len() + 1)
    }

    fn send_message(&mut self) {
        if self.is_loading {
            return;
        }

        let prompt = self.draft_message.trim().to_owned();
        if prompt.is_empty() {
            return;
        }

        self.active_session_mut()
            .messages
            .push(format!("You: {prompt}"));
        self.draft_message.clear();
        self.error_message = None;

        if prompt.eq_ignore_ascii_case("/error") {
            self.error_message = Some("Simulated request failure. Try sending again.".to_owned());
            return;
        }

        let conversation = self.active_session().messages.clone();
        let full_response = match self.provider.generate_reply(&prompt, &conversation) {
            Ok(reply) => reply,
            Err(err) => {
                self.error_message = Some(format!("Provider error: {err}"));
                return;
            }
        };
        self.pending_response = Some(full_response.chars().collect());
        self.streamed_chars = 0;
        self.active_session_mut().messages.push("Auvro: ".to_owned());
        self.stream_line_index = Some(self.active_session().messages.len() - 1);
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

        if let Some(line_idx) = self.stream_line_index {
            self.active_session_mut().messages[line_idx] = format!("Auvro: {streamed_text}");
        }

        if self.streamed_chars >= chars_len {
            self.is_loading = false;
            self.pending_response = None;
            self.stream_line_index = None;
        }
    }

    fn render_sessions(&mut self, ui: &mut egui::Ui) {
        ui.heading("Chats");
        ui.add_space(8.0);

        for (idx, session) in self.sessions.iter_mut().enumerate() {
            let selected = self.selected_session == idx;
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if ui.selectable_label(selected, &session.name).clicked() {
                        self.selected_session = idx;
                    }

                    if selected {
                        ui.small("active");
                    }
                });

                if selected && self.renaming_session {
                    ui.add_space(4.0);
                    ui.label("Rename chat");
                    ui.text_edit_singleline(&mut session.name);
                    if ui.button("Done").clicked() {
                        self.renaming_session = false;
                    }
                }
            });

            ui.add_space(6.0);
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("+ New Chat").clicked() {
                self.sessions.push(ChatSession {
                    name: self.new_session_name(),
                    messages: vec!["Auvro: New chat started. What would you like to discuss?"
                        .to_owned()],
                });
                self.selected_session = self.sessions.len() - 1;
                self.renaming_session = false;
            }

            if ui
                .add_enabled(self.sessions.len() > 1, egui::Button::new("Delete"))
                .clicked()
            {
                self.sessions.remove(self.selected_session);
                if self.selected_session >= self.sessions.len() {
                    self.selected_session = self.sessions.len() - 1;
                }
                self.renaming_session = false;
            }
        });

        ui.add_space(4.0);
        if ui.button("Rename").clicked() {
            self.renaming_session = true;
        }
    }

    fn render_chat_panel(&mut self, ui: &mut egui::Ui) {
        let session_name = self.active_session().name.as_str();

        ui.heading(format!("Chat - {session_name}"));
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .id_salt("chat_scroll")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.active_session().messages {
                    ui.label(line);
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

        let (enter_pressed, shift_held) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.modifiers.shift));
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
                    if ui.button("Log Out").clicked() {
                        self.logout();
                    }
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
                        .sessions
                        .get(self.selected_session)
                        .map_or("General", |s| s.name.as_str());

                    egui::ComboBox::from_id_salt("session_selector")
                        .selected_text(selected_name)
                        .show_ui(ui, |ui| {
                            for (idx, session) in self.sessions.iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_session == idx, &session.name)
                                    .clicked()
                                {
                                    self.selected_session = idx;
                                }
                            }
                        });

                    if ui.button("+ New").clicked() {
                        self.sessions.push(ChatSession {
                            name: self.new_session_name(),
                            messages: vec!["Auvro: New chat started. What would you like to discuss?"
                                .to_owned()],
                        });
                        self.selected_session = self.sessions.len() - 1;
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
