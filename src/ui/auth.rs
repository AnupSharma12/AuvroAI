use eframe::egui;

pub(crate) fn render(app: &mut crate::AppState, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading("Welcome to AuvroAI");
        ui.label("Sign in to sync chats with Supabase");
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.auth_mode, crate::AuthMode::Login, "Login");
            ui.selectable_value(&mut app.auth_mode, crate::AuthMode::SignUp, "Sign Up");
        });

        ui.add_space(12.0);

        if app.auth_mode == crate::AuthMode::SignUp {
            ui.label("Full name");
            ui.add_sized(
                [420.0, 32.0],
                egui::TextEdit::singleline(&mut app.auth_full_name).hint_text("Jane Doe"),
            );
            ui.add_space(8.0);
        }

        ui.label("Email");
        ui.add_sized(
            [420.0, 32.0],
            egui::TextEdit::singleline(&mut app.auth_email).hint_text("you@example.com"),
        );

        ui.add_space(8.0);
        ui.label("Password");
        ui.add_sized(
            [420.0, 32.0],
            egui::TextEdit::singleline(&mut app.auth_password)
                .password(true)
                .hint_text("Enter password"),
        );

        if app.auth_mode == crate::AuthMode::SignUp {
            ui.add_space(8.0);
            ui.label("Confirm password");
            ui.add_sized(
                [420.0, 32.0],
                egui::TextEdit::singleline(&mut app.auth_confirm_password)
                    .password(true)
                    .hint_text("Confirm password"),
            );
        }

        let auth_ready = app.auth_error.is_none()
            && !crate::env::SUPABASE_URL.trim().is_empty()
            && !crate::env::SUPABASE_PUBLISHABLE_KEY.trim().is_empty();

        if let Some(error) = &app.auth_error {
            ui.add_space(10.0);
            ui.colored_label(egui::Color32::from_rgb(255, 99, 99), error);
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| match app.auth_mode {
            crate::AuthMode::Login => {
                if ui.button("Login").clicked() {
                    if auth_ready {
                        app.login_with_email();
                    } else {
                        app.auth_notice = Some(app.auth_error.clone().unwrap_or_else(|| {
                            "Supabase is not configured. Set SUPABASE_URL and SUPABASE_PUBLISHABLE_KEY in .env."
                                .to_owned()
                        }));
                    }
                }
            }
            crate::AuthMode::SignUp => {
                if ui.button("Sign Up").clicked() {
                    if auth_ready {
                        app.signup_with_email();
                    } else {
                        app.auth_notice = Some(app.auth_error.clone().unwrap_or_else(|| {
                            "Supabase is not configured. Set SUPABASE_URL and SUPABASE_PUBLISHABLE_KEY in .env."
                                .to_owned()
                        }));
                    }
                }
            }
        });

        if let Some(notice) = &app.auth_notice {
            ui.add_space(10.0);
            let lowered = notice.to_ascii_lowercase();
            let is_error = lowered.contains("failed") || lowered.contains("required");
            let color = if is_error {
                egui::Color32::from_rgb(255, 99, 99)
            } else {
                egui::Color32::from_rgb(48, 146, 85)
            };
            ui.colored_label(color, notice);
        }
    });
}
