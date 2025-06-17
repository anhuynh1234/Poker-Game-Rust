//! # Database Operations
//!
//! This module handles all database operations for the server.
//!
//! Uses MongoDB to store and update player data, game states, and history.
//! Includes functions to initialize the database, update game results, handle player actions, and retrieve stats.
use mongodb::{
    bson::Document,
    bson::doc,
    options::{ClientOptions, ServerApi, ServerApiVersion},
    Client, Collection, bson
};
use futures_util::stream::StreamExt;
use crate::five_card_draw::Player;
use serde_json::json;
use serde_json::Value;


/// Connects to the MongoDB database and returns the main collections used by the server.
///
/// Collections returned:
/// - `players`: Stores player data and stats.
/// - `lobbies`: Stores game lobby information.
/// - `games`: Stores active game states.
/// - `history`: Stores game history and past results.
///
/// # Returns
/// A tuple with the four MongoDB collections.
///
/// # Errors
/// Returns an error if the database connection or collection setup fails.
pub async fn init_db() -> mongodb::error::Result<(
    Collection<Document>,
    Collection<Document>,
    Collection<Document>,
    Collection<Document>,
)> {
    let uri = "mongodb://localhost:27017";
    let mut client_options = ClientOptions::parse(uri).await?;
    let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
    client_options.server_api = Some(server_api);

    let client = Client::with_options(client_options)?;
    let players_collection = client.database("dealer").collection("players");
    let lobbies_collection = client.database("dealer").collection("lobbies");
    let games_collection = client.database("dealer").collection("games");
    let history_collection = client.database("dealer").collection("history");

    Ok((players_collection, lobbies_collection, games_collection, history_collection))
}

/// Retrieves the current bet amount for a specific player.
///
/// # Arguments
/// * `players_collection` - Reference to the MongoDB players collection.
/// * `username` - The name of the player to query.
///
/// # Returns
/// An `Option<i32>` containing the bet amount if found.
pub async fn get_player_bet(
    players_collection: &Collection<Document>,
    username: &str,
) -> Option<i32> {
    let filter = doc! { "name": username };

    match players_collection.find_one(filter).await {
        Ok(Some(player_doc)) => {
            // Try to extract the "bet" field
            match player_doc.get_i32("bet") {
                Ok(bet) => Some(bet),
                Err(e) => {
                    println!("Failed to get 'bet' field: {}", e);
                    None
                }
            }
        }
        Ok(None) => {
            println!("No player found with name: {}", username);
            None
        }
        Err(e) => {
            println!("Database error: {}", e);
            None
        }
    }
}

/// Updates stats for players who folded in the current game.
///
/// Increases games played, losses, and money lost for each folded player.
///
/// # Arguments
/// * `players_collection` - Reference to the MongoDB players collection.
/// * `folded_players` - List of players who folded.
///
/// # Returns
/// MongoDB operation result.
pub async fn update_players_folded(
    players_collection: &Collection<Document>,
    folded_players: &[Player],
) -> mongodb::error::Result<()> {
    for player in folded_players {
        let filter = doc! { "name": &player.id };

        let update = doc! {
            "$inc": {
                "games_played": 1,
                "losses": 1,
                "money_lost": player.money_lost*-1,
            },
        };

        if let Err(e) = players_collection.update_one(filter, update).await {
            eprintln!("[DB] Failed to update folded player {}: {}", player.id, e);
        } else {
            println!("[DB] Updated folded player: {}", player.id);
        }
    }

    Ok(())
}

/// Gets the swap choice for a specific player.
///
/// # Arguments
/// * `players_collection` - Reference to MongoDB players collection.
/// * `player_id` - Player's unique identifier.
///
/// # Returns
/// An `Option<String>` containing the swap string if found.
pub async fn get_player_swap(
    players_collection: &Collection<Document>,
    player_id: &str,
) -> Option<String> {
    let filter = doc! { "name": player_id };
    if let Ok(Some(doc)) = players_collection.find_one(filter).await {
        doc.get_str("swap").ok().map(|s| s.to_string())
    } else {
        None
    }
}

/// Updates player statistics after a game ends.
///
/// - Increments games played for all players.
/// - Updates wins, losses, money won or lost based on game results.
///
/// # Arguments
/// * `players_collection` - Reference to the MongoDB players collection.
/// * `winner_id` - The ID of the winning player.
/// * `folded_players` - List of players who folded.
/// * `pot_amount` - Total pot amount to assign to the winner.
///
/// # Returns
/// MongoDB operation result.
pub async fn update_game_results(
    players_collection: &Collection<Document>,
    winner_id: &str,
    folded_players: &[Player],
    pot_amount: i32,
) -> mongodb::error::Result<()> {
    for player in folded_players {
        let filter = doc! { "name": &player.id };

        if winner_id == player.id {
            let winner_update = doc! {
                "$inc": {
                    "games_played": 1,
                    "wins": 1,
                    "money_win": pot_amount,
                    "money_lost": player.money_lost*-1,
                },
            };

            if let Err(e) = players_collection.update_one(filter, winner_update).await {
                eprintln!("[DB] Failed to update folded player {}: {}", player.id, e);
            } else {
                println!("[DB] Updated folded player: {}", player.id);
            }
        }else {
            let update = doc! {
                "$inc": {
                    "games_played": 1,
                    "losses": 1,
                    "money_lost": player.money_lost*-1,
                },
            };
    
            if let Err(e) = players_collection.update_one(filter, update).await {
                eprintln!("[DB] Failed to update folded player {}: {}", player.id, e);
            } else {
                println!("[DB] Updated folded player: {}", player.id);
            }
        }

    }

    Ok(())
}

/// Retrieves a list of all player names from database.
///
/// # Arguments
/// * `players_collection` - Reference to the MongoDB players collection.
///
/// # Returns
/// A comma-separated `String` of player names.
pub async fn handle_stats(
    players_collection: &Collection<Document>,
) -> String {
    // Use an empty filter to retrieve all documents.
    let filter = doc! {};
    let mut cursor = match players_collection.find(filter).await {
        Ok(cursor) => cursor,
        Err(e) => {
            println!("Database error: {}", e);
            return "".to_string();
        }
    };

    let mut names: Vec<String> = Vec::new();

    while let Some(result) = cursor.next().await {
        match result {
            Ok(doc) => {
                // Attempt to extract the "name" field.
                if let Ok(name) = doc.get_str("name") {
                    names.push(name.to_string());
                } else {
                    println!("Failed to get 'name' field from document: {:?}", doc);
                }
            }
            Err(e) => {
                println!("Error retrieving document: {}", e);
            }
        }
    }

    // Return all names as a comma-separated string.
    names.join(", ")
}

/// Retrieves detailed statistics for a specific player.
///
/// Converts BSON doc to a JSON string.
///
/// # Arguments
/// * `players_collection` - Reference to the MongoDB players collection.
/// * `username` - The name of the player to query.
///
/// # Returns
/// A JSON `String` of the player's stats, or an error message if not found.
pub async fn get_user_stats(
    players_collection: &Collection<Document>,
    username: &str,
) -> String {
    let filter = doc! { "name": username };
    match players_collection.find_one(filter).await {
        Ok(Some(player_doc)) => {
            // Convert the BSON document to a JSON string.
            match serde_json::to_string(&player_doc) {
                Ok(json_str) => json_str,
                Err(e) => format!("Error serializing player data: {}", e),
            }
        }
        Ok(None) => format!("No player found with name: {}", username),
        Err(e) => format!("Database error: {}", e),
    }
}


/// Clears the games collection and initializes a new game state document with _id = 1.
pub async fn init_game_state(
    games_collection: &Collection<Document>,
) -> mongodb::error::Result<()> {
    // Step 1: Clear the collection
    match games_collection.delete_many(doc! {}).await {
        Ok(result) => println!("[DB] Cleared games collection ({} docs deleted)", result.deleted_count),
        Err(e) => {
            eprintln!("[DB] Failed to clear games collection: {}", e);
            return Err(e);
        }
    }

    let initial_state = json!({
        "pot": 0,
        "round": 0,
        "round current bet": 0,
        "player current bets": null,
        "cards": null,
        "community": null,
        "info": "Game initialized"
    });

    // Step 2: Convert JSON state to BSON document and insert it
    let mut doc = bson::to_document(&initial_state)
        .map_err(|e| {
            eprintln!("[DB] Failed to serialize initial game state: {}", e);
            mongodb::error::Error::from(e)
        })?;

    doc.insert("_id", 1);

    games_collection.insert_one(doc).await.map(|_| {
        println!("[DB] Initialized new game state with _id = 1");
    })
}


/// Retrieves the current game state from the MongoDB collection for spectators.
///
/// Converts the full game state BSON document with `_id = 1` to a JSON string
/// so it can be displayed on the client side.
///
/// # Arguments
/// * `games_collection` - Reference to the MongoDB `games` collection.
///
/// # Returns
/// A `String` containing the serialized JSON game state or an error message
/// if the document is missing or fails to serialize.
pub async fn handle_spectate_command(games_collection: &Collection<Document>) -> String {
    let filter = doc! { "_id": 1 };

    match games_collection.find_one(filter).await {
        Ok(Some(doc)) => {
            // Serialize the entire document into a JSON string
            match serde_json::to_string(&doc) {
                Ok(json_str) => json_str,
                Err(e) => json!({ "error": format!("Serialization failed: {}", e) }).to_string(),
            }
        }
        Ok(None) => json!({ "error": "Game state not initialized." }).to_string(),
        Err(e) => json!({ "error": format!("Database error: {}", e) }).to_string(),
    }
}


/// Updates a specific field in the game state document with `_id = 1`.
///
/// This is used by the server to update various parts of the game state
/// (e.g., pot, current bets, community cards) in the MongoDB `games` collection.
///
/// # Arguments
/// * `games_collection` - Reference to the MongoDB `games` collection.
/// * `key` - The field name to update (e.g., `"pot"`, `"cards"`).
/// * `value` - The new JSON value to set for the specified field.
///
/// # Returns
/// A `mongodb::error::Result<()>` indicating success or failure of the update.
pub async fn update_game_state_field(
    games_collection: &Collection<Document>,
    key: &str,
    value: Value,
) -> mongodb::error::Result<()> {
    let update = doc! {
        "$set": {
            key: bson::to_bson(&value).unwrap()
        }
    };

    games_collection
        .update_one(doc! { "_id": 1 }, update)
        .await?;

    println!("[DB] Updated game field: {} = {}", key, value);
    Ok(())
}
