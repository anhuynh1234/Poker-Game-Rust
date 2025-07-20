use crate::egui::RichText;
use crate::egui::ScrollArea;
use crate::ui::stats::color::*;
use crate::AppState;
use crate::PlayerApp;
use eframe::egui;
use eframe::egui::Align;
use eframe::egui::Frame;
use eframe::egui::Layout;
use egui::{Color32, Vec2};

/// Draws the player statistics page.
/// - Displays a list of players.
/// - Allows searching for specific player statistics.
/// - Displays detailed user stats.
pub fn draw_stats_page(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(Frame::default().fill(BACKGROUND_COLOR))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                // Process incoming messages from the network thread.
                if let Some(rx) = &app.net_to_ui_rx {
                    while let Ok(msg) = rx.try_recv() {
                        // If the message contains the searched username, assume it's the stats reply.
                        if !app.stats_search_query.is_empty()
                            && msg.contains(&app.stats_search_query)
                        {
                            app.user_stats = msg;
                        } else {
                            // Otherwise, assume it's the list of users.
                            *app.output.lock().unwrap() = msg;
                        }
                    }
                }

                ui.vertical_centered(|ui| {
                    ui.colored_label(
                        HEADING_COLOR,
                        RichText::new("Stats Page").heading().strong(),
                    );
                    ui.add_space(10.0);
                });

                ui.group(|ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), 0.0)); // Optional width

                    ui.vertical_centered(|ui| {
                        ui.colored_label(
                            LIST_HEADER_COLOR,
                            RichText::new("Users").heading().strong(),
                        );
                        ui.add_space(10.0);
                    });

                    let list_output = app.output.lock().unwrap().clone();
                    for user in list_output.split(',').map(|s| s.trim()) {
                        ui.label(RichText::new(format!("    - {}", user)).color(Color32::WHITE));
                    }
                });

                ui.separator();

                // Display the search bar.
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search user:").color(LIST_HEADER_COLOR));
                    ui.text_edit_singleline(&mut app.stats_search_query);
                    if ui.button("Get").clicked() {
                        if let Some(tx) = &app.ui_to_net_tx {
                            // Send a JSON message to the server to request stats for the specified user.
                            let msg = serde_json::json!({
                                "command": "get_user_stats",
                                "username": app.stats_search_query,
                            })
                            .to_string();
                            let _ = tx.send(msg);
                        }
                    }
                });

                ui.separator();

                // Display the searched user's stats if available.
                if !app.user_stats.is_empty() {
                    ui.group(|ui| {
                        ui.set_min_size(Vec2::new(ui.available_width(), 0.0));
                        ui.label(
                            RichText::new("User Stats:")
                                .strong()
                                .underline()
                                .color(USER_NAME_COLOR),
                        );

                        match serde_json::from_str::<serde_json::Value>(&app.user_stats) {
                            Ok(parsed) => {
                                let name =
                                    parsed.get("name").and_then(|v| v.as_str()).unwrap_or("N/A");
                                let wins = parsed.get("wins").and_then(|v| v.as_i64()).unwrap_or(0);
                                let losses =
                                    parsed.get("losses").and_then(|v| v.as_i64()).unwrap_or(0);
                                let games_played = parsed
                                    .get("games_played")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                let money_win = parsed
                                    .get("money_win")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                let money_lost = parsed
                                    .get("money_lost")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);

                                ui.label(
                                    RichText::new(format!("• Name: {}", name))
                                        .color(USER_ITEM_COLOR),
                                );
                                ui.label(
                                    RichText::new(format!("• Wins: {}", wins))
                                        .color(USER_ITEM_COLOR),
                                );
                                ui.label(
                                    RichText::new(format!("• Losses: {}", losses))
                                        .color(USER_ITEM_COLOR),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "• Total Games Played: {}",
                                        games_played
                                    ))
                                    .color(USER_ITEM_COLOR),
                                );
                                ui.label(
                                    RichText::new(format!("• Money Won: ${}", money_win))
                                        .color(Color32::GREEN),
                                );
                                ui.label(
                                    RichText::new(format!("• Money Lost: ${}", money_lost))
                                        .color(Color32::RED),
                                );
                            }
                            Err(_) => {
                                ui.label(
                                    RichText::new("Invalid user stats data.").color(Color32::RED),
                                );
                            }
                        }
                    });
                }

                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.vertical_centered(|ui| {
                        if ui.button("Exit").clicked() {
                            app.state = AppState::Auth;
                        }
                    });
                });
            });
        });
}
