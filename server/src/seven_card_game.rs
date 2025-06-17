//! # Seven Card Stud Game Flow
//!
//! This module controls the full game logic for Seven Card Stud poker.
//! It handles the entire game lifecycle, including:
//! - Player setup and ante collection
//! - Dealing cards across multiple "streets"
//! - Multiple betting rounds
//! - Determining the winner
//! - Broadcasting game state to clients
//! - Updating results in the database
use std::{
    collections::HashMap,
    sync::mpsc::{self, Sender},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use mongodb::{
    bson::Document,
    Collection,
    bson::doc
};
use serde_json::json;
use crate::comms::*;
use crate::*;
use crate::seven_card_stud::*;

/// Runs a game of Seven Card Stud poker.
///
/// This function orchestrates all stages of the game:
/// - Starts the game and notifies players
/// - Collects the ante from each player
/// - Deals cards through all "streets" (rounds)
/// - Manages multiple betting rounds
/// - Determines and announces the winner
/// - Updates game results in the database
///
/// # Arguments
/// * `clients` - Shared connection list of clients.
/// * `player_names` - List of player names participating.
/// * `players_collection` - MongoDB collection for players.
/// * `lobbies_collection` - MongoDB collection for lobbies.
/// * `games_collection` - MongoDB collection for ongoing games.
/// * `history_collection` - MongoDB collection for game history.
///
pub async fn run_seven_card_game(
    clients: Arc<Mutex<HashMap<std::net::SocketAddr, ClientInfo>>>,
    player_names: Vec<String>,
    players_collection: Arc<Collection<Document>>,
    lobbies_collection: Arc<Collection<Document>>,
    games_collection: Arc<Collection<Document>>,
    history_collection: Arc<Collection<Document>>,

) {
    let variant = GAME_VARIANT.get().unwrap();
    broadcast_to_game_players(&clients, "Started the game: {}");

    let mut poker_game:SevenCardStudGame = SevenCardStudGame::new(player_names.clone());
    // poker_game.current_players = poker_game.players.clone();

    println!("[Game] Created PokerGame for variant: {}", variant);
    println!("[Game] Players: {:?}", poker_game.current_players.iter().map(|p| &p.id).collect::<Vec<_>>());


    
    // Collecting ante
    let ante: i32 = 5;
    broadcast_to_game_players(&clients, "Collecting ante of 5");
    
    for player in &mut poker_game.current_players {
        player.money_lost += ante as i32;
        poker_game.pot += ante;
    }

    update_game_state_field(&games_collection, "info", "Collecting ante for all players".into()).await.unwrap();
    update_game_state_field(&games_collection, "pot", poker_game.pot.clone().into()).await.unwrap();



    // Deal cards
    poker_game.deal_third_street();
    let mut hands_map: HashMap<String, Vec<String>> = HashMap::new();
    for player in &poker_game.current_players {
        let cards = player
            .hand
            .iter()
            .map(|card| format!("{}", card))
            .collect::<Vec<_>>();
            // .join(", ");

        hands_map.insert(player.id.clone(), cards.clone());
        println!(" - {}: {}", player.id, cards.join(", "));
    }

    let message = json!({
        "7 card hands": hands_map
    });
    
    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();



    // First round of betting
    let bring_in_player_id = poker_game.determine_bring_in().unwrap_or_else(|| poker_game.current_players[0].id.clone());
    // Determining player bring in index
    let bring_in_index = poker_game
        .current_players
        .iter()
        .position(|p| p.id == bring_in_player_id)
        .unwrap_or(0);

    let mut player_bet_index: i32 = bring_in_index as i32;
    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();

    loop {
        if player_bet_index >= poker_game.current_players.len() as i32{
            player_bet_index = 0;
        }

        let player_id = poker_game.current_players[player_bet_index as usize].id.clone();

        if player_id == first_highest_bet || poker_game.current_players.len() == 1 {
            break;
        }

        loop {
            let mut bet_amounts_map: HashMap<String, i32> = HashMap::new();
            for p in &poker_game.current_players {
                bet_amounts_map.insert(p.id.clone(), p.bet_amount);
            }

            let message = json!({
                "7 card hands": hands_map,
                "bet": player_id.clone(),
                "pot": poker_game.pot,
                "round current bet": poker_game.current_bet,
                "player bet amount": bet_amounts_map
            });
            
            let json_str = message.to_string();
            println!("JSON to send: {}", json_str);
            broadcast_to_game_players(&clients, json_str.as_str());

            update_game_state_field(&games_collection, "pot", poker_game.pot.into()).await.unwrap();
            update_game_state_field(&games_collection, "round current bet", poker_game.current_bet.into()).await.unwrap();
            let bet_amounts_value = serde_json::to_value(&bet_amounts_map).unwrap();
            update_game_state_field(&games_collection, "player current bets", bet_amounts_value).await.unwrap();
            let hands_value = serde_json::to_value(&hands_map).unwrap();
            update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();
            
            let filter = doc! { "name": &player_id.clone() };
            let update = doc! { "$set": { "bet_turn": true } };
            let _ = players_collection.update_one(filter.clone(), update).await;
            
            // Wait till the bet is made
            loop {
                // Sleep to avoid hammering the DB
                thread::sleep(Duration::from_millis(300));
                
                // Fetch the latest value
                if let Ok(Some(doc)) = players_collection.find_one(filter.clone()).await {
                    if let Some(bet_turn) = doc.get_bool("bet_turn").ok() {
                        if !bet_turn {
                            println!("{} has completed their bet.", player_id);
                            break;
                        }
                    }
                }
            }
            
            if let Some(bet_amount) = get_player_bet(&players_collection, &player_id).await {
                if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                    if bet_amount == -1 {
                        println!("{} folds.", player_id.clone());
                        if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                            poker_game.players.push(player.clone());
                        }
                        poker_game.current_players.remove(player_bet_index as usize);
                        player_bet_index -= 1;
                        send_to_player_by_id(&clients, player_id.as_str(), "You folded");
                        break;
                    }
                    
                    if player.bet_amount + bet_amount >= poker_game.current_bet {
                        if player.bet_amount + bet_amount > poker_game.current_bet {
                            first_highest_bet = player.id.clone();
                            if poker_game.current_bet < 0 {
                                poker_game.current_bet = 0;
                            }
                            poker_game.current_bet = player.bet_amount + bet_amount;
                        }
                        player.money_lost += bet_amount;
                        player.bet_amount += bet_amount;
                        poker_game.pot += bet_amount;
                        break;
                    }
                }
                
            }        
        }
        player_bet_index += 1;
    }



    // After 1st round of betting
    update_players_folded(&players_collection, &poker_game.players.clone()).await.unwrap();

    if poker_game.current_players.len() == 1 {
        let winner_id = poker_game.current_players[0].id.clone();
        broadcast_to_game_players(&clients, format!("Game is over, winner is {}", winner_id).as_str());
        update_game_state_field(&games_collection, "winner", format!("Game is over, winner is {}", winner_id).as_str().into()).await.unwrap();

        println!("[Game] Winner determined: {}", winner_id);
        
        // Update database with results
        if let Err(e) = db::update_game_results(
            &players_collection,
            &winner_id,
            &poker_game.current_players.clone(),
            poker_game.pot,
        ).await {
            eprintln!("Failed to update game results: {}", e);
        }
        
        return;
    }



    // --- Deal 4th Street ---
    poker_game.deal_fourth_street();
    let mut hands_map: HashMap<String, Vec<String>> = HashMap::new();
    for player in &poker_game.current_players {
        let cards = player
            .hand
            .iter()
            .map(|card| format!("{}", card))
            .collect::<Vec<_>>();
            // .join(", ");

        hands_map.insert(player.id.clone(), cards.clone());
        println!(" - {}: {}", player.id, cards.join(", "));
    }

    let message = json!({
        "7 card hands": hands_map
    });
    
    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();



    // Second round of betting
    // Resetting bet for all players to 0
    for player in &mut poker_game.current_players {
        player.bet_amount = 0;
    }

    let best_hand_id = poker_game.determine_best_faceup_hand_id().unwrap_or_else(|| poker_game.current_players[0].id.clone());
    // Determining player bring in index
    let best_hand_index = poker_game
        .current_players
        .iter()
        .position(|p| p.id == best_hand_id)
        .unwrap_or(0);

    let mut player_bet_index: i32 = best_hand_index as i32;
    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();

    loop {
        if player_bet_index >= poker_game.current_players.len() as i32{
            player_bet_index = 0;
        }

        let player_id = poker_game.current_players[player_bet_index as usize].id.clone();

        if player_id == first_highest_bet || poker_game.current_players.len() == 1 {
            break;
        }

        loop {
            let mut bet_amounts_map: HashMap<String, i32> = HashMap::new();
            for p in &poker_game.current_players {
                bet_amounts_map.insert(p.id.clone(), p.bet_amount);
            }


            let message = json!({
                "7 card hands": hands_map,
                "bet": player_id.clone(),
                "pot": poker_game.pot,
                "round current bet": poker_game.current_bet,
                "player bet amount": bet_amounts_map,
            });
            
            let json_str = message.to_string();
            println!("JSON to send: {}", json_str);
            broadcast_to_game_players(&clients, json_str.as_str());

            update_game_state_field(&games_collection, "pot", poker_game.pot.into()).await.unwrap();
            update_game_state_field(&games_collection, "round current bet", poker_game.current_bet.into()).await.unwrap();
            let bet_amounts_value = serde_json::to_value(&bet_amounts_map).unwrap();
            update_game_state_field(&games_collection, "player current bets", bet_amounts_value).await.unwrap();
            let hands_value = serde_json::to_value(&hands_map).unwrap();
            update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();
            
            let filter = doc! { "name": &player_id.clone() };
            let update = doc! { "$set": { "bet_turn": true } };
            let _ = players_collection.update_one(filter.clone(), update).await;
            
            // Wait till the bet is made
            loop {
                // Sleep to avoid hammering the DB
                thread::sleep(Duration::from_millis(300));
                
                // Fetch the latest value
                if let Ok(Some(doc)) = players_collection.find_one(filter.clone()).await {
                    if let Some(bet_turn) = doc.get_bool("bet_turn").ok() {
                        if !bet_turn {
                            println!("{} has completed their bet.", player_id);
                            break;
                        }
                    }
                }
            }
            
            if let Some(bet_amount) = get_player_bet(&players_collection, &player_id).await {
                if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                    if bet_amount == -1 {
                        println!("{} folds.", player_id.clone());
                        if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                            poker_game.players.push(player.clone());
                        }
                        poker_game.current_players.remove(player_bet_index as usize);
                        player_bet_index -= 1;
                        send_to_player_by_id(&clients, player_id.as_str(), "You folded");
                        break;
                    }

                    if player.bet_amount + bet_amount >= poker_game.current_bet {
                        if player.bet_amount + bet_amount > poker_game.current_bet {
                            first_highest_bet = player.id.clone();
                            if poker_game.current_bet < 0 {
                                poker_game.current_bet = 0;
                            }
                            poker_game.current_bet = player.bet_amount + bet_amount;
                        }
                        player.money_lost += bet_amount;
                        player.bet_amount += bet_amount;
                        poker_game.pot += bet_amount;
                        break;
                    }
                }
                
            }        
        }
        player_bet_index += 1;
    }



    // After 2nd round of betting
    update_players_folded(&players_collection, &poker_game.players.clone()).await.unwrap();

    if poker_game.current_players.len() == 1 {
        let winner_id = poker_game.current_players[0].id.clone();
        broadcast_to_game_players(&clients, format!("Game is over, winner is {}", winner_id).as_str());
        update_game_state_field(&games_collection, "winner", format!("Game is over, winner is {}", winner_id).as_str().into()).await.unwrap();

        println!("[Game] Winner determined: {}", winner_id);
        
        // Update database with results
        if let Err(e) = db::update_game_results(
            &players_collection,
            &winner_id,
            &poker_game.current_players.clone(),
            poker_game.pot,
        ).await {
            eprintln!("Failed to update game results: {}", e);
        }
        
        return;
    }



    // --- Deal 5th Street ---
    poker_game.deal_fifth_street();
    let mut hands_map: HashMap<String, Vec<String>> = HashMap::new();
    for player in &poker_game.current_players {
        let cards = player
            .hand
            .iter()
            .map(|card| format!("{}", card))
            .collect::<Vec<_>>();
            // .join(", ");

        hands_map.insert(player.id.clone(), cards.clone());
        println!(" - {}: {}", player.id, cards.join(", "));
    }

    let message = json!({
        "7 card hands": hands_map
    });
    
    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();



    // third round of betting
    // Resetting bet for all players to 0
    for player in &mut poker_game.current_players {
        player.bet_amount = 0;
    }

    let best_hand_id = poker_game.determine_best_faceup_hand_after_fifth_street().unwrap_or_else(|| poker_game.current_players[0].id.clone());
    // Determining player bring in index
    let best_hand_index = poker_game
        .current_players
        .iter()
        .position(|p| p.id == best_hand_id)
        .unwrap_or(0);

    let mut player_bet_index: i32 = best_hand_index as i32;
    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();

    loop {
        if player_bet_index >= poker_game.current_players.len() as i32{
            player_bet_index = 0;
        }

        let player_id = poker_game.current_players[player_bet_index as usize].id.clone();

        if player_id == first_highest_bet || poker_game.current_players.len() == 1 {
            break;
        }

        loop {
            let mut bet_amounts_map: HashMap<String, i32> = HashMap::new();
            for p in &poker_game.current_players {
                bet_amounts_map.insert(p.id.clone(), p.bet_amount);
            }


            let message = json!({
                "7 card hands": hands_map,
                "bet": player_id.clone(),
                "pot": poker_game.pot,
                "round current bet": poker_game.current_bet,
                "player bet amount": bet_amounts_map,
            });
            
            let json_str = message.to_string();
            println!("JSON to send: {}", json_str);
            broadcast_to_game_players(&clients, json_str.as_str());

            update_game_state_field(&games_collection, "pot", poker_game.pot.into()).await.unwrap();
            update_game_state_field(&games_collection, "round current bet", poker_game.current_bet.into()).await.unwrap();
            let bet_amounts_value = serde_json::to_value(&bet_amounts_map).unwrap();
            update_game_state_field(&games_collection, "player current bets", bet_amounts_value).await.unwrap();
            let hands_value = serde_json::to_value(&hands_map).unwrap();
            update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();
            
            let filter = doc! { "name": &player_id.clone() };
            let update = doc! { "$set": { "bet_turn": true } };
            let _ = players_collection.update_one(filter.clone(), update).await;
            
            // Wait till the bet is made
            loop {
                // Sleep to avoid hammering the DB
                thread::sleep(Duration::from_millis(300));
                
                // Fetch the latest value
                if let Ok(Some(doc)) = players_collection.find_one(filter.clone()).await {
                    if let Some(bet_turn) = doc.get_bool("bet_turn").ok() {
                        if !bet_turn {
                            println!("{} has completed their bet.", player_id);
                            break;
                        }
                    }
                }
            }
            
            if let Some(bet_amount) = get_player_bet(&players_collection, &player_id).await {
                if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                    if bet_amount == -1 {
                        println!("{} folds.", player_id.clone());
                        if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                            poker_game.players.push(player.clone());
                        }
                        poker_game.current_players.remove(player_bet_index as usize);
                        player_bet_index -= 1;
                        send_to_player_by_id(&clients, player_id.as_str(), "You folded");
                        break;
                    }

                    if player.bet_amount + bet_amount >= poker_game.current_bet {
                        if player.bet_amount + bet_amount > poker_game.current_bet {
                            first_highest_bet = player.id.clone();
                            if poker_game.current_bet < 0 {
                                poker_game.current_bet = 0;
                            }
                            poker_game.current_bet = player.bet_amount + bet_amount;
                        }
                        player.money_lost += bet_amount;
                        player.bet_amount += bet_amount;
                        poker_game.pot += bet_amount;
                        break;
                    }
                }
            }        
        }
        player_bet_index += 1;
    }



    // After 3rd round of betting
    update_players_folded(&players_collection, &poker_game.players.clone()).await.unwrap();

    if poker_game.current_players.len() == 1 {
        let winner_id = poker_game.current_players[0].id.clone();
        broadcast_to_game_players(&clients, format!("Game is over, winner is {}", winner_id).as_str());
        update_game_state_field(&games_collection, "winner", format!("Game is over, winner is {}", winner_id).as_str().into()).await.unwrap();

        println!("[Game] Winner determined: {}", winner_id);
        
        // Update database with results
        if let Err(e) = db::update_game_results(
            &players_collection,
            &winner_id,
            &poker_game.current_players.clone(),
            poker_game.pot,
        ).await {
            eprintln!("Failed to update game results: {}", e);
        }
        
        return;
    }


    
    // --- Deal 6th Street ---
    poker_game.deal_sixth_street();
    let mut hands_map: HashMap<String, Vec<String>> = HashMap::new();
    for player in &poker_game.current_players {
        let cards = player
            .hand
            .iter()
            .map(|card| format!("{}", card))
            .collect::<Vec<_>>();
            // .join(", ");

        hands_map.insert(player.id.clone(), cards.clone());
        println!(" - {}: {}", player.id, cards.join(", "));
    }

    let message = json!({
        "7 card hands": hands_map
    });
    
    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();



    // 4th round of betting
    // Resetting bet for all players to 0
    for player in &mut poker_game.current_players {
        player.bet_amount = 0;
    }

    let best_hand_id = poker_game.determine_best_faceup_hand_after_sixth_street().unwrap_or_else(|| poker_game.current_players[0].id.clone());
    // Determining player bring in index
    let best_hand_index = poker_game
        .current_players
        .iter()
        .position(|p| p.id == best_hand_id)
        .unwrap_or(0);

    let mut player_bet_index: i32 = best_hand_index as i32;
    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();

    loop {
        if player_bet_index >= poker_game.current_players.len() as i32{
            player_bet_index = 0;
        }

        let player_id = poker_game.current_players[player_bet_index as usize].id.clone();

        if player_id == first_highest_bet || poker_game.current_players.len() == 1 {
            break;
        }

        loop {
            let mut bet_amounts_map: HashMap<String, i32> = HashMap::new();
            for p in &poker_game.current_players {
                bet_amounts_map.insert(p.id.clone(), p.bet_amount);
            }


            let message = json!({
                "7 card hands": hands_map,
                "bet": player_id.clone(),
                "pot": poker_game.pot,
                "round current bet": poker_game.current_bet,
                "player bet amount": bet_amounts_map,
            });
            
            let json_str = message.to_string();
            println!("JSON to send: {}", json_str);
            broadcast_to_game_players(&clients, json_str.as_str());

            update_game_state_field(&games_collection, "pot", poker_game.pot.into()).await.unwrap();
            update_game_state_field(&games_collection, "round current bet", poker_game.current_bet.into()).await.unwrap();
            let bet_amounts_value = serde_json::to_value(&bet_amounts_map).unwrap();
            update_game_state_field(&games_collection, "player current bets", bet_amounts_value).await.unwrap();
            let hands_value = serde_json::to_value(&hands_map).unwrap();
            update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();
            
            let filter = doc! { "name": &player_id.clone() };
            let update = doc! { "$set": { "bet_turn": true } };
            let _ = players_collection.update_one(filter.clone(), update).await;
            
            // Wait till the bet is made
            loop {
                // Sleep to avoid hammering the DB
                thread::sleep(Duration::from_millis(300));
                
                // Fetch the latest value
                if let Ok(Some(doc)) = players_collection.find_one(filter.clone()).await {
                    if let Some(bet_turn) = doc.get_bool("bet_turn").ok() {
                        if !bet_turn {
                            println!("{} has completed their bet.", player_id);
                            break;
                        }
                    }
                }
            }
            
            if let Some(bet_amount) = get_player_bet(&players_collection, &player_id).await {
                if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                    if bet_amount == -1 {
                        println!("{} folds.", player_id.clone());
                        if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                            poker_game.players.push(player.clone());
                        }
                        poker_game.current_players.remove(player_bet_index as usize);
                        player_bet_index -= 1;
                        send_to_player_by_id(&clients, player_id.as_str(), "You folded");
                        break;
                    }

                    if player.bet_amount + bet_amount >= poker_game.current_bet {
                        if player.bet_amount + bet_amount > poker_game.current_bet {
                            first_highest_bet = player.id.clone();
                            if poker_game.current_bet < 0 {
                                poker_game.current_bet = 0;
                            }
                            poker_game.current_bet = player.bet_amount + bet_amount;
                        }
                        player.money_lost += bet_amount;
                        player.bet_amount += bet_amount;
                        poker_game.pot += bet_amount;
                        break;
                    }
                }
            }        
        }
        player_bet_index += 1;
    }



    // After 4th round of betting
    update_players_folded(&players_collection, &poker_game.players.clone()).await.unwrap();

    if poker_game.current_players.len() == 1 {
        let winner_id = poker_game.current_players[0].id.clone();
        broadcast_to_game_players(&clients, format!("Game is over, winner is {}", winner_id).as_str());
        update_game_state_field(&games_collection, "winner", format!("Game is over, winner is {}", winner_id).as_str().into()).await.unwrap();

        println!("[Game] Winner determined: {}", winner_id);
        
        // Update database with results
        if let Err(e) = db::update_game_results(
            &players_collection,
            &winner_id,
            &poker_game.current_players.clone(),
            poker_game.pot,
        ).await {
            eprintln!("Failed to update game results: {}", e);
        }
        
        return;
    }



    // --- Deal 7th Street ---
    poker_game.deal_seventh_street();
    let mut hands_map: HashMap<String, Vec<String>> = HashMap::new();
    for player in &poker_game.current_players {
        let cards = player
            .hand
            .iter()
            .map(|card| format!("{}", card))
            .collect::<Vec<_>>();
            // .join(", ");

        hands_map.insert(player.id.clone(), cards.clone());
        println!(" - {}: {}", player.id, cards.join(", "));
    }

    let message = json!({
        "7 card hands": hands_map
    });
    
    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();



    // fifth round of betting
    // Resetting bet for all players to 0
    for player in &mut poker_game.current_players {
        player.bet_amount = 0;
    }

    let best_hand_id = poker_game.determine_best_faceup_hand_after_sixth_street().unwrap_or_else(|| poker_game.current_players[0].id.clone());
    // Determining player bring in index
    let best_hand_index = poker_game
        .current_players
        .iter()
        .position(|p| p.id == best_hand_id)
        .unwrap_or(0);

    let mut player_bet_index: i32 = best_hand_index as i32;
    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();

    loop {
        if player_bet_index >= poker_game.current_players.len() as i32{
            player_bet_index = 0;
        }

        let player_id = poker_game.current_players[player_bet_index as usize].id.clone();

        if player_id == first_highest_bet || poker_game.current_players.len() == 1 {
            break;
        }

        loop {
            let mut bet_amounts_map: HashMap<String, i32> = HashMap::new();
            for p in &poker_game.current_players {
                bet_amounts_map.insert(p.id.clone(), p.bet_amount);
            }


            let message = json!({
                "7 card hands": hands_map,
                "bet": player_id.clone(),
                "pot": poker_game.pot,
                "round current bet": poker_game.current_bet,
                "player bet amount": bet_amounts_map
            });
            
            let json_str = message.to_string();
            println!("JSON to send: {}", json_str);
            broadcast_to_game_players(&clients, json_str.as_str());

            update_game_state_field(&games_collection, "pot", poker_game.pot.into()).await.unwrap();
            update_game_state_field(&games_collection, "round current bet", poker_game.current_bet.into()).await.unwrap();
            let bet_amounts_value = serde_json::to_value(&bet_amounts_map).unwrap();
            update_game_state_field(&games_collection, "player current bets", bet_amounts_value).await.unwrap();
            let hands_value = serde_json::to_value(&hands_map).unwrap();
            update_game_state_field(&games_collection, "7 card hands", hands_value).await.unwrap();
            
            let filter = doc! { "name": &player_id.clone() };
            let update = doc! { "$set": { "bet_turn": true } };
            let _ = players_collection.update_one(filter.clone(), update).await;
            
            // Wait till the bet is made
            loop {
                // Sleep to avoid hammering the DB
                thread::sleep(Duration::from_millis(300));
                
                // Fetch the latest value
                if let Ok(Some(doc)) = players_collection.find_one(filter.clone()).await {
                    if let Some(bet_turn) = doc.get_bool("bet_turn").ok() {
                        if !bet_turn {
                            println!("{} has completed their bet.", player_id);
                            break;
                        }
                    }
                }
            }
            
            if let Some(bet_amount) = get_player_bet(&players_collection, &player_id).await {
                if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                    if bet_amount == -1 {
                        println!("{} folds.", player_id.clone());
                        if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                            poker_game.players.push(player.clone());
                        }
                        poker_game.current_players.remove(player_bet_index as usize);
                        player_bet_index -= 1;
                        send_to_player_by_id(&clients, player_id.as_str(), "You folded");
                        break;
                    }

                    if player.bet_amount + bet_amount >= poker_game.current_bet {
                        if player.bet_amount + bet_amount > poker_game.current_bet {
                            first_highest_bet = player.id.clone();
                            if poker_game.current_bet < 0 {
                                poker_game.current_bet = 0;
                            }
                            poker_game.current_bet = player.bet_amount + bet_amount;
                        }
                        player.money_lost += bet_amount;
                        player.bet_amount += bet_amount;
                        poker_game.pot += bet_amount;
                        break;
                    }
                }
            }        
        }
        player_bet_index += 1;
    }



    // After 5th round of betting
    update_players_folded(&players_collection, &poker_game.players.clone()).await.unwrap();

    if poker_game.current_players.len() == 1 {
        let winner_id = poker_game.current_players[0].id.clone();
        broadcast_to_game_players(&clients, format!("Game is over, winner is {}", winner_id).as_str());
        update_game_state_field(&games_collection, "winner", format!("Game is over, winner is {}", winner_id).as_str().into()).await.unwrap();

        println!("[Game] Winner determined: {}", winner_id);
        
        // Update database with results
        if let Err(e) = db::update_game_results(
            &players_collection,
            &winner_id,
            &poker_game.current_players.clone(),
            poker_game.pot,
        ).await {
            eprintln!("Failed to update game results: {}", e);
        }
        
        return;
    }




    
    // Showdown
    if let Some(winner) = poker_game.showdown() {
        println!("[Game] Winner determined: {}", winner);
        update_game_state_field(&games_collection, "winner", format!("Game is over, winner is {}", winner).as_str().into()).await.unwrap();
        
        // Create a showdown JSON message to send to all players.
        let showdown_msg = json!({
            "winner": winner,
            "showdown": hands_map,
            "pot": poker_game.pot
        })
        .to_string();
    
        broadcast_to_game_players(&clients, showdown_msg.as_str());
    }



    // Updating results after game
    if let Some(winner) = poker_game.showdown() {
        println!("[Game] Winner determined: {}", winner);
        
        // Update database with results
        if let Err(e) = db::update_game_results(
            &players_collection,
            &winner,
            &poker_game.current_players.clone(),
            poker_game.pot,
        ).await {
            eprintln!("Failed to update game results: {}", e);
        }
    }

    // Simulate game running
    thread::sleep(Duration::from_secs(5));
    println!("[Game] Game finished.");
}