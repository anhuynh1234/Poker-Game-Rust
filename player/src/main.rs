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
