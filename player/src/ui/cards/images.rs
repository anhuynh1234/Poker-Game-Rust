use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use image::GenericImageView;

/// Load a card image based on string like "Q of Spades" or "5 of Diamonds"
pub fn load_card_texture(ctx: &Context, hand_str: &str) -> Option<TextureHandle> {
    // Convert face cards to full names
    let rank_map = [("A", "ace"), ("K", "king"), ("Q", "queen"), ("J", "jack")];

    // Split "Q of Spades" into ["Q", "Spades"]
    let parts: Vec<&str> = hand_str.split_whitespace().collect();
    if parts.len() != 3 || parts[1].to_lowercase() != "of" {
        eprintln!("Invalid card format: '{}'", hand_str);
        return None;
    }

    let raw_rank = parts[0];
    let suit = parts[2];

    // Normalize rank
    let rank = rank_map
        .iter()
        .find(|(k, _)| *k == raw_rank)
        .map(|(_, v)| *v)
        .unwrap_or(raw_rank) // Use number as-is
        .to_lowercase();

    let suit = suit.to_lowercase();

    let filename = format!("src/ui/cards/images/{}_of_{}.png", rank, suit);

    match image::open(&filename) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = img.dimensions();
            let pixels = rgba.into_raw();
            let color_image =
                ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);

            Some(ctx.load_texture(&filename, color_image, TextureOptions::default()))
        }
        Err(e) => {
            eprintln!("Failed to load card '{}': {}", filename, e);
            None
        }
    }
}
