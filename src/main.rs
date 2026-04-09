use eframe::egui;

#[derive(Default)]
struct AuvroApp {
    draft_message: String,
    sessions: Vec<String>,
}

impl eframe::App for AuvroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("session_sidebar")
            .resizable(true)
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.heading("Sessions");
                if self.sessions.is_empty() {
                    self.sessions.push("New Chat".to_owned());
                }
                for session in &self.sessions {
                    ui.label(session);
                }
            });

        egui::SidePanel::right("settings_panel")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.label("Provider: OpenAI-compatible");
                ui.label("Model: gpt-4.1-mini");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("AuvroAI");
            ui.separator();
            ui.label("Chat output will stream here.");
            ui.add_space(12.0);
            ui.label("Message");
            ui.text_edit_multiline(&mut self.draft_message);
            if ui.button("Send").clicked() {
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
