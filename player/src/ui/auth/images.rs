use crate::egui;
use image::GenericImageView;

pub const POKER_LOGO_TEXTURE: Option<egui::TextureHandle> =
    image::open("assets/logo.png").ok().map(|img| {
        let rgba = img.to_rgba8();
        let (width, height) = img.dimensions();
        let pixels = rgba.into_raw();
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);
        ctx.load_texture("logo", color_image, egui::TextureOptions::default())
    });
