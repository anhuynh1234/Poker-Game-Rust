use crate::egui::RichText;
use crate::egui::ScrollArea;
use crate::ui::waiting::color::BACKGROUND_COLOR;
use crate::ui::waiting::color::HEADING_COLOR;
use crate::AppState;
use crate::PlayerApp;
use eframe::egui;
use eframe::egui::Align;
use eframe::egui::Frame;
use eframe::egui::Layout;
use serde_json::json;

/// Draws the "Ready" screen where the player can start the game or view stats.
///
/// - Allows the player to signal they are ready to play the game.
/// - Allows navigating to the stats page
/// - Allows player to spectate the current game
pub fn draw_ready(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(Frame::default().fill(BACKGROUND_COLOR))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                // Show logo
                if let Some(texture) = &app.logo_texture {
                    // Get available width of the panel
                    let available_width = ui.available_width();

                    // Desired width: e.g. 50% of available width
                    let desired_width = available_width * 0.3;

                    // Keep aspect ratio
                    let aspect_ratio = texture.size()[0] as f32 / texture.size()[1] as f32;
                    let desired_height = desired_width / aspect_ratio;

                    // Center horizontally
                    ui.vertical_centered(|ui| {
                        ui.image((texture.id(), egui::vec2(desired_width, desired_height)));
                    });

                    ui.add_space(20.0); // Optional vertical spacing
                }

                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.colored_label(HEADING_COLOR, RichText::new("Ready").heading().strong());
                    ui.add_space(20.0);
                });

                ui.separator();
                ui.add_space(20.0);

                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.vertical_centered(|ui| {
                        if ui.button("Ready").clicked() {
                            if let Some(tx) = &app.ui_to_net_tx {
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
            });
        });
}
