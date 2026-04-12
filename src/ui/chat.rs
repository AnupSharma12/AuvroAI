use eframe::egui;

pub(crate) fn render_empty_state(app: &mut crate::AppState, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(70.0);
        ui.heading("AuvroAI");
        ui.label("What can I help you with today?");
        ui.add_space(16.0);

        if ui.button("New Chat").clicked() {
            app.creating_new_chat = true;
            app.selected_session = None;
        }

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            for suggestion in [
                "Summarize a long article",
                "Draft a professional email",
                "Plan my learning roadmap",
            ] {
                if ui.button(suggestion).clicked() {
                    app.creating_new_chat = true;
                    app.selected_session = None;
                    app.draft_message = suggestion.to_owned();
                }
            }
        });
    });
}

pub(crate) fn render_chat_panel(app: &mut crate::AppState, ui: &mut egui::Ui) {
    if app.sessions.is_empty() && !app.creating_new_chat {
        render_empty_state(app, ui);
        return;
    }

    let session_name = app
        .selected_session
        .and_then(|idx| app.sessions.get(idx))
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
            if let Some(session) = app.selected_session.and_then(|idx| app.sessions.get(idx)) {
                for message in &session.messages {
                    let prefix = if message.role == "user" { "You" } else { "Auvro" };
                    ui.label(format!("{prefix}: {}", message.content));
                }
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
                && app.auth_error.is_none()
                && !crate::env::SUPABASE_PUBLISHABLE_KEY.trim().is_empty(),
            egui::Button::new("Send"),
        )
        .clicked();
    if send_clicked {
        app.send_message();
    }
}
