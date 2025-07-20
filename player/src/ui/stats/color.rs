use eframe::egui::Color32;

const BACKGROUND_HEX_COLOR: u32 = 0x181C14;
const LIST_HEADER_HEX_COLOR: u32 = 0x886e4e;
const USER_ITEM_HEX_COLOR: u32 = 0xedf1ef;
const USER_NAME_HEX_COLOR: u32 = 0x67d0aa;

pub const HEADING_COLOR: Color32 = Color32::WHITE;

pub const BACKGROUND_COLOR: Color32 = Color32::from_rgb(
    (BACKGROUND_HEX_COLOR >> 16) as u8,       // Red
    (BACKGROUND_HEX_COLOR >> 8 & 0xFF) as u8, // Green
    (BACKGROUND_HEX_COLOR & 0xFF) as u8,      // Blue
);

pub const LIST_HEADER_COLOR: Color32 = Color32::from_rgb(
    (LIST_HEADER_HEX_COLOR >> 16) as u8,       // Red
    (LIST_HEADER_HEX_COLOR >> 8 & 0xFF) as u8, // Green
    (LIST_HEADER_HEX_COLOR & 0xFF) as u8,      // Blue
);

pub const USER_NAME_COLOR: Color32 = Color32::from_rgb(
    (USER_NAME_HEX_COLOR >> 16) as u8,       // Red
    (USER_NAME_HEX_COLOR >> 8 & 0xFF) as u8, // Green
    (USER_NAME_HEX_COLOR & 0xFF) as u8,      // Blue
);

pub const USER_ITEM_COLOR: Color32 = Color32::from_rgb(
    (USER_ITEM_HEX_COLOR >> 16) as u8,       // Red
    (USER_ITEM_HEX_COLOR >> 8 & 0xFF) as u8, // Green
    (USER_ITEM_HEX_COLOR & 0xFF) as u8,      // Blue
);
