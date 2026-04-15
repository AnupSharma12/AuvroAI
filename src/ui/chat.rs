use eframe::egui;
use egui_commonmark::CommonMarkViewer;

const CHAT_MAX_WIDTH: f32 = 800.0;

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
        crate::ui::render_app_logo(ui, 72.0);
        ui.add_space(14.0);
        ui.label(egui::RichText::new("AuvroAI").size(24.0).strong());
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

fn render_message_row(
    app: &mut crate::AppState,
    ui: &mut egui::Ui,
    message: &crate::api::conversations::Message,
    message_index: usize,
) {
    let is_user = message.role == "user";
    let available_width = ui.available_width();
    let max_width = if is_user {
        (available_width - 20.0).clamp(180.0, 650.0)
    } else {
        (available_width - 20.0).clamp(180.0, 720.0)
    };
    let is_live_stream = !is_user
        && app.is_loading
        && app.stream_line_index == Some(message_index)
        && !app.streaming_buffer.is_empty();

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

            let frame_response = egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    if is_user {
                        ui.set_width(max_width);
                    } else {
                        ui.set_max_width(max_width);
                    }

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

                    if is_user {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&message.content)
                                    .size(15.0)
                                    .color(egui::Color32::from_rgb(0, 0, 0)),
                            )
                            .wrap(),
                        );
                    } else if is_live_stream {
                        let streaming_content = app
                            .streaming_buffer
                            .replace('\u{202F}', " ")
                            .replace('\u{00A0}', " ");
                        CommonMarkViewer::new().show(
                            ui,
                            &mut app.markdown_cache,
                            &streaming_content,
                        );
                    } else {
                        let assistant_content = message
                            .content
                            .replace('\u{202F}', " ")
                            .replace('\u{00A0}', " ");
                        CommonMarkViewer::new().show(
                            ui,
                            &mut app.markdown_cache,
                            &assistant_content,
                        );
                    }
                });

            if !is_user {
                let message_rect = frame_response.response.rect;
                let copy_rect = egui::Rect::from_min_size(
                    egui::pos2(message_rect.right() - 42.0, message_rect.bottom() - 30.0),
                    egui::vec2(36.0, 22.0),
                );

                if ui
                    .put(
                        copy_rect,
                        egui::Button::new(egui::RichText::new("📋").size(13.0))
                            .corner_radius(egui::CornerRadius::same(4)),
                    )
                    .clicked()
                {
                    ui.ctx().copy_text(message.content.clone());
                }
            }
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

    let content_width = ui.available_width().clamp(200.0, CHAT_MAX_WIDTH);
    let message_list_height = (ui.available_height() - 150.0).max(96.0);

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

                let messages_snapshot = app.messages.clone();
                for (message_index, message) in messages_snapshot.iter().enumerate() {
                    render_message_row(
                        app,
                        ui,
                        message,
                        message_index,
                    );
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

        let draft_lines = app.draft_message.lines().count().clamp(1, 4) as f32;
        let composer_height = 52.0 + (draft_lines - 1.0) * 16.0;
        let can_send = !app.is_loading
            && !app.messages_loading
            && app.auth_error.is_none()
            && !crate::env::SUPABASE_PUBLISHABLE_KEY.trim().is_empty();

        let available_rect = ui.available_rect_before_wrap();
        let composer_width = (available_rect.width() - 24.0).clamp(180.0, 720.0);
        let composer_x = available_rect.left() + ((available_rect.width() - composer_width) * 0.5);
        let composer_y = ui.cursor().min.y;
        let composer_rect = egui::Rect::from_min_size(
            egui::pos2(composer_x, composer_y),
            egui::vec2(composer_width, composer_height),
        );

        ui.scope_builder(egui::UiBuilder::new().max_rect(composer_rect), |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(255, 255, 255))
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgb(223, 227, 233),
                ))
                .corner_radius(egui::CornerRadius::same(18))
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    let button_width = 72.0;
                    let gap = 8.0;
                    let button_group_width = button_width;
                    let editor_width = (composer_width - button_group_width - gap - 24.0).max(96.0);

                    ui.horizontal(|ui| {
                        let text_response = ui.add_sized(
                            [editor_width, composer_height - 24.0],
                            egui::TextEdit::multiline(&mut app.draft_message)
                                .desired_rows(1)
                                .desired_width(composer_width - 88.0)
                                .hint_text("Message AuvroAI...")
                                .frame(true)
                                .id_salt("composer_input"),
                        );

                        let shift_held = ui.input(|i| i.modifiers.shift);
                        let submitted = text_response.has_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !shift_held;
                        if submitted {
                            app.draft_message = app.draft_message.trim_end_matches('\n').to_owned();
                            app.send_message();
                        }

                        ui.add_space(gap);

                        if app.is_loading {
                            let stop_clicked = ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Stop").color(egui::Color32::WHITE),
                                    )
                                    .fill(egui::Color32::from_rgb(180, 30, 30))
                                    .corner_radius(egui::CornerRadius::same(8))
                                    .min_size(egui::vec2(72.0, 40.0)),
                                )
                                .clicked();

                            if stop_clicked {
                                app.stop_streaming();
                            }
                        } else {
                            let send_clicked = ui
                                .add_enabled(
                                    can_send,
                                    egui::Button::new(egui::RichText::new("Send").strong())
                                        .min_size(egui::vec2(72.0, 40.0)),
                                )
                                .clicked();

                            if send_clicked {
                                app.send_message();
                            }
                        }
                    });
                });
        });

        ui.add_space(composer_height + 8.0);
    });

    render_avatar_menu(app, ui.ctx());
}
