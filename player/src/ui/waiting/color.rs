use eframe::egui::Color32;

const BACKGROUND_HEX_COLOR: u32 = 0x181C14;

pub const HEADING_COLOR: Color32 = Color32::WHITE;

pub const BACKGROUND_COLOR: Color32 = Color32::from_rgb(
    (BACKGROUND_HEX_COLOR >> 16) as u8,       // Red
    (BACKGROUND_HEX_COLOR >> 8 & 0xFF) as u8, // Green
    (BACKGROUND_HEX_COLOR & 0xFF) as u8,      // Blue
);
