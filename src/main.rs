use eframe::egui;

struct AuvroApp {
    draft_message: String,
    sessions: Vec<String>,
    selected_session: usize,
    chat_log: Vec<String>,
}

impl Default for AuvroApp {
    fn default() -> Self {
        Self {
            draft_message: String::new(),
            sessions: vec!["General".to_owned(), "Ideas".to_owned()],
            selected_session: 0,
            chat_log: vec!["assistant> Welcome to AuvroAI".to_owned()],
        }
    }
}

impl eframe::App for AuvroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("app_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("AuvroAI");
                ui.separator();
                ui.label("Cross-platform Rust chat app shell");
            });
        });

        egui::SidePanel::left("session_sidebar")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Sessions");
                ui.add_space(6.0);

                for (idx, session) in self.sessions.iter().enumerate() {
                    if ui
                        .selectable_label(self.selected_session == idx, session)
                        .clicked()
                    {
                        self.selected_session = idx;
                    }
                }

                ui.add_space(8.0);
                if ui.button("+ New Session").clicked() {
                    self.sessions
                        .push(format!("Session {}", self.sessions.len() + 1));
                    self.selected_session = self.sessions.len() - 1;
                }
            });

        egui::SidePanel::right("settings_panel")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.heading("Quick Start");
                ui.add_space(8.0);
                ui.label("No API key required.");
                ui.label("No setup required.");
                ui.label("Type a message and press Send.");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let session_name = self
                .sessions
                .get(self.selected_session)
                .map_or("General", String::as_str);

            ui.heading(format!("Chat - {session_name}"));
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("chat_scroll")
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.chat_log {
                        ui.label(line);
                    }
                });

            ui.add_space(12.0);
            ui.label("Message");
            ui.text_edit_multiline(&mut self.draft_message);

            if ui.button("Send").clicked() && !self.draft_message.trim().is_empty() {
                self.chat_log
                    .push(format!("you> {}", self.draft_message.trim()));
                self.chat_log
                    .push("assistant> Placeholder response".to_owned());
                self.draft_message.clear();
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 680.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AuvroAI",
        options,
        Box::new(|_cc| Ok(Box::<AuvroApp>::default())),
    )
}
