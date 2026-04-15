use eframe::egui;

fn render_status(ui: &mut egui::Ui, status: &Option<crate::SettingsStatus>) {
    if let Some(status) = status {
        let color = if status.is_error {
            egui::Color32::from_rgb(255, 99, 99)
        } else {
            egui::Color32::from_rgb(48, 146, 85)
        };
        ui.colored_label(color, &status.message);
    }
}

pub(crate) fn render_settings_screen(app: &mut crate::AppState, ui: &mut egui::Ui) {
    let _ = app
        .model_cache
        .ensure_loaded(crate::env::OPENROUTER_API_KEY);

    let initials = app.profile_initials();
    let profile_display_name = app
        .profile
        .as_ref()
        .and_then(|profile| profile.display_name.clone())
        .unwrap_or_default();
    let profile_theme = crate::ThemePreference::from_profile(
        app.profile
            .as_ref()
            .and_then(|profile| profile.theme.as_deref()),
    );
    let current_email = app.user_email.clone().unwrap_or_default();

    let name_changed = app.settings_name_draft.trim() != profile_display_name.trim();
    let can_save_name = !app.profile_loading && name_changed && !app.settings_name_draft.trim().is_empty();

    let email_trimmed = app.settings_email_draft.trim();
    let email_changed = email_trimmed != current_email.trim();
    let email_valid = email_trimmed.contains('@') && email_trimmed.contains('.');
    let can_save_email = !app.profile_loading && email_changed && email_valid;

    let password_trimmed = app.settings_password_draft.trim();
    let password_confirm_trimmed = app.settings_password_confirm_draft.trim();
    let can_save_password = !app.profile_loading
        && !password_trimmed.is_empty()
        && !password_confirm_trimmed.is_empty()
        && app.settings_password_draft == app.settings_password_confirm_draft
        && password_trimmed.len() >= 8;

    ui.horizontal(|ui| {
        if ui.button("<- Back").clicked() {
            app.show_settings = false;
            app.profile_menu_open = false;
            app.profile_menu_anchor = None;
        }
        ui.heading("Settings");
    });
    ui.add_space(8.0);

    if app.profile_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Saving settings...");
        });
        ui.add_space(8.0);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.group(|ui| {
            ui.heading("Profile");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                app.render_profile_avatar(ui, &initials, 32.0, false);
                ui.vertical(|ui| {
                    ui.label(app.user_full_name.as_deref().unwrap_or("AuvroAI User"));
                    if let Some(email) = &app.user_email {
                        ui.small(email);
                    }
                    ui.add_space(6.0);
                    if ui
                        .add_enabled(!app.profile_loading, egui::Button::new("Change photo"))
                        .clicked()
                    {
                        app.pick_and_upload_avatar();
                    }
                    render_status(ui, &app.settings_photo_status);
                });
            });

            ui.add_space(8.0);
            ui.label("Display name");
            ui.add_sized(
                [460.0, 30.0],
                egui::TextEdit::singleline(&mut app.settings_name_draft).hint_text("Your name"),
            );
            if ui
                .add_enabled(can_save_name, egui::Button::new("Save display name"))
                .clicked()
            {
                app.save_profile_name();
            }
            render_status(ui, &app.settings_name_status);

            ui.add_space(10.0);
            ui.label("Email");
            ui.add_sized(
                [460.0, 30.0],
                egui::TextEdit::singleline(&mut app.settings_email_draft)
                    .hint_text("you@example.com"),
            );
            if ui
                .add_enabled(can_save_email, egui::Button::new("Save email"))
                .clicked()
            {
                app.change_account_email();
            }
            render_status(ui, &app.settings_email_status);

            ui.add_space(10.0);
            ui.label("New password");
            ui.add_sized(
                [460.0, 30.0],
                egui::TextEdit::singleline(&mut app.settings_password_draft)
                    .password(true)
                    .hint_text("At least 8 characters"),
            );

            ui.label("Confirm password");
            ui.add_sized(
                [460.0, 30.0],
                egui::TextEdit::singleline(&mut app.settings_password_confirm_draft)
                    .password(true)
                    .hint_text("Re-enter password"),
            );

            if ui
                .add_enabled(can_save_password, egui::Button::new("Save password"))
                .clicked()
            {
                app.change_account_password();
            }
            render_status(ui, &app.settings_password_status);
        });

        ui.add_space(12.0);
        ui.group(|ui| {
            ui.heading("Appearance");
            ui.add_space(8.0);

            egui::ComboBox::from_id_salt("settings_theme_selector")
                .selected_text(match app.theme_preference {
                    crate::ThemePreference::System => "System",
                    crate::ThemePreference::Light => "Light",
                    crate::ThemePreference::Dark => "Dark",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.theme_preference,
                        crate::ThemePreference::System,
                        "System",
                    );
                    ui.selectable_value(
                        &mut app.theme_preference,
                        crate::ThemePreference::Light,
                        "Light",
                    );
                    ui.selectable_value(
                        &mut app.theme_preference,
                        crate::ThemePreference::Dark,
                        "Dark",
                    );
                });

            let theme_changed = app.theme_preference != profile_theme;
            if ui
                .add_enabled(theme_changed && !app.profile_loading, egui::Button::new("Save theme"))
                .clicked()
            {
                app.save_theme_preference();
            }
            render_status(ui, &app.settings_theme_status);
        });

        ui.add_space(12.0);
        ui.group(|ui| {
            ui.heading("Model metadata");
            ui.add_space(8.0);

            match app.model_cache.state_snapshot() {
                crate::cache::model_metadata::CacheState::Empty => {
                    ui.label("Metadata not loaded yet.");
                }
                crate::cache::model_metadata::CacheState::Loading => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Fetching model metadata...");
                    });
                }
                crate::cache::model_metadata::CacheState::Loaded { models, .. } => {
                    ui.label(format!("Loaded {} models.", models.len()));
                }
                crate::cache::model_metadata::CacheState::Failed(message) => {
                    ui.colored_label(egui::Color32::from_rgb(255, 99, 99), message);
                }
            }
        });

        ui.add_space(12.0);
        ui.group(|ui| {
            ui.heading("Account");
            ui.add_space(8.0);

            ui.label("Type DELETE to enable account deletion");
            ui.add_sized(
                [220.0, 28.0],
                egui::TextEdit::singleline(&mut app.delete_account_confirmation)
                    .hint_text("DELETE"),
            );

            let button = egui::Button::new(
                egui::RichText::new("Delete account")
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(egui::Color32::from_rgb(170, 45, 45));

            if ui
                .add_enabled(
                    app.delete_account_confirmation.trim() == "DELETE" && !app.profile_loading,
                    button,
                )
                .clicked()
            {
                app.request_delete_account();
            }
            render_status(ui, &app.settings_account_status);
        });
    });
}
