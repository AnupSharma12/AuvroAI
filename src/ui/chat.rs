use eframe::egui;

fn render_chat_avatar_button(app: &mut crate::AppState, ui: &mut egui::Ui) {
    let initials = app.profile_initials();
    let response = app.render_profile_avatar(ui, &initials, 16.0, true);
    if response.clicked() {
        app.profile_menu_open = !app.profile_menu_open;
        app.profile_menu_anchor = Some(response.rect.left_bottom() + egui::vec2(-120.0, 8.0));
    }
}

fn render_avatar_menu(app: &mut crate::AppState, ctx: &egui::Context) {
    if !app.profile_menu_open {
        return;
    }

    let anchor = app
        .profile_menu_anchor
        .unwrap_or_else(|| egui::pos2(24.0, 84.0));

    egui::Area::new("chat_avatar_menu".into())
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(140.0);

                if ui.button("Settings").clicked() {
                    app.show_settings = true;
                    app.profile_menu_open = false;
                }

                if ui.button("Log out").clicked() {
                    app.profile_menu_open = false;
                    app.logout();
                }
            });
        });
}

pub(crate) fn render_empty_state(app: &mut crate::AppState, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(70.0);
        ui.heading("AuvroAI");
        ui.label("What can I help you with today?");
        ui.add_space(16.0);

        if ui.button("New Chat").clicked() {
            app.start_new_chat();
        }

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            for suggestion in [
                "Summarize a long article",
                "Draft a professional email",
                "Plan my learning roadmap",
            ] {
                if ui.button(suggestion).clicked() {
                    app.start_new_chat();
                    app.draft_message = suggestion.to_owned();
                }
            }
        });
    });
}

pub(crate) fn render_chat_panel(app: &mut crate::AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            render_chat_avatar_button(app, ui);
        });
    });
    ui.add_space(6.0);

    if app.active_conversation_id.is_none() && app.conversations.is_empty() && !app.creating_new_chat {
        render_empty_state(app, ui);
        render_avatar_menu(app, ui.ctx());
        return;
    }

    if app.active_conversation_id.is_none() && app.creating_new_chat {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.spinner();
            ui.label("Creating chat...");
        });
        render_avatar_menu(app, ui.ctx());
        return;
    }

    let Some(active_conversation_id) = app.active_conversation_id else {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.heading("Select a chat");
            ui.label("Choose a conversation from the sidebar or start a new chat.");
        });
        render_avatar_menu(app, ui.ctx());
        return;
    };

    let conversation_name = app
        .conversations
        .iter()
        .find(|conversation| conversation.id == active_conversation_id)
        .map(|conversation| conversation.title.as_str())
        .unwrap_or("Chat");

    ui.heading(format!("Chat - {conversation_name}"));
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(8.0);

    if app.messages_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Loading messages...");
        });
        ui.add_space(8.0);
    }

    egui::ScrollArea::vertical()
        .id_salt("chat_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if app.messages.is_empty() && !app.messages_loading {
                ui.label("No messages yet. Send your first message to get started.");
                ui.add_space(8.0);
            }

            for message in &app.messages {
                let prefix = if message.role == "user" { "You" } else { "Auvro" };
                ui.label(format!("{prefix}: {}", message.content));
            }
        });

    ui.add_space(8.0);
    if app.is_loading {
        ui.colored_label(egui::Color32::from_rgb(255, 196, 61), "Auvro is typing...");
    }

    if let Some(error) = &app.error_message {
        ui.colored_label(
            egui::Color32::from_rgb(255, 99, 99),
            format!("error: {error}"),
        );
        if ui.button("Clear Error").clicked() {
            app.error_message = None;
        }
    }

    ui.add_space(12.0);
    ui.label("Message");
    let editor = ui.add(
        egui::TextEdit::multiline(&mut app.draft_message)
            .desired_rows(4)
            .hint_text("Type your message..."),
    );

    let (enter_pressed, shift_held) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.modifiers.shift));
    if editor.has_focus() && enter_pressed && !shift_held {
        app.draft_message = app.draft_message.trim_end_matches('\n').to_owned();
        app.send_message();
    }

    let send_clicked = ui
        .add_enabled(
            !app.is_loading
                && !app.messages_loading
                && app.auth_error.is_none()
                && !crate::env::SUPABASE_PUBLISHABLE_KEY.trim().is_empty()
                && app.active_conversation_id.is_some(),
            egui::Button::new("Send"),
        )
        .clicked();
    if send_clicked {
        app.send_message();
    }

    render_avatar_menu(app, ui.ctx());
}
