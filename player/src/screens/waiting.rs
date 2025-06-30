use crate::egui::RichText;
use crate::egui::ScrollArea;
use crate::ui::waiting::color::BACKGROUND_COLOR;
use crate::ui::waiting::color::HEADING_COLOR;
use crate::ui::waiting::images::load_poker_table;
use crate::AppState;
use crate::PlayerApp;
use eframe::egui;
use eframe::egui::Frame;
use serde_json::json;

/// Draws the "Ready" screen where the player can start the game or view stats.
///
/// - Allows the player to signal they are ready to play the game.
/// - Allows navigating to the stats page
/// - Allows player to spectate the current game
pub fn draw_ready(app: &mut PlayerApp, ctx: &egui::Context) {
    if app.table_texture.is_none() {
        app.table_texture = load_poker_table(ctx);
    }

    egui::CentralPanel::default()
        .frame(Frame::default().fill(BACKGROUND_COLOR))
        .show(ctx, |ui| {
            if let Some(texture) = &app.table_texture {
                let painter = ui.painter();
                let screen_rect = ui.max_rect();

                let screen_width = screen_rect.width();
                let screen_height = screen_rect.height();

                // Desired dimensions
                let img_width = screen_width * 0.8;
                let img_height = screen_height * 0.5;

                // Center the image in the panel
                let img_x = screen_rect.left() + (screen_width - img_width) / 2.0;
                let img_y = screen_rect.top() + (screen_height - img_height) / 2.0;

                let image_rect = egui::Rect::from_min_size(
                    egui::pos2(img_x, img_y),
                    egui::vec2(img_width, img_height),
                );

                painter.image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(
                        egui::Pos2::ZERO,
                        egui::Pos2::new(texture.size_vec2().x, texture.size_vec2().y),
                    ),
                    egui::Color32::WHITE,
                );
            }

            ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.colored_label(HEADING_COLOR, RichText::new("Ready").heading().strong());
                    ui.add_space(10.0);
                });

                if ui.button("Ready").clicked() {
                    if let Some(tx) = &app.ui_to_net_tx {
                        // Send a "button1" command.
                        let msg = json!({
                            "command": "ready",
                            "username": app.username
                        })
                        .to_string();

                        let _ = tx.send(msg);
                    }

                    app.state = AppState::InGame;
                }

                if ui.button("See Stats").clicked() {
                    if let Some(tx) = &app.ui_to_net_tx {
                        let msg = json!({
                            "command": "stats",
                        })
                        .to_string();

                        let _ = tx.send(msg);
                    }

                    app.state = AppState::Stats;
                }

                if ui.button("Spectate").clicked() {
                    app.state = AppState::Spectator;
                }
            });
        });
}
