use eframe::egui;

pub mod auth;
pub mod chat;
pub mod settings;
pub mod sidebar;

pub(crate) fn render_app_logo(ui: &mut egui::Ui, size: f32) {
	let texture = app_logo_texture(ui.ctx());
	let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
	ui.painter().image(
		texture.id(),
		rect,
		egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
		egui::Color32::WHITE,
	);
}

fn app_logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
	let texture_id = egui::Id::new("auvro_app_logo_texture");

	if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(texture_id)) {
		return texture;
	}

	let color_image = match image::load_from_memory(include_bytes!("../../assets/icons/Auvro.png")) {
		Ok(image) => {
			let image = image.to_rgba8();
			let size = [image.width() as usize, image.height() as usize];
			let pixels = image.into_raw();
			egui::ColorImage::from_rgba_unmultiplied(size, &pixels)
		}
		Err(_) => egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]),
	};
	let texture = ctx.load_texture("auvro_app_logo", color_image, egui::TextureOptions::LINEAR);

	ctx.data_mut(|data| data.insert_temp(texture_id, texture.clone()));
	texture
}
