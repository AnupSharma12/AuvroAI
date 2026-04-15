use eframe::egui;

pub(crate) fn render_sessions(app: &mut crate::AppState, ui: &mut egui::Ui, _ctx: &egui::Context) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new("Chats")
                .size(18.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new("Your recent threads")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if ui
                .add_enabled(!app.is_loading, egui::Button::new("New chat"))
                .clicked()
            {
                app.start_new_chat();
            }

            if ui
                .add_enabled(
                    app.active_conversation_id.is_some()
                        && app.pending_delete_conversation_id.is_none(),
                    egui::Button::new("Delete"),
                )
                .clicked()
            {
                app.pending_delete_conversation_id = app.active_conversation_id;
            }
        });
    });

    ui.add_space(14.0);

    if app.conversations.is_empty() {
        let empty_text = if app.creating_new_chat {
            "Creating chat..."
        } else {
            "No chats yet. Start a new one."
        };

        egui::Frame::new()
            .fill(egui::Color32::from_rgba_premultiplied(255, 255, 255, 4))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 12),
            ))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(empty_text)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        return;
    }

    for conversation in app.conversations.clone() {
        let selected = app.active_conversation_id == Some(conversation.id);
        let label = crate::AuvroApp::sidebar_title(&conversation.title);

        let background = if selected {
            egui::Color32::from_rgba_premultiplied(255, 255, 255, 10)
        } else {
            egui::Color32::from_rgba_premultiplied(255, 255, 255, 4)
        };

        egui::Frame::new()
            .fill(background)
            .stroke(egui::Stroke::new(
                1.0,
                if selected {
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 28)
                } else {
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 12)
                },
            ))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                if ui
                    .add_sized(
                        [ui.available_width(), 20.0],
                        egui::Button::new(
                            egui::RichText::new(label).strong().color(ui.visuals().text_color()),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    app.select_conversation(conversation.id);
                }
            });

        ui.add_space(8.0);
    }
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
