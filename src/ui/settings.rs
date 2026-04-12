use crate::cache::model_metadata::CacheState;
use eframe::egui;

pub(crate) fn render_account_menu(app: &mut crate::AppState, ctx: &egui::Context) {
    if !app.profile_menu_open {
        return;
    }

    egui::Window::new("Account")
        .collapsible(false)
        .resizable(false)
        .default_width(260.0)
        .show(ctx, |ui| {
            ui.label(app.user_full_name.as_deref().unwrap_or("AuvroAI User"));
            if let Some(email) = &app.user_email {
                ui.small(email);
            }
            ui.separator();

            if ui.button("Settings").clicked() {
                app.settings_open = true;
                app.profile_menu_open = false;
                app.settings_notice = None;
            }

            if ui.button("Log Out").clicked() {
                app.profile_menu_open = false;
                app.logout();
            }

            if ui.button("Close").clicked() {
                app.profile_menu_open = false;
            }
        });
}

pub(crate) fn render_settings_window(app: &mut crate::AppState, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }

    let initials = app.profile_initials();
    egui::Window::new("Settings")
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                crate::AuvroApp::render_profile_avatar(ui, &initials, 28.0);
                ui.vertical(|ui| {
                    ui.label(app.user_full_name.as_deref().unwrap_or("AuvroAI User"));
                    if let Some(email) = &app.user_email {
                        ui.small(email);
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            ui.label("Name");
            ui.add_sized(
                [460.0, 30.0],
                egui::TextEdit::singleline(&mut app.settings_name_draft).hint_text("Your name"),
            );
            if ui.button("Save Name").clicked() {
                app.save_profile_name();
            }

            ui.add_space(10.0);
            ui.label("Email");
            ui.add_sized(
                [460.0, 30.0],
                egui::TextEdit::singleline(&mut app.settings_email_draft)
                    .hint_text("you@example.com"),
            );
            if ui.button("Change Email").clicked() {
                app.change_account_email();
            }

            ui.add_space(10.0);
            ui.label("New Password");
            ui.add_sized(
                [460.0, 30.0],
                egui::TextEdit::singleline(&mut app.settings_password_draft)
                    .password(true)
                    .hint_text("Enter new password"),
            );
            if ui.button("Change Password").clicked() {
                app.change_account_password();
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label("Model");
            let _ = app.model_cache.ensure_loaded(crate::env::OPENROUTER_API_KEY);

            match app.model_cache.state_snapshot() {
                CacheState::Empty | CacheState::Loading => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading available models...");
                    });
                }
                CacheState::Failed(err) => {
                    ui.colored_label(egui::Color32::from_rgb(255, 99, 99), err);
                    if ui.button("Retry").clicked() {
                        app.model_cache.invalidate();
                        let _ = app.model_cache.ensure_loaded(crate::env::OPENROUTER_API_KEY);
                    }
                }
                CacheState::Loaded { .. } => {
                    if let Some(models) = app.model_cache.get_models() {
                        let selected_text = if app.selected_model_id.trim().is_empty() {
                            "Select a model".to_owned()
                        } else {
                            app.selected_model_id.clone()
                        };

                        egui::ComboBox::from_id_salt("settings_model_selector")
                            .selected_text(selected_text)
                            .width(460.0)
                            .show_ui(ui, |ui| {
                                for model in &models {
                                    let label = format!(
                                        "{} ({}) | ctx={} | p={} c={}",
                                        model.id,
                                        model.name,
                                        model.context_length,
                                        model.prompt_price_per_1k,
                                        model.completion_price_per_1k
                                    );
                                    if ui
                                        .selectable_label(app.selected_model_id == model.id, label)
                                        .clicked()
                                    {
                                        app.selected_model_id = model.id.clone();
                                        match app.save_selected_model_id() {
                                            Ok(()) => {
                                                app.settings_notice =
                                                    Some("Selected model saved to Supabase.".to_owned());
                                            }
                                            Err(err) => app.settings_notice = Some(err),
                                        }
                                    }
                                }
                            });
                    } else {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Refreshing models...");
                        });
                    }
                }
            }

            if let Some(notice) = &app.settings_notice {
                ui.add_space(8.0);
                let lower = notice.to_ascii_lowercase();
                let is_error = lower.contains("failed")
                    || lower.contains("missing")
                    || lower.contains("valid")
                    || lower.contains("least")
                    || lower.contains("error")
                    || lower.contains("invalid");
                let color = if is_error {
                    egui::Color32::from_rgb(255, 99, 99)
                } else {
                    egui::Color32::from_rgb(48, 146, 85)
                };
                ui.colored_label(color, notice);
            }

            ui.add_space(10.0);
            if ui.button("Close Settings").clicked() {
                app.settings_open = false;
            }
        });
}
