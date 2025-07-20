use crate::ui::cards::images::load_card_texture;
use crate::ui::in_game::color::*;
use crate::AppState;
use crate::PlayerApp;
use eframe::egui;
use eframe::egui::Align;
use eframe::egui::Frame;
use eframe::egui::Layout;
use eframe::egui::ScrollArea;
use serde_json::json;
use serde_json::Value;

/// Draws the main in-game screen.
///
/// - Displays community cards, player hands, and game info.
/// - Allows placing bets and performing swaps.
/// - Shows game progression and current pot.
pub fn draw_in_game(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
    .frame(Frame::default().fill(BACKGROUND_COLOR))
    .show(ctx, |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            let current_bet = &mut app.current_bet;

            ui.heading("In-Game");
            ui.label("Game is running...");

            if let Some(rx) = &app.net_to_ui_rx {
                // Drain the channel, updating app.output with the most recent message.
                while let Ok(msg) = rx.try_recv() {
                    *app.output.lock().unwrap() = msg;
                }
            }

            let output = app.output.lock().unwrap().clone();
            if let Ok(parsed) = serde_json::from_str::<Value>(&output) {
                // Print community cards
                if let Some(community) = parsed.get("community").and_then(|v| v.as_array()) {
                    ui.separator();
                    ui.heading("Community Cards:");
                    ui.horizontal(|ui| {
                        for card in community {
                            if let Some(card_str) = card.as_str() {
                                if let Some(texture) = load_card_texture(ctx, card_str) {
                                    ui.image((texture.id(), egui::vec2(60.0, 100.0)));
                                }
                            }
                        }
                    });
                }

                // Print cards
                if let Some(cards_obj) = parsed.get("cards") {
                    if let Some(cards_map) = cards_obj.as_object() {
                        ui.separator();
                        ui.label("Table Hands:");
                        // {"cards":{"q":["5 of Diamonds","Q of Spades"],"w":["J of Spades","6 of Hearts"]}

                        for (username, cards_val) in cards_map {
                            ui.horizontal(|ui| {
                                if username == &app.username {
                                    ui.label(format!("{} (You):", username));
                                    if let Some(card_array) = cards_val.as_array() {
                                        for card in card_array {
                                            if let Some(card_str) = card.as_str() {
                                                if let Some(texture) = load_card_texture(ctx, card_str) {
                                                    ui.image((texture.id(), egui::vec2(60.0, 100.0)));
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // For other players, just show blanks or hidden cards
                                    ui.label(format!("{}:", username));
                                    if let Some(card_array) = cards_val.as_array() {
                                        for _ in card_array {
                                            ui.label("X");
                                        }
                                    }
                                }
                            });
                        }
                    }
                }

                // Print 7 card hands
                if let Some(cards_obj) = parsed.get("7 card hands") {
                    if let Some(cards_map) = cards_obj.as_object() {
                        ui.separator();
                        ui.label("Hands:");

                        for (username, cards_val) in cards_map {
                            ui.horizontal(|ui| {
                                if username == &app.username {
                                    // Show all 3 cards to the player
                                    ui.label(format!("{} (You):", username));
                                    if let Some(card_array) = cards_val.as_array() {
                                        for card in card_array {
                                            if let Some(card_str) = card.as_str() {
                                                if let Some(texture) = load_card_texture(ctx, card_str) {
                                                    ui.image((texture.id(), egui::vec2(60.0, 100.0)));
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // Show only the third card (index 2) to others
                                    ui.label(format!("{}:", username));
                                    if let Some(card_array) = cards_val.as_array() {
                                        for (i, card) in card_array.iter().enumerate() {
                                            if i == 2 || i == 3 || i == 4 || i == 5 {
                                                if let Some(card_str) = card.as_str() {
                                                    if let Some(texture) = load_card_texture(ctx, card_str) {
                                                    ui.image((texture.id(), egui::vec2(60.0, 100.0)));
                                                }// show the face-up card
                                                }
                                            } else {
                                                ui.label("X"); // hidden cards
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                }

                // Prompt user to bet
                if let Some(bet_turn) = parsed.get("bet").and_then(|v| v.as_str()) {
                    if bet_turn == app.username {
                        ui.separator();
                        ui.label("It's your turn to bet!");

                        // Input box for bet amount
                        ui.horizontal(|ui| {
                            ui.label("Enter your bet, enter 0 to check, -1 to fold:");
                            ui.text_edit_singleline(current_bet);
                            if ui.button("Place Bet").clicked() {
                                if let Some(tx) = &app.ui_to_net_tx {
                                    let bet_msg = json!({
                                        "command": "bet",
                                        "username": app.username,
                                        "amount": current_bet.trim().parse::<i32>().unwrap_or(0)
                                    })
                                    .to_string();
                                    let _ = tx.send(bet_msg);
                                }
                            }
                        });
                    }
                }

                // Print extra info
                if let Some(info) = parsed.get("info").and_then(|v| v.as_str()) {
                    ui.separator();
                    ui.label(info);
                }

                // Promp user to swap cards
                if let Some(swap_turn) = parsed.get("swap").and_then(|v| v.as_str()) {
                    if swap_turn == app.username {
                        ui.separator();
                        ui.label("It's your turn to swap!");
                        ui.label("Enter the indices of the cards you want to swap (comma-separated, starting from 0).");

                        // Input field for swap indices
                        ui.horizontal(|ui| {
                            ui.label("Swap indices:");
                            ui.text_edit_singleline(&mut app.current_swap); // reuse current_bet for simplicity
                            if ui.button("Submit Swap").clicked() {
                                if let Some(tx) = &app.ui_to_net_tx {
                                    let swap_msg = json!({
                                        "command": "swap",
                                        "username": app.username,
                                        "indices": app.current_swap.trim() // send string, server will parse
                                    })
                                    .to_string();
                                    let _ = tx.send(swap_msg);
                                }
                            }
                        });
                    }
                }

                // showing the current pot amount
                if let Some(pot) = parsed.get("pot").and_then(|v| v.as_i64()) {
                    ui.label(format!("Pot: {}", pot));
                }

                // showing the current round bet
                if let Some(mut curr_bet) = parsed.get("round current bet").and_then(|v| v.as_i64()) {
                    if curr_bet < 0 {
                        curr_bet = 0;
                    }
                    ui.label(format!("Round current bet: {}", curr_bet));
                }

                // showing player bet amount
                if let Some(bet_amounts_map) = parsed.get("player bet amount").and_then(|v| v.as_object()) {
                    ui.separator();
                    ui.label("Player Bet Amounts:");
                    for (username, value) in bet_amounts_map {
                        if let Some(amount) = value.as_i64() {
                            if username == &app.username {
                                ui.label(format!("{} (You): {}", username, amount));
                            } else {
                                ui.label(format!("{}: {}", username, amount));
                            }
                        }
                    }
                }

                // showing all hands for all players
                if let Some(hands_obj) = parsed.get("showdown").and_then(|v| v.as_object()) {
                    ui.label("Player Hands:");
                    for (username, hand_value) in hands_obj.iter() {
                        let hand_str = if username == &app.username {
                            // For yourself, show the actual cards with a clear label
                            format!("> You ({}) | Hand: {}", username, hand_value)
                        } else {
                            // For other players, display their name and hand neatly
                            format!("  {} | Hand: {}", username, hand_value)
                        };
                        ui.label(hand_str);
                    }
                }
                // Showing winner
                if let Some(winner) = parsed.get("winner").and_then(|v| v.as_str()) {
                    ui.label(format!("Winner is {}", winner));
                }
            } else {
                ui.label(format!("Server: {}", output));
            }

            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.vertical_centered(|ui| {
                    if ui.button("Exit Game").clicked() {
                        app.state = AppState::Auth;
                    }
                });
            });
        });
    });
}
