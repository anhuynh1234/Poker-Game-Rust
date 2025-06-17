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

use eframe::{egui, App, Frame, NativeOptions};
use serde_json::json;
use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant}
};
use serde_json::Value;

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
    Spectator
}

pub struct PlayerApp {
    pub username: String,
    pub password: String,
    pub dealer_ip: String,
    pub mode: Mode,
    pub stats_search_query: String,
    pub user_stats: String,
    pub current_bet: String,
    pub current_swap: String,
    pub state: AppState,
    /// Stores the server response from the one-shot auth command.
    pub output: Arc<Mutex<String>>,
    /// Stores the server response for game state requests for spectator mode.
    pub game_state: Arc<Mutex<String>>,
    pub last_spectate_request_time: Instant,
    /// Channel to send messages from the UI to the network thread.
    pub ui_to_net_tx: Option<Sender<String>>,
    /// Channel to receive messages from the network thread.
    pub net_to_ui_rx: Option<Receiver<String>>,
}

impl Default for PlayerApp {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            dealer_ip: "127.0.0.1".to_string(),
            mode: Mode::Login,
            stats_search_query: String::new(),
            user_stats: String::new(),
            current_bet: "0".to_string(),
            current_swap: "".to_string(),
            state: AppState::Auth,
            output: Arc::new(Mutex::new(String::new())),
            last_spectate_request_time: Instant::now(),
            game_state: Arc::new(Mutex::new(String::new())),
            ui_to_net_tx: None,
            net_to_ui_rx: None,
        }
    }
}

impl App for PlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        match self.state {
            AppState::Auth => draw_auth_screen(self, ctx),
            AppState::InGame => draw_in_game(self, ctx),
            AppState::Ready => draw_ready(self, ctx),
            AppState::Stats => draw_stats_page(self, ctx),
            AppState::Spectator => draw_spectator_page(self, ctx)
        }
        ctx.request_repaint();
    }
}

/// Draws the authentication screen with login/register options.
///
/// - Allows players to enter username, password, and server IP.
/// - Spawns an authentication network thread upon submission.
fn draw_auth_screen(app: &mut PlayerApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Poker Player - Login / Register");

        ui.horizontal(|ui| {
            ui.radio_value(&mut app.mode, Mode::Register, "Register");
            ui.radio_value(&mut app.mode, Mode::Login, "Login");
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Username:");
            ui.text_edit_singleline(&mut app.username);
        });
        ui.horizontal(|ui| {
            ui.label("Password:");
            ui.text_edit_singleline(&mut app.password);
        });
        ui.horizontal(|ui| {
            ui.label("Dealer IP:");
            ui.text_edit_singleline(&mut app.dealer_ip);
        });

        if ui.button("Submit").clicked() {
            let username = app.username.clone();
            let password = app.password.clone();
            let mode = app.mode.clone();
            let dealer_ip = app.dealer_ip.clone();
            let output_ref = Arc::clone(&app.output);

            // Spawn a thread to send the one-shot auth command.
            thread::spawn(move || {
                match TcpStream::connect(format!("{}:8080", dealer_ip)) {

                    Ok(mut stream) => {
                        let command = match mode {
                            Mode::Register => "register",
                            Mode::Login => "login",
                        };

                        println!("[Client] Selected command: {}", command);
                        println!("[Client] Sending credentials - username: {}, password: {}", username, password);

                        let message = json!({
                            "command": command,
                            "username": username,
                            "password": password,
                        })
                        .to_string();

                        println!("[Client] Sending JSON: {}", message);

                        if stream.write_all(message.as_bytes()).is_ok() {
                            println!("[Client] Message sent successfully.");
                    
                            let mut buffer = [0; 512];
                            match stream.read(&mut buffer) {
                                Ok(size) => {
                                    let reply = String::from_utf8_lossy(&buffer[..size]).to_string();
                                    println!("[Client] Received response ({} bytes): {}", size, reply);
                                    *output_ref.lock().unwrap() = reply;
                                }
                                Err(e) => {
                                    println!("[Client] Error reading from server: {}", e);
                                    *output_ref.lock().unwrap() = format!("Error reading from server: {}", e);
                                }
                            }
                        } else {
                            println!("[Client] Failed to send message to server.");
                            *output_ref.lock().unwrap() = "Failed to send message.".to_string();
                        }
                    }
                    Err(e) => {
                        *output_ref.lock().unwrap() =
                            format!("Connection failed: {}", e);
                    }
                }
            });
        }

        ui.separator();
        let reply = app.output.lock().unwrap().clone();
        ui.label(format!("Server response: {}", reply));

        // If the server response indicates a successful login, spawn the persistent thread.
        if reply.contains("game") || reply.contains("Welcome") {
            // Create channels for communication.
            let (ui_to_net_tx, ui_to_net_rx) = mpsc::channel::<String>();
            let (net_to_ui_tx, net_to_ui_rx) = mpsc::channel::<String>();
            let dealer_ip = app.dealer_ip.clone();
            let username = app.username.clone();
            // Spawn one background thread for persistent communication.
            thread::spawn(move || {
                // Connect persistently.
                let mut client = TcpStream::connect(format!("{}:8080", dealer_ip))
                    .expect("Failed to connect persistently");
                client
                    .set_nonblocking(true)
                    .expect("Failed to set non-blocking");
                loop {
                    // Read from the server.
                    let mut buff = vec![0; MSG_SIZE];
                    match client.read(&mut buff) {
                        Ok(_) => {
                            let msg_bytes = buff
                                .into_iter()
                                .take_while(|&x| x != 0)
                                .collect::<Vec<u8>>();
                            if let Ok(msg_str) = String::from_utf8(msg_bytes) {
                                if net_to_ui_tx.send(msg_str).is_err() {
                                    break;
                                }
                            }
                        }
                        Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
                        Err(_) => {
                            println!("Persistent connection severed");
                            break;
                        }
                    }
                    // Check if UI sent a message.
                    match ui_to_net_rx.try_recv() {
                        Ok(msg) => {
                            let mut buff = msg.into_bytes();
                            buff.resize(MSG_SIZE, 0);
                            if let Err(e) = client.write_all(&buff) {
                                println!("Failed to send message: {}", e);
                            }
                        }
                        Err(TryRecvError::Empty) => (),
                        Err(TryRecvError::Disconnected) => break,
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            });

            // Save the channel handles in the app.
            app.ui_to_net_tx = Some(ui_to_net_tx);
            app.net_to_ui_rx = Some(net_to_ui_rx);
            // Transition state.
            app.state = AppState::Ready;
            *app.output.lock().unwrap() = String::new();
        }
    });
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
                }).to_string();
                
                let _ = tx.send(msg);
            }

            app.state = AppState::InGame;
        }

        if ui.button("See Stats").clicked() {
            if let Some(tx) = &app.ui_to_net_tx {
                let msg = json!({
                    "command": "stats",
                }).to_string();
                
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
                    let games_played = parsed.get("games_played").and_then(|v| v.as_i64()).unwrap_or(0);
                    let money_win = parsed.get("money_win").and_then(|v| v.as_i64()).unwrap_or(0);
                    let money_lost = parsed.get("money_lost").and_then(|v| v.as_i64()).unwrap_or(0);

                    ui.label(format!("Name: {}", name));
                    ui.label(format!("Wins: {}", wins));
                    ui.label(format!("Losses: {}", losses));
                    ui.label(format!("Total Games Played: {}", games_played));
                    ui.label(format!("Money Won: {}", money_win));
                    ui.label(format!("Money Lost: {}", money_lost));
                },
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
        if now.duration_since(app.last_spectate_request_time).as_secs_f32() > 2.0 {
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
                                for (i, card) in card_array.iter().enumerate() {
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

            if let Some(bets_map) = parsed.get("player current bets").and_then(|v| v.as_object()) {
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



#[cfg(test)]
mod tests {
    use super::*;
    // use std::sync::{Arc, Mutex};
    // use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

    /// 1) Ensure PlayerApp::default() yields expected default values.
    #[test]
    fn test_default_player_app() {
        let app = PlayerApp::default();
        assert_eq!(app.username, "");
        assert_eq!(app.password, "");
        assert_eq!(app.dealer_ip, "127.0.0.1");
        assert_eq!(app.mode, Mode::Login);
        assert_eq!(app.state, AppState::Auth);
        assert_eq!(*app.output.lock().unwrap(), "");
    }

    /// 2) mode switching from Login to Register, etc.
    #[test]
    fn test_mode_switch() {
        let mut app = PlayerApp::default(); //default is login
        assert_eq!(app.mode, Mode::Login);

        app.mode = Mode::Register; // switching to register 
        assert_eq!(app.mode, Mode::Register);

        app.mode = Mode::Login; // switching back to login 
        assert_eq!(app.mode, Mode::Login);
    }

    /// 3) username & password can be input into the text fields and read back
    #[test]
    fn test_text_field_input() {
        let mut app = PlayerApp::default();
        app.username = "test".to_string();
        app.password = "testpw".to_string();
        assert_eq!(app.username, "test");
        assert_eq!(app.password, "testpw");
    }

    /// 4) transitioning to ready state
    #[test]
    fn test_auth_success_transition() {
        let mut app = PlayerApp::default();
        let (tx_ui, _rx_net) = mpsc::channel();
        let (_tx_net, rx_ui) = mpsc::channel();

        app.ui_to_net_tx = Some(tx_ui);
        app.net_to_ui_rx = Some(rx_ui);
        *app.output.lock().unwrap() = "Welcome test_user, you are now in the game.".to_string();

        // simulate the auth success block
        if app.output.lock().unwrap().contains("Welcome") {
            app.state = AppState::Ready;
            *app.output.lock().unwrap() = String::new();
        }

        assert_eq!(app.state, AppState::Ready);
        assert_eq!(*app.output.lock().unwrap(), "");
    }

    /// Tests state transition when exiting the game.
    #[test]
    fn test_exit_game_resets_state() {
        let mut app = PlayerApp::default();
        app.state = AppState::InGame;

        app.state = AppState::Auth; // pressing "exit game" brings back to auth screen
        assert_eq!(app.state, AppState::Auth); 
    }

    #[test]
    fn test_state_transitions() {
        let mut app = PlayerApp::default();

        app.state = AppState::Ready;
        assert_eq!(app.state, AppState::Ready);

        app.state = AppState::Stats;
        assert_eq!(app.state, AppState::Stats);

        app.state = AppState::InGame;
        assert_eq!(app.state, AppState::InGame);

        app.state = AppState::Spectator;
        assert_eq!(app.state, AppState::Spectator);

        app.state = AppState::Auth;
        assert_eq!(app.state, AppState::Auth);
    }

    #[test]
    fn test_stats_search_query_input() {
        let mut app = PlayerApp::default();
        app.stats_search_query = "player123".to_string();
        assert_eq!(app.stats_search_query, "player123");
    }

    #[test]
    fn test_user_stats_parsing() {
        let mut app = PlayerApp::default();
        let json_data = r#"{
            "name": "testuser",
            "wins": 5,
            "losses": 3,
            "games_played": 8,
            "money_win": 1200,
            "money_lost": 800
        }"#;

        app.stats_search_query = "testuser".to_string();
        app.user_stats = json_data.to_string();

        let parsed: Value = serde_json::from_str(&app.user_stats).unwrap();
        assert_eq!(parsed["name"], "testuser");
        assert_eq!(parsed["wins"], 5);
        assert_eq!(parsed["games_played"], 8);
    }

    #[test]
    fn test_network_channels_set_after_login() {
        let mut app = PlayerApp::default();
        assert!(app.ui_to_net_tx.is_none());
        assert!(app.net_to_ui_rx.is_none());

        let (tx_ui, _rx_net) = mpsc::channel();
        let (_tx_net, rx_ui) = mpsc::channel();

        app.ui_to_net_tx = Some(tx_ui);
        app.net_to_ui_rx = Some(rx_ui);

        assert!(app.ui_to_net_tx.is_some());
        assert!(app.net_to_ui_rx.is_some());
    }

    #[test]
    fn test_current_bet_and_swap_input() {
        let mut app = PlayerApp::default();
        app.current_bet = "10".to_string();
        app.current_swap = "0,2,4".to_string();

        assert_eq!(app.current_bet, "10");
        assert_eq!(app.current_swap, "0,2,4");
    }

}
