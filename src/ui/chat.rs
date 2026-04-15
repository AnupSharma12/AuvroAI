use eframe::egui;

const CHAT_MAX_WIDTH: f32 = 800.0;

enum MessageBlock {
    Paragraph(String),
    Code {
        language: Option<String>,
        code: String,
    },
}

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

fn render_empty_state(app: &mut crate::AppState, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(52.0);
        ui.label(
            egui::RichText::new("AuvroAI")
                .size(24.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new("A calm, centered workspace for focused chat")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(20.0);

        egui::Frame::new()
            .fill(egui::Color32::from_rgba_premultiplied(255, 255, 255, 4))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 14),
            ))
            .corner_radius(egui::CornerRadius::same(16))
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(520.0);
                ui.vertical_centered(|ui| {
                    if ui
                        .add_sized([180.0, 40.0], egui::Button::new("New chat"))
                        .clicked()
                    {
                        app.start_new_chat();
                    }

                    ui.add_space(16.0);
                    ui.horizontal_wrapped(|ui| {
                        for suggestion in [
                            "Summarize a long article",
                            "Draft a professional email",
                            "Plan my learning roadmap",
                        ] {
                            if ui
                                .add(egui::Button::new(suggestion).frame(false))
                                .clicked()
                            {
                                app.start_new_chat();
                                app.draft_message = suggestion.to_owned();
                            }
                        }
                    });
                });
            });
    });
}

fn split_message_blocks(content: &str) -> Vec<MessageBlock> {
    let mut blocks = Vec::new();
    let mut text_buffer = String::new();
    let mut code_buffer = String::new();
    let mut in_code_block = false;
    let mut code_language: Option<String> = None;

    for line in content.lines() {
        let fence = line.trim_start();
        if fence.starts_with("```") {
            if in_code_block {
                blocks.push(MessageBlock::Code {
                    language: code_language.take(),
                    code: code_buffer.trim_end().to_owned(),
                });
                code_buffer.clear();
                in_code_block = false;
            } else {
                if !text_buffer.trim().is_empty() {
                    blocks.push(MessageBlock::Paragraph(text_buffer.trim().to_owned()));
                    text_buffer.clear();
                }

                let language = fence.trim_start_matches("```").trim();
                code_language = if language.is_empty() {
                    None
                } else {
                    Some(language.to_owned())
                };
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_buffer.push_str(line);
            code_buffer.push('\n');
        } else {
            text_buffer.push_str(line);
            text_buffer.push('\n');
        }
    }

    if in_code_block {
        blocks.push(MessageBlock::Code {
            language: code_language,
            code: code_buffer.trim_end().to_owned(),
        });
    } else if !text_buffer.trim().is_empty() {
        blocks.push(MessageBlock::Paragraph(text_buffer.trim().to_owned()));
    }

    if blocks.is_empty() {
        blocks.push(MessageBlock::Paragraph(content.to_owned()));
    }

    blocks
}

fn render_paragraph(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    for (index, paragraph) in text.split("\n\n").enumerate() {
        if index > 0 {
            ui.add_space(8.0);
        }

        ui.add(
            egui::Label::new(egui::RichText::new(paragraph).size(15.0).color(color))
                .wrap(),
        );
    }
}

fn render_code_block(ui: &mut egui::Ui, language: Option<&str>, code: &str) {
    let label = language.unwrap_or("code");
    let border = ui.visuals().widgets.noninteractive.bg_stroke.color;

    egui::Frame::new()
        .fill(egui::Color32::from_rgba_premultiplied(17, 21, 28, 235))
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(label.to_ascii_uppercase())
                        .small()
                        .strong()
                        .color(ui.visuals().weak_text_color()),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("Copy").frame(false)).clicked() {
                        ui.ctx().copy_text(code.to_owned());
                    }
                });
            });

            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(code)
                        .monospace()
                        .size(14.0)
                        .color(egui::Color32::from_rgb(226, 231, 238)),
                )
                .wrap(),
            );
        });
}

fn render_message_row(app: &crate::AppState, ui: &mut egui::Ui, message: &crate::api::conversations::Message) {
    let is_user = message.role == "user";
    let blocks = split_message_blocks(&message.content);
    let max_width = if is_user { 650.0 } else { 720.0 };

    ui.with_layout(
        if is_user {
            egui::Layout::right_to_left(egui::Align::TOP)
        } else {
            egui::Layout::left_to_right(egui::Align::TOP)
        },
        |ui| {
            ui.set_max_width(max_width);

            let fill = egui::Color32::from_rgb(255, 255, 255);
            let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 223, 228));

            egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    ui.set_width(max_width);

                    ui.horizontal(|ui| {
                        let label = if is_user { "You" } else { "Auvro" };

                        ui.label(
                            egui::RichText::new(label)
                                .small()
                                .strong()
                                .color(egui::Color32::from_rgb(0, 0, 0)),
                        );

                        if !is_user && app.is_loading {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Streaming")
                                    .small()
                                    .color(egui::Color32::from_rgb(0, 0, 0)),
                            );
                        }
                    });

                    ui.add_space(8.0);

                    for block in blocks {
                        match block {
                            MessageBlock::Paragraph(text) => {
                                render_paragraph(
                                    ui,
                                    &text,
                                    egui::Color32::from_rgb(0, 0, 0),
                                );
                            }
                            MessageBlock::Code { language, code } => {
                                render_code_block(ui, language.as_deref(), &code);
                            }
                        }

                        ui.add_space(10.0);
                    }
                });
        },
    );

    ui.add_space(12.0);
}

pub(crate) fn render_chat_panel(app: &mut crate::AppState, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let conversation_name = app
                .active_conversation_id
                .and_then(|conversation_id| {
                    app.conversations
                        .iter()
                        .find(|conversation| conversation.id == conversation_id)
                })
                .map(|conversation| conversation.title.as_str())
                .unwrap_or("New chat");

            ui.label(
                egui::RichText::new(conversation_name)
                    .size(18.0)
                    .strong(),
            );
            ui.label(
                egui::RichText::new("Centered conversation canvas")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            render_chat_avatar_button(app, ui);
        });
    });

    ui.add_space(10.0);

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

    let Some(_active_conversation_id) = app.active_conversation_id else {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.heading("Select a chat");
            ui.label("Choose a conversation from the sidebar or start a new chat.");
        });
        render_avatar_menu(app, ui.ctx());
        return;
    };

    let content_width = ui.available_width().min(CHAT_MAX_WIDTH);
    let message_list_height = (ui.available_height() - 170.0).max(240.0);

    ui.vertical_centered(|ui| {
        ui.set_max_width(CHAT_MAX_WIDTH);

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
            .max_height(message_list_height)
            .show(ui, |ui| {
                ui.set_width(content_width);

                if app.messages.is_empty() && !app.messages_loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(36.0);
                        ui.label(
                            egui::RichText::new("No messages yet")
                                .size(16.0)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("Send the first note to start the thread.")
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ui.add_space(20.0);
                }

                for message in &app.messages {
                    render_message_row(app, ui, message);
                }
            });

        if app.is_loading {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    egui::RichText::new("Streaming response")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        }

        if let Some(error) = &app.error_message {
            ui.add_space(6.0);
            ui.colored_label(
                egui::Color32::from_rgb(255, 111, 111),
                format!("error: {error}"),
            );
            if ui.button("Clear error").clicked() {
                app.error_message = None;
            }
        }

        ui.add_space(12.0);

        let draft_lines = app.draft_message.lines().count().clamp(1, 6) as f32;
        let composer_height = 42.0 + (draft_lines - 1.0) * 18.0;
        let can_send = !app.is_loading
            && !app.messages_loading
            && app.auth_error.is_none()
            && !crate::env::SUPABASE_PUBLISHABLE_KEY.trim().is_empty()
            && app.active_conversation_id.is_some();

        egui::Frame::new()
            .fill(egui::Color32::from_rgb(255, 255, 255))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(223, 227, 233),
            ))
            .corner_radius(egui::CornerRadius::same(18))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(content_width);

                ui.horizontal(|ui| {
                    let editor_width = (ui.available_width() - 84.0).max(220.0);
                    let editor = ui.add_sized(
                        [editor_width, composer_height],
                        egui::TextEdit::multiline(&mut app.draft_message)
                            .desired_rows(2)
                            .frame(false)
                            .hint_text("Message AuvroAI...")
                            .id_salt("composer_input"),
                    );

                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let shift_held = ui.input(|i| i.modifiers.shift);
                    if editor.has_focus() && enter_pressed && !shift_held && can_send {
                        app.draft_message = app.draft_message.trim_end_matches('\n').to_owned();
                        app.send_message();
                    }

                    ui.add_space(10.0);

                    let send_clicked = ui
                        .add_enabled(
                            can_send,
                            egui::Button::new(
                                egui::RichText::new("Send").strong(),
                            ),
                        )
                        .clicked();
                    if send_clicked {
                        app.send_message();
                    }
                });
            });
    });

    render_avatar_menu(app, ui.ctx());
}
