//! # User Info Handling
//!
//! This module manages user registration and login functionalities.
//! It connects to the MongoDB `players_collection` and processes incoming JSON data
//! to register new players or authenticate existing ones.
//!
//! Functions:
//! - `handle_registration`: Register a new player.
//! - `handle_login`: Authenticate an existing player.
use serde_json::Value;
use mongodb::bson::doc;
use mongodb::{
    bson::Document,
    options::{ClientOptions, ServerApi, ServerApiVersion},
    Client, Collection,
};

/// Handles new user registration.
///
/// Expects a JSON string input containing:
/// - `"username"`:  username.
/// - `"password"`:  password.
///
/// # Behavior:
/// - If the JSON is invalid, returns an error message.
/// - If either the username or password is missing, returns an error.
/// - If the username already exists in the database, returns an error.
/// - If all checks pass, creates a new player document in the `players_collection`.
///
/// # Arguments
/// * `players_collection` - MongoDB collection for storing player documents.
/// * `data` - JSON string containing registration info.
///
/// # Returns
/// A string message indicating success or why it failed.
///
/// # Example Input JSON
/// ```json
/// { "username": "player1", "password": "secret" }
/// ```
pub async fn handle_registration(players_collection: &Collection<Document>, data: &str) -> String {
    let parsed: Result<Value, _> = serde_json::from_str(data);
    if parsed.is_err() {
        return "Invalid JSON format.".to_string();
    }

    let json = parsed.unwrap();
    let username = json.get("username").and_then(Value::as_str).unwrap_or("");
    let password = json.get("password").and_then(Value::as_str).unwrap_or("");

    if username.is_empty() || password.is_empty() {
        return "Username and password required.".to_string();
    }

    let existing = players_collection
        .find_one(doc! { "name": username })
        .await
        .unwrap();

    if existing.is_some() {
        return format!("Username '{}' already exists.", username);
    }

    let player_doc = doc! {
        "name": username,
        "password": password,
        "wins": 0,
        "losses": 0,
        "games_played": 0,
        "money_win": 0,
        "money_lost": 0,
        "prompts": doc! {},
        "hand": "",
        "bet": -1,
        "bet_turn": false,
        "money": -1,
        "swap": -1,
        "swap_turn": false
    };

    let _ = players_collection.insert_one(player_doc).await;
    format!("Player '{}' registered successfully.", username)
}

/// Handles user login/authentication.
///
/// Expects a JSON string input containing:
/// - `"username"`: Username to authenticate.
/// - `"password"`: Password to authenticate.
///
/// # Behavior:
/// - If the JSON is invalid, returns an error message.
/// - If either the username or password is missing, returns error.
/// - If the username exists and password matches, returns welcome message.
/// - If the password is incorrect or user does not exist, returns error.
///
/// # Arguments
/// * `players_collection` - MongoDB collection containing player documents.
/// * `data` - JSON string containing login info.
///
/// # Returns
/// A string message indicating success or the reason for failure.
///
/// # Example Input JSON
/// ```json
/// { "username": "player1", "password": "secret" }
/// ```
pub async fn handle_login(players_collection: &Collection<Document>, data: &str) -> String {
    let parsed: Result<Value, _> = serde_json::from_str(data);
    if parsed.is_err() {
        return "Invalid JSON format.".to_string();
    }

    let json = parsed.unwrap();
    let username = json.get("username").and_then(Value::as_str).unwrap_or("");
    let password = json.get("password").and_then(Value::as_str).unwrap_or("");

    if username.is_empty() || password.is_empty() {
        return "Username and password required.".to_string();
    }

    let filter = doc! { "name": username };
    if let Some(player_doc) = players_collection.find_one(filter).await.unwrap() {
        if let Some(stored_pass) = player_doc.get_str("password").ok() {
            if stored_pass == password {
                return format!("Welcome, {}! You are now in the waiting room.", username);
            }
        }
        return "Invalid password.".to_string();
    }

    "No such user found.".to_string()
}