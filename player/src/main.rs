//! # Poker Player Client
//!
//! This crate implements a GUI-based client for a selected poker game.
//!
//! Built using [`eframe`] and [`egui`] for the user interface, and TCP networking for communication with the dealer (server).
//!
//! ## Features
//! - TCP client-server communication
//! - Player registration & login
//! - Live in-game updates (bets, swaps, cards, pot, etc.)
//! - View player stats from the server
//! - User-friendly GUI using egui
//!
//! ## Usage
//! To run the application, execute:
//! ```bash
//! cargo run
//! ```
//!
//! To run tests:
//! ```bash
//! cargo test
//! ```
//!
//! ## Dependencies
//! - eframe (egui framework)
//! - serde_json (for JSON encoding/decoding)
//! - tokio
//! - std::net (for TCP streams)

mod app;
mod screens;
mod tests;
mod ui;

use app::PlayerApp;
use eframe::{egui, App, Frame, NativeOptions};
use serde_json::json;
use serde_json::Value;
use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

///maximum size of a network message buffer.
const MSG_SIZE: usize = 2048;

/// Modes for player authentication.
#[derive(Debug, PartialEq, Clone)]
pub enum Mode {
    Register,
    Login,
}

/// The overall application state.
#[derive(Debug, PartialEq, Clone)]
pub enum AppState {
    /// Authentication screen.
    Auth,
    /// Active game screen.
    InGame,
    /// Player statistics screen.
    Stats,
    /// Game ready screen (preparing to start).
    Ready,
    Spectator,
}

/// Draws the "Ready" screen where the player can start the game or view stats.
///
/// - Allows the player to signal they are ready to play the game.
/// - Allows navigating to the stats page
fn draw_ready(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Ready");
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
}

/// Draws the player statistics page.
///
/// - Displays a list of players.
/// - Allows searching for specific player statistics.
/// - Displays detailed user stats.
fn draw_stats_page(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Process incoming messages from the network thread.
        if let Some(rx) = &app.net_to_ui_rx {
            while let Ok(msg) = rx.try_recv() {
                // If the message contains the searched username, assume it's the stats reply.
                if !app.stats_search_query.is_empty() && msg.contains(&app.stats_search_query) {
                    app.user_stats = msg;
                } else {
                    // Otherwise, assume it's the list of users.
                    *app.output.lock().unwrap() = msg;
                }
            }
        }

        ui.heading("Stats Page");

        // Display the list of users.
        let list_output = app.output.lock().unwrap().clone();
        ui.label("List of Users:");
        for user in list_output.split(',').map(|s| s.trim()) {
            ui.label(user);
        }

        ui.separator();

        // Display the search bar.
        ui.horizontal(|ui| {
            ui.label("Search user:");
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
            ui.heading("User Stats:");
            // Try to parse the user stats JSON.
            match serde_json::from_str::<serde_json::Value>(&app.user_stats) {
                Ok(parsed) => {
                    let name = parsed.get("name").and_then(|v| v.as_str()).unwrap_or("N/A");
                    let wins = parsed.get("wins").and_then(|v| v.as_i64()).unwrap_or(0);
                    let losses = parsed.get("losses").and_then(|v| v.as_i64()).unwrap_or(0);
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

                    ui.label(format!("Name: {}", name));
                    ui.label(format!("Wins: {}", wins));
                    ui.label(format!("Losses: {}", losses));
                    ui.label(format!("Total Games Played: {}", games_played));
                    ui.label(format!("Money Won: {}", money_win));
                    ui.label(format!("Money Lost: {}", money_lost));
                }
                Err(_) => {
                    ui.label("Invalid user stats data.");
                }
            }
        }

        if ui.button("Exit").clicked() {
            app.state = AppState::Auth;
        }
    });
}

/// Draws the main in-game screen.
///
/// - Displays community cards, player hands, and game info.
/// - Allows placing bets and performing swaps.
/// - Shows game progression and current pot.
fn draw_in_game(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
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
                            ui.label(card_str);
                        }
                    }
                });
            }

            // Print cards
            if let Some(cards_obj) = parsed.get("cards") {
                if let Some(cards_map) = cards_obj.as_object() {
                    ui.separator();
                    ui.label("Table Hands:");

                    for (username, cards_val) in cards_map {
                        ui.horizontal(|ui| {
                            if username == &app.username {
                                ui.label(format!("{} (You):", username));
                                if let Some(card_array) = cards_val.as_array() {
                                    for card in card_array {
                                        if let Some(card_str) = card.as_str() {
                                            ui.label(card_str);
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
                                            ui.label(card_str);
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
                                                ui.label(card_str); // show the face-up card
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

        if ui.button("Exit Game").clicked() {
            app.state = AppState::Auth;
        }
    });
}

fn draw_spectator_page(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Spectator View");

        // Periodic polling every 2 seconds
        let now = Instant::now();
        if now
            .duration_since(app.last_spectate_request_time)
            .as_secs_f32()
            > 2.0
        {
            if let Some(tx) = &app.ui_to_net_tx {
                let msg = json!({ "command": "spectate" }).to_string();
                let _ = tx.send(msg);
            }
            app.last_spectate_request_time = now;
        }

        // Receive and store the most recent game state
        if let Some(rx) = &app.net_to_ui_rx {
            while let Ok(msg) = rx.try_recv() {
                *app.game_state.lock().unwrap() = msg;
            }
        }

        // Read and parse the current game state
        let state_str = app.game_state.lock().unwrap().clone();

        if let Ok(parsed) = serde_json::from_str::<Value>(&state_str) {
            if let Some(community) = parsed.get("community").and_then(|v| v.as_array()) {
                ui.separator();
                ui.heading("Community Cards:");
                ui.horizontal(|ui| {
                    for card in community {
                        if let Some(card_str) = card.as_str() {
                            ui.label(card_str);
                        }
                    }
                });
            }

            if let Some(cards_obj) = parsed.get("cards").and_then(|v| v.as_object()) {
                ui.separator();
                ui.heading("Player Hands:");
                for (username, cards_val) in cards_obj {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", username));
                        if let Some(card_array) = cards_val.as_array() {
                            for card in card_array {
                                if let Some(card_str) = card.as_str() {
                                    ui.label(card_str);
                                }
                            }
                        }
                    });
                }
            }

            if let Some(cards_obj) = parsed.get("7 card hands") {
                if let Some(cards_map) = cards_obj.as_object() {
                    ui.separator();
                    ui.heading("7 Card Stud Hands:");
                    for (username, cards_val) in cards_map {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", username));
                            if let Some(card_array) = cards_val.as_array() {
                                for (_i, card) in card_array.iter().enumerate() {
                                    if let Some(card_str) = card.as_str() {
                                        ui.label(card_str);
                                    }
                                }
                            }
                        });
                    }
                }
            }

            if let Some(round_bet) = parsed.get("round current bet").and_then(|v| v.as_i64()) {
                ui.separator();
                ui.label(format!("Round Current Bet: {}", round_bet));
            }

            if let Some(bets_map) = parsed
                .get("player current bets")
                .and_then(|v| v.as_object())
            {
                ui.separator();
                ui.heading("Player Bets This Round:");
                for (username, value) in bets_map {
                    if let Some(amount) = value.as_i64() {
                        ui.label(format!("{}: {}", username, amount));
                    }
                }
            }

            if let Some(info) = parsed.get("info").and_then(|v| v.as_str()) {
                ui.separator();
                ui.label(info);
            }

            if let Some(mut pot) = parsed.get("pot").and_then(|v| v.as_i64()) {
                if pot < 0 {
                    pot = 0;
                }
                ui.label(format!("Pot: {}", pot));
            }

            if let Some(winner) = parsed.get("winner").and_then(|v| v.as_str()) {
                ui.separator();
                ui.label(format!("Winner is: {}", winner));
            }
        } else {
            ui.label("Waiting for valid game state...");
        }

        if ui.button("Exit Game").clicked() {
            app.state = AppState::Auth;
        }
    });
}

fn main() {
    let native_options = NativeOptions::default();
    let _ = eframe::run_native(
        "Poker Player",
        native_options,
        Box::new(|_cc| Box::new(PlayerApp::default())),
    );
}
