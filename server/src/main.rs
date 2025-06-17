//! # Server Main Module
//!
//! This is the main entry point for the server application.
//! It manages:
//! - Client connections over TCP
//! - User registration and login
//! - Lobby and player management
//! - Game variant selection and game start
//! - Command handling from clients
//!
//! The server uses Tokio for asynchronous operations and MongoDB for persistent player data storage.
//! Supported game modes include:
//! - 5 Card Draw
//! - 7 Card Stud
//! - Texas Hold'em
//!
//! The server starts by setting game configuration, binding to port 8080, and
//! waits for player connections. Once the configured number of players join,
//! the selected game variant is launched in a separate async task.

mod db;
mod user_info;
mod comms;
mod five_card_game;
mod deck;
mod five_card_draw;
mod seven_card_stud;
mod texas_holdem;
mod texas_game;
mod seven_card_game;

use std::{
    collections::HashMap,
    io::ErrorKind,
    net::TcpListener,
    sync::mpsc::{self, Sender},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use mongodb::bson::doc;
use serde_json::Value;
use std::sync::OnceLock;
use db::*;
use user_info::*;
use comms::*;
use five_card_game::*;
use texas_game::*;
use seven_card_game::*;

/// Number of players in the game, set at startup.
static NUM_PLAYERS: OnceLock<usize> = OnceLock::new();
/// Selected game variant, set at startup.
static GAME_VARIANT: OnceLock<String> = OnceLock::new();
/// Map of active game players and their socket addresses.
static GAME_PLAYERS: OnceLock<Arc<Mutex<HashMap<String, std::net::SocketAddr>>>> = OnceLock::new();

/// Buffer size for incoming messages.
const MSG_SIZE: usize = 2048;

#[derive(Debug)]
struct ClientInfo {
    addr: std::net::SocketAddr,
    sender: Sender<String>, // for future broadcasting
}
/// Main function to start the server.
///
/// - Initializes game configuration.
/// - Connects to MongoDB.
/// - Starts TCP listener on port 8080.
/// - Spawns new thread for each client.
/// - Handles client commands (register, login, ready, etc.).
#[tokio::main]
async fn main() {
    setup_game_config();

    let player_list = Arc::new(Mutex::new(HashMap::new()));
    GAME_PLAYERS.set(Arc::clone(&player_list)).unwrap();

    let (players_collection, lobbies_collection, games_collection, history_collection) =
        init_db().await.expect("Failed to connect to MongoDB");
    let players_collection = Arc::new(players_collection);
    let lobbies_collection = Arc::new(lobbies_collection);
    let games_collection = Arc::new(games_collection);
    let history_collection = Arc::new(history_collection);
    let _ = init_game_state(&games_collection).await;

    println!("[Server] Connected to MongoDB and initialized collections.");

    let server_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    let server_addr = format!("{}:8080", server_ip);
    let server = TcpListener::bind("0.0.0.0:8080").expect("Listener failed to bind");
    server.set_nonblocking(true).expect("Failed to initialize non-blocking");

    let clients: Arc<Mutex<HashMap<std::net::SocketAddr, ClientInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    let (server_tx, server_rx) = mpsc::channel::<(std::net::SocketAddr, String)>();

    println!("Server listening on {}", server_addr);

    loop {
        // Accept new clients
        match server.accept() {
            Ok((socket, addr)) => {
                println!("Client connected: {}", addr);

                let (client_tx, client_rx) = mpsc::channel::<String>();
                let server_tx_clone = server_tx.clone();
                let clients_clone = Arc::clone(&clients);

                // Register the client
                clients.lock().unwrap().insert(
                    addr,
                    ClientInfo {
                        addr,
                        sender: client_tx.clone(),
                    },
                );
                println!(
                    "[Server] Inserting client: addr = {}, sender = mpsc::Sender", addr);

                // Handle client in separate thread
                thread::spawn(move || handle_client(socket, addr, server_tx_clone, client_rx));
            }
            Err(ref err) if err.kind() == ErrorKind::WouldBlock => {} // no new connections
            Err(e) => {
                eprintln!("Error accepting client: {}", e);
            }
        }

        // Process messages received from clients
        match server_rx.try_recv() {
            Ok((addr, msg)) => {
                println!("[{}] {}", addr, msg);

                // Parse and respond to commands (simplified example)
                if let Ok(json) = serde_json::from_str::<Value>(&msg) {
                    if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                        match cmd {
                            "register" => {
                                println!("{} is registering", addr);
                                let response = handle_registration(&players_collection, &msg).await;
                                send_to_client(&clients, &addr, &response);
                            }
                            "login" => {
                                println!("{} is logging in", addr);
                                let response = handle_login(&players_collection, &msg).await;
                                send_to_client(&clients, &addr, &response);
                            }
                            "ready" => {
                                if let Some(json) = serde_json::from_str::<Value>(&msg).ok() {
                                    if let Some(username) = json.get("username").and_then(|v| v.as_str()) {

                                        let players = GAME_PLAYERS.get().unwrap();
                                        let mut game_players = players.lock().unwrap();
                                        let max_players = *NUM_PLAYERS.get().unwrap();

                                        if game_players.len() < max_players {
                                            if !game_players.contains_key(&username.to_string()) {
                                                game_players.insert(username.to_string(), addr);
                                                println!("[Game] {} added to game player list.", username);
                                                send_to_client(&clients, &addr, &format!("Welcome {}, you are now in the game.", username));

                                                if game_players.len() == max_players {
                                                    println!("[Game] All players joined. Spawning game thread...");
                                                
                                                    let game_clients = Arc::clone(&clients);
                                                    let players_for_game = game_players.keys().cloned().collect();
                                                    
                                                    let pc = Arc::clone(&players_collection);
                                                    let lc = Arc::clone(&lobbies_collection);
                                                    let gc = Arc::clone(&games_collection);
                                                    let hc = Arc::clone(&history_collection);
                                                    
                                                    println!("[Game] Spawning game thread now...");
                                                    let variant = GAME_VARIANT.get().unwrap().clone();

                                                    println!("[Game] Selected variant: {}", variant);
                                                    tokio::spawn(async move {
                                                        match variant.as_str() {
                                                            "5card" => {
                                                                println!("[Game] Running 5 Card Draw");
                                                                run_five_card_game(game_clients, players_for_game, pc, lc, gc, hc).await;
                                                            }
                                                            "7card" => {
                                                                println!("[Game] Running 7 Card Stud");
                                                                run_seven_card_game(game_clients, players_for_game, pc, lc, gc, hc).await;
                                                            }
                                                            "texas" => {
                                                                println!("[Game] Running Texas Hold'em");
                                                                run_texas_game(game_clients, players_for_game, pc, lc, gc, hc).await;
                                                            }
                                                            _ => {
                                                                eprintln!("[Game] Unknown game variant selected.");
                                                            }
                                                        }
                                                    });
                                                    
                                                }
                                            } else {
                                                send_to_client(&clients, &addr, "You are already in the game.");
                                            }
                                        } else {
                                            println!("[Game] Game full, rejecting player: {}", username);
                                            send_to_client(&clients, &addr, "Game is full. You are logged in but not in the game.");
                                        }

                                        println!("[Debug] Max players allowed: {}", max_players);
                                        println!("[Debug] Current number of players: {}", game_players.len());

                                        // Debug: Print each player and their address
                                        for (username, addr) in game_players.iter() {
                                            println!("[Debug] Player: {}, Address: {}", username, addr);
                                        }
                                    }
                                }
                            }
                            "stats" => { 
                                println!("Showing Stats"); 
                                let response = handle_stats(&players_collection).await;
                                send_to_client(&clients, &addr, &response);

                            }
                            "get_user_stats" => {
                                println!("Getting User Stats");
                                if let Some(username) = json.get("username").and_then(|v| v.as_str()) {
                                    // Note: `players_collection` is an Arc. We pass a reference to the inner collection.
                                    let response = get_user_stats(&*players_collection, username).await;
                                    send_to_client(&clients, &addr, &response);
                                } else {
                                    send_to_client(&clients, &addr, "Username not provided.");
                                }
                            }
                            "bet" => {
                                let username = json.get("username").and_then(|v| v.as_str()).unwrap_or("");
                                let amount = json.get("amount").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

                                let filter = doc! { "name": username };
                                let update = doc! {
                                    "$set": {
                                        "bet": amount,
                                        "bet_turn": false
                                    }
                                };
                                let _ = players_collection.update_one(filter, update).await;
                            }
                            "swap" => {
                                let username = json.get("username").and_then(|v| v.as_str()).unwrap_or("");
                                let swap_str = json.get("indices").and_then(|v| v.as_str()).unwrap_or("");

                                let filter = doc! { "name": username };
                                let update = doc! {
                                    "$set": {
                                        "swap": swap_str,
                                        "swap_turn": false
                                    }
                                };
                                let _ = players_collection.update_one(filter, update).await;
                            }
                            "spectate" => {
                                println!("{} requested spectate", addr);
                                let response = handle_spectate_command(&games_collection).await;
                                send_to_client(&clients, &addr, &response);
                            }
                            _ => {
                                send_to_client(&clients, &addr, "Unknown command.");
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }

        thread::sleep(Duration::from_millis(100));
    }
}

/// Attempts to determine the local IP address of the machine by creating a UDP socket
/// and connecting to a well-known external IP address (Google DNS: `8.8.8.8:80`).
///
/// This function does not actually send any data but uses the operating system's
/// routing table to determine the IP address that would be used for outbound traffic.
///
/// # Returns
///
/// * `Some(String)` - The local IP address as a string, if successfully determined.
/// * `None` - If the local IP could not be determined.
///
/// # Examples
///
/// ```
/// let local_ip = get_local_ip();
/// if let Some(ip) = local_ip {
///     println!("Local IP address: {}", ip);
/// } else {
///     println!("Could not determine local IP address.");
/// }
/// ```
fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;

    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                return Some(local_addr.ip().to_string());
            }
        }
    }
    None
}


/// Prompts the user at startup to configure game parameters:
/// - Number of players
/// - Game variant (5 Card Draw, 7 Card Stud, Texas Hold'em)
fn setup_game_config() {
    use std::io::{stdin, stdout, Write};

    // Number of players
    print!("Enter number of players [default = 2]: ");
    stdout().flush().unwrap();
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    let num_players = input.trim().parse::<usize>().unwrap_or(2);
    NUM_PLAYERS.set(num_players).unwrap();
    println!("Game will run with {} player(s).", num_players);

    // Game variant
    println!("Select game variant:");
    println!("1 - 5 Card Draw (default)");
    println!("2 - 7 Card Stud");
    println!("3 - Texas Hold'em");
    print!("Enter choice [1-3]: ");
    stdout().flush().unwrap();
    input.clear();
    stdin().read_line(&mut input).unwrap();
    let variant = match input.trim() {
        "2" => "7card",
        "3" => "texas",
        "1" => "5card",
        _ => "texas",
    };
    GAME_VARIANT.set(variant.to_string()).unwrap();
    println!("Selected variant: {}", variant);
}



// cargo test -- --test-threads=1
// cargo test -- --test-threads=1
// cargo test -- --test-threads=1
// cargo test -- --test-threads=1


#[cfg(test)]
mod test {
    use super::*;  
    use mongodb::{
        bson::{doc, Document},
        options::{ClientOptions, ServerApi, ServerApiVersion},
        Client, Collection,
    };
    use rand::{distributions::Alphanumeric, Rng};
    use tokio::runtime::Runtime;

    /// test collection
    async fn get_test_players_collection() -> Collection<Document> {
        let uri = "mongodb://localhost:27017";
        let mut client_options = ClientOptions::parse(uri).await.unwrap();
        let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
        client_options.server_api = Some(server_api);

        let client = Client::with_options(client_options).unwrap();
        let db = client.database("test_server");

        // clear players collection before each test
        let coll = db.collection::<Document>("players");
        coll.delete_many(doc! {}).await.unwrap(); 
        coll
    }

    fn random_username() -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect();
        format!("user_{}", s)
    }

    // 1) handle_registration with a new user
    #[tokio::test]
    async fn test_handle_registration_new_user() {
        let players_collection = get_test_players_collection().await;
        let username = random_username();
        let json_str = format!(r#"{{
            "command": "register",
            "username": "{}",
            "password": "mypassword"
        }}"#, username);

        let result = handle_registration(&players_collection, &json_str).await;
        assert!(
            result.contains("registered successfully"),
            "Registration should succeed for a new user."
        );

        let filter = doc! { "name": &username };
        let user_doc = players_collection.find_one(filter).await.unwrap();
        assert!(
            user_doc.is_some(),
            "User should be in DB after registration"
        );
    }

    // 2) handle_registration with an existing user
    #[tokio::test]
    async fn test_handle_registration_existing_user() {
        let players_collection = get_test_players_collection().await;
        let username = random_username();

        let json_str_1 = format!(r#"{{
            "command": "register",
            "username": "{}",
            "password": "password123"
        }}"#, username);
        let _ = handle_registration(&players_collection, &json_str_1).await;

        let json_str_2 = format!(r#"{{
            "command": "register",
            "username": "{}",
            "password": "newpassword"
        }}"#, username);
        let result2 = handle_registration(&players_collection, &json_str_2).await;
        assert!(
            result2.contains("already exists"),
            "Should detect duplicate registration"
        );
    }

    // 3) andle_login with valid credentials
    #[tokio::test]
    async fn test_handle_login_success() {
        let players_collection = get_test_players_collection().await;
        let username = random_username();
        
        let reg_json = format!(r#"{{
            "command": "register",
            "username": "{}",
            "password": "secret"
        }}"#, username);
        handle_registration(&players_collection, &reg_json).await;

        let login_json = format!(r#"{{
            "command": "login",
            "username": "{}",
            "password": "secret"
        }}"#, username);

        let result = handle_login(&players_collection, &login_json).await;
        assert!(result.contains("Welcome"), "Should log in successfully");
    }

    
    // 4) handle_login with wrong password
    #[tokio::test]
    async fn test_handle_login_wrong_password() {
        let players_collection = get_test_players_collection().await;
        let username = random_username();

        let reg_json = format!(r#"{{
            "command": "register",
            "username": "{}",
            "password": "realpassword"
        }}"#, username);
        handle_registration(&players_collection, &reg_json).await;

        let login_json = format!(r#"{{
            "command": "login",
            "username": "{}",
            "password": "wrongpassword"
        }}"#, username);

        let result = handle_login(&players_collection, &login_json).await;
        assert!(result.contains("Invalid password"), "Should reject wrong password");
    }


    // 5) handle_login with nonexistent user
    #[tokio::test]
    async fn test_handle_login_user_not_found() {
        let players_collection = get_test_players_collection().await;

        let login_json = r#"{
            "command": "login",
            "username": "nonexistent_user_xyz",
            "password": "whatever"
        }"#;

        let result = handle_login(&players_collection, &login_json).await;
        assert!(
            result.contains("No such user found"),
            "Should fail for non-existing user"
        );
    }

    // 6) handle_stats when no users exist
    #[tokio::test]
    async fn test_handle_stats_no_users() {
        let players_collection = get_test_players_collection().await;

        let result = handle_stats(&players_collection).await;
        assert!(
            result.is_empty(),
            "Should return empty stats when no users in DB"
        );
    }

    // 7) handle_stats with multiple users
    #[tokio::test]
    async fn test_handle_stats_with_users() {
        let players_collection = get_test_players_collection().await;
        
        let user1 = random_username();
        let user2 = random_username();

        players_collection.insert_one(doc! { "name": &user1, "password": "pass" }).await.unwrap();
        players_collection.insert_one(doc! { "name": &user2, "password": "pass" }).await.unwrap();

        let stats_str = handle_stats(&players_collection).await;
        // function returns a comma-separated string with user1, user2
        assert!(stats_str.contains(&user1), "Stats should contain first user");
        assert!(stats_str.contains(&user2), "Stats should contain second user");
    }

    // 8) get_user_stats for an existing user
    #[tokio::test]
    async fn test_get_user_stats_found() {
        let players_collection = get_test_players_collection().await;
        let username = random_username();

        players_collection.insert_one(doc! {
            "name": &username,
            "password": "mypassword",
            "wins": 5,
            "losses": 2
        }).await.unwrap();

        let stats_json = get_user_stats(&players_collection, &username).await;
        assert!(stats_json.contains(&username), "Should show the correct user in JSON");
        assert!(stats_json.contains("\"wins\":5"), "Should contain correct wins");
        assert!(stats_json.contains("\"losses\":2"), "Should contain correct losses");
    }

    // 9) get_user_stats for a nonexistent user
    #[tokio::test]
    async fn test_get_user_stats_not_found() {
        let players_collection = get_test_players_collection().await;

        let result = get_user_stats(&players_collection, "does_not_exist").await;
        assert!(
            result.contains("No player found"),
            "Should return not-found message"
        );
    }

    // 10) bet update
    #[tokio::test]
    async fn test_bet_command_sim() {
        let players_collection = get_test_players_collection().await;
        let username = random_username();

        // Insert the user doc (simulate registration)
        players_collection.insert_one(doc! {
            "name": &username,
            "password": "securepwd",
            "bet": -1,
            "bet_turn": true
        }).await.unwrap();

   
        let bet_amount = 50;
        let filter = doc! { "name": &username };
        let update = doc! {
            "$set": { "bet": bet_amount, "bet_turn": false }
        };
        let res = players_collection.update_one(filter.clone(), update).await.unwrap();
        assert_eq!(res.matched_count, 1, "Update should affect 1 document");

        let user_doc = players_collection.find_one(filter).await.unwrap().unwrap();
        let bet_in_db = user_doc.get_i32("bet").unwrap();
        let bet_turn_in_db = user_doc.get_bool("bet_turn").unwrap();

        assert_eq!(bet_in_db, bet_amount, "Bet amount should be updated in DB");
        assert_eq!(bet_turn_in_db, false, "bet_turn should now be false");
    }
}
