use eframe::egui;

pub(crate) fn render_sessions(app: &mut crate::AppState, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!app.is_loading, egui::Button::new("+ New Chat"))
            .clicked()
        {
            app.start_new_chat();
        }

        if ui
            .add_enabled(
                app.active_conversation_id.is_some() && app.pending_delete_conversation_id.is_none(),
                egui::Button::new("Delete"),
            )
            .clicked()
        {
            app.pending_delete_conversation_id = app.active_conversation_id;
        }
    });

    ui.add_space(8.0);
    ui.heading("Chats");
    ui.add_space(8.0);

    if app.conversations.is_empty() {
        if app.creating_new_chat {
            ui.label("Creating chat...");
        } else {
            ui.label("No chats yet. Start a new one.");
        }
        return;
    }

    for conversation in app.conversations.clone() {
        let selected = app.active_conversation_id == Some(conversation.id);
        let label = crate::AuvroApp::sidebar_title(&conversation.title);

        if ui
            .add_sized(
                [ui.available_width(), 30.0],
                egui::Button::new(label).selected(selected),
            )
            .clicked()
        {
            app.select_conversation(conversation.id);
        }
        ui.add_space(4.0);
    }
}

pub(crate) fn render_compact_controls(app: &mut crate::AppState, ctx: &egui::Context) {
    egui::TopBottomPanel::top("compact_controls").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label("Chat");
            let selected_name = app
                .active_conversation_id
                .and_then(|id| app.conversations.iter().find(|conversation| conversation.id == id))
                .map_or("Select Chat", |conversation| conversation.title.as_str());

            egui::ComboBox::from_id_salt("conversation_selector")
                .selected_text(selected_name)
                .show_ui(ui, |ui| {
                    let mut pending_selection = None;
                    for conversation in app.conversations.iter() {
                        if ui
                            .selectable_label(
                                app.active_conversation_id == Some(conversation.id),
                                &conversation.title,
                            )
                            .clicked()
                        {
                            pending_selection = Some(conversation.id);
                        }
                    }

                    if let Some(conversation_id) = pending_selection {
                        app.select_conversation(conversation_id);
                    }
                });

            if ui
                .add_enabled(!app.is_loading, egui::Button::new("+ New"))
                .clicked()
            {
                app.start_new_chat();
            }

            if ui
                .add_enabled(
                    app.active_conversation_id.is_some() && app.pending_delete_conversation_id.is_none(),
                    egui::Button::new("Delete"),
                )
                .clicked()
            {
                app.pending_delete_conversation_id = app.active_conversation_id;
            }
        });
    });
}

pub(crate) fn render_delete_confirmation(app: &mut crate::AppState, ctx: &egui::Context) {
    let Some(conversation_id) = app.pending_delete_conversation_id else {
        return;
    };

    let title = app
        .conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id)
        .map(|conversation| conversation.title.clone())
        .unwrap_or_else(|| "this chat".to_owned());

    egui::Window::new("Delete conversation")
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!("Delete \"{}\"? This cannot be undone.", title));
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    app.pending_delete_conversation_id = None;
                }

                if ui
                    .add_enabled(!app.is_loading, egui::Button::new("Delete"))
                    .clicked()
                {
                    app.request_delete_conversation(conversation_id);
                    app.pending_delete_conversation_id = None;
                }
            });
        });
}
