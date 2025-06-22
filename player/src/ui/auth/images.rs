use crate::egui::ColorImage;
use crate::egui::TextureHandle;
use crate::egui::TextureOptions;
use eframe::egui;
use image::GenericImageView;

pub fn load_poker_logo(ctx: &egui::Context) -> Option<TextureHandle> {
    match image::open("src/ui/auth/images/poker_game_logo.png") {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = img.dimensions();
            let pixels = rgba.into_raw();
            let color_image =
                ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);
            Some(ctx.load_texture("poker_game_logo", color_image, TextureOptions::default()))
        }
        Err(e) => {
            eprintln!("Failed to load image: {}", e);
            None
        }
    }
}
