use eframe::egui;

pub(crate) fn render_sessions(app: &mut crate::AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button("+ New Chat").clicked() {
            app.selected_session = None;
            app.creating_new_chat = true;
            app.error_message = None;
            app.renaming_session = false;
        }
    });

    ui.add_space(8.0);
    ui.heading("Chats");
    ui.add_space(8.0);

    for idx in 0..app.sessions.len() {
        let selected = app.selected_session == Some(idx);
        let label = crate::AuvroApp::sidebar_title(&app.sessions[idx].name);

        if ui
            .add_sized(
                [ui.available_width(), 30.0],
                egui::Button::new(label).selected(selected),
            )
            .clicked()
        {
            app.selected_session = Some(idx);
            app.creating_new_chat = false;
            app.load_selected_session_messages();
        }
        ui.add_space(4.0);
    }
}

pub(crate) fn render_compact_controls(app: &mut crate::AppState, ctx: &egui::Context) {
    egui::TopBottomPanel::top("compact_controls").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label("Chat");
            let selected_name = app
                .selected_session
                .and_then(|idx| app.sessions.get(idx))
                .map_or("New Chat", |s| s.name.as_str());

            egui::ComboBox::from_id_salt("session_selector")
                .selected_text(selected_name)
                .show_ui(ui, |ui| {
                    let mut pending_selection: Option<usize> = None;
                    for (idx, session) in app.sessions.iter().enumerate() {
                        if ui
                            .selectable_label(app.selected_session == Some(idx), &session.name)
                            .clicked()
                        {
                            pending_selection = Some(idx);
                        }
                    }

                    if let Some(idx) = pending_selection {
                        app.selected_session = Some(idx);
                        app.creating_new_chat = false;
                        app.load_selected_session_messages();
                    }
                });

            if ui.button("+ New").clicked() {
                app.selected_session = None;
                app.creating_new_chat = true;
            }
        });
    });
}
