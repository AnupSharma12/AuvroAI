use eframe::egui;

fn is_error_notice(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    lowered.contains("failed")
        || lowered.contains("required")
        || lowered.contains("invalid")
        || lowered.contains("missing")
        || lowered.contains("error")
        || lowered.contains("could not")
        || lowered.contains("not configured")
}

fn text_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    id_source: &'static str,
    width: f32,
    label_color: egui::Color32,
) -> bool {
    ui.label(egui::RichText::new(label).small().color(label_color));
    ui.add_sized(
        [width, 40.0],
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .id_salt(id_source),
    )
    .changed()
}

fn password_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    id_source: &'static str,
    width: f32,
    show_password: &mut bool,
    label_color: egui::Color32,
) -> bool {
    ui.label(egui::RichText::new(label).small().color(label_color));

    let mut changed = false;
    ui.horizontal(|ui| {
        let input_w = (width - 84.0).max(140.0);
        let response = ui.add_sized(
            [input_w, 38.0],
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .password(!*show_password)
                .id_salt(id_source),
        );
        changed |= response.changed();

        if ui
            .add_sized(
                [76.0, 38.0],
                egui::Button::new(if *show_password { "Hide" } else { "Show" }),
            )
            .clicked()
        {
            *show_password = !*show_password;
        }
    });

    changed
}

fn notice_box(ui: &mut egui::Ui, text: &str, is_error: bool, width: f32) {
    if is_error {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(254, 226, 226))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(width);
                ui.label(
                    egui::RichText::new(text)
                        .size(13.0)
                        .color(egui::Color32::from_rgb(153, 27, 27)),
                );
            });
    } else {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(236, 253, 245))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(width);
                ui.label(
                    egui::RichText::new(text)
                        .size(13.0)
                        .color(egui::Color32::from_rgb(21, 128, 61)),
                );
            });
    }
}

pub(crate) fn render(app: &mut crate::AppState, ui: &mut egui::Ui) {
    const CARD_MAX_WIDTH: f32 = 400.0;
    const CARD_MARGIN_X: f32 = 16.0;
    const CARD_PADDING: i8 = 16;

    let card_width = (ui.available_width() - CARD_MARGIN_X)
        .clamp(280.0, CARD_MAX_WIDTH);
    let content_width = card_width - (CARD_PADDING as f32 * 2.0);
    let heading_size = if card_width < 340.0 { 20.0 } else { 22.0 };

    let show_password_id = ui.make_persistent_id("auth_show_password");
    let mut show_password = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<bool>(show_password_id).unwrap_or(false));

    ui.add_space((ui.available_height() * 0.02).clamp(4.0, 14.0));

    egui::ScrollArea::vertical()
        .id_salt("auth_form_scroll")
        .auto_shrink([true, true])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .stroke(egui::Stroke::new(
                        1.0,
                        ui.visuals().widgets.noninteractive.bg_stroke.color,
                    ))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(CARD_PADDING))
                    .show(ui, |ui| {
                        ui.set_min_width(content_width);
                        ui.set_max_width(content_width);
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);

                        let weak_text = ui.visuals().weak_text_color();
                        let label_color = egui::Color32::from_rgb(115, 115, 115);

                        ui.label(
                            egui::RichText::new("Welcome")
                                .size(heading_size)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("Sign in to sync your chats and profile")
                                .small()
                                .color(weak_text),
                        );

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut app.auth_mode, crate::AuthMode::Login, "Login");
                            ui.selectable_value(&mut app.auth_mode, crate::AuthMode::SignUp, "Sign Up");
                        });

                        ui.add_space(8.0);
                        let mut edited = false;

                        if app.auth_mode == crate::AuthMode::SignUp {
                            edited |= text_field(
                                ui,
                                "Display Name",
                                &mut app.auth_full_name,
                                "Jane Doe",
                                "auth_display_name",
                                content_width,
                                label_color,
                            );
                            ui.add_space(10.0);
                        }

                        edited |= text_field(
                            ui,
                            "Email",
                            &mut app.auth_email,
                            "you@example.com",
                            "auth_email",
                            content_width,
                            label_color,
                        );
                        ui.add_space(10.0);

                        edited |= password_field(
                            ui,
                            "Password",
                            &mut app.auth_password,
                            "Enter password",
                            "auth_password",
                            content_width,
                            &mut show_password,
                            label_color,
                        );

                        if app.auth_mode == crate::AuthMode::SignUp {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("Confirm Password").small().color(label_color));
                            edited |= ui
                                .add_sized(
                                    [content_width, 38.0],
                                    egui::TextEdit::singleline(&mut app.auth_confirm_password)
                                        .hint_text("Confirm password")
                                        .password(true)
                                        .id_salt("auth_confirm_password"),
                                )
                                .changed();
                        }

                        if edited
                            && app
                                .auth_notice
                                .as_deref()
                                .map(is_error_notice)
                                .unwrap_or(false)
                        {
                            app.auth_notice = None;
                        }

                        let auth_ready = app.auth_error.is_none()
                            && !crate::env::SUPABASE_URL.trim().is_empty()
                            && !crate::env::SUPABASE_PUBLISHABLE_KEY.trim().is_empty();

                        ui.add_space(12.0);
                        let action_label = if app.auth_mode == crate::AuthMode::Login {
                            "Login"
                        } else {
                            "Create Account"
                        };

                        if ui
                            .add_sized(
                                [content_width, 40.0],
                                egui::Button::new(
                                    egui::RichText::new(action_label)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(ui.visuals().selection.bg_fill)
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(egui::CornerRadius::same(8)),
                            )
                            .clicked()
                        {
                            match app.auth_mode {
                                crate::AuthMode::Login => {
                                    if auth_ready {
                                        app.login_with_email();
                                    } else {
                                        app.auth_notice = Some(app.auth_error.clone().unwrap_or_else(|| {
                                            "Supabase is not configured. Set SUPABASE_URL and SUPABASE_PUBLISHABLE_KEY in .env."
                                                .to_owned()
                                        }));
                                    }
                                }
                                crate::AuthMode::SignUp => {
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
                        }

                        ui.add_space(10.0);
                        if let Some(error) = &app.auth_error {
                            notice_box(ui, error, true, content_width);
                            ui.add_space(10.0);
                        } else if let Some(notice) = &app.auth_notice {
                            notice_box(ui, notice, is_error_notice(notice), content_width);
                            ui.add_space(10.0);
                        }

                        ui.horizontal(|ui| {
                            let is_login = app.auth_mode == crate::AuthMode::Login;
                            let prompt = if is_login {
                                "No account?"
                            } else {
                                "Already have an account?"
                            };
                            let link_text = if is_login { "Sign Up" } else { "Login" };

                            ui.label(egui::RichText::new(prompt).small().color(weak_text));
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(link_text)
                                            .small()
                                            .underline()
                                            .color(ui.visuals().selection.bg_fill),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                app.auth_mode = if is_login {
                                    crate::AuthMode::SignUp
                                } else {
                                    crate::AuthMode::Login
                                };
                            }
                        });
                    });
            });
        });

    ui.ctx()
        .data_mut(|d| d.insert_persisted(show_password_id, show_password));
}
