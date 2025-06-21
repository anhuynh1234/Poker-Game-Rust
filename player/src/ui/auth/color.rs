use eframe::egui::Color32;

const HEX_COLOR: u32 = 0x181C14;

pub const BACKGROUND_COLOR: Color32 = Color32::from_rgb(
    (HEX_COLOR >> 16) as u8,       // Red
    (HEX_COLOR >> 8 & 0xFF) as u8, // Green
    (HEX_COLOR & 0xFF) as u8,      // Blue
);
