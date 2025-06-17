//! # Texas Hold'em Game Flow
//!
//! This module runs a full game of Texas Hold'em poker.
//! It handles the entire game process including:
//! - Small and big blinds
//! - Dealing cards (hole cards, community cards: flop, turn, river)
//! - Four rounds of betting
//! - Showdown and determining winner
//! - Broadcasting game state to players
//! - Updating the database with game results
//!
//! Note: All player interactions are done via broadcasting JSON messages
//! and waiting for players to update their actions in the MongoDB database.

use std::{
    collections::HashMap,
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
use crate::texas_holdem::*;

/// Runs a full game of Texas Hold'em.
///
/// # Flow:
/// 1. Starts the game and sets up the player list.
/// 2. Collects blinds from small and big blind players.
/// 3. Deals hole cards to each player.
/// 4. Runs multiple betting rounds:
///     - Pre-flop
///     - Post-flop
///     - Turn
///     - River
/// 5. Deals community cards at each stage (flop, turn, river).
/// 6. Handles all betting rounds by broadcasting to players and waiting for bets.
/// 7. Determines winner at showdown or when only one player remains.
/// 8. Updates the game result in the database.
/// 9. Sends final game result to all players.
///
/// # Arguments
/// * `clients` - shared list of connected clients.
/// * `player_names` - List of players in the game.
/// * `players_collection` - MongoDB collection for player data.
/// * `lobbies_collection` - MongoDB collection for lobby data.
/// * `games_collection` - MongoDB collection for ongoing games.
/// * `history_collection` - MongoDB collection for game history.
///
/// # Notes
/// - Communication is asynchronous: the server sends messages and waits for player responses.
/// - Players respond by updating their documents in the database (e.g., `bet_turn`, `bet`).
/// - If only one player remains after any betting round, they are declared the winner immediately.
/// - Player folds are handled by checking for `-1` in their bet amount.
///
/// # Broadcasts
/// - Game state is continuously broadcast to players as JSON strings:
///     - Player hands
///     - Community cards
///     - Current pot and bet amounts
///     - Whose turn it is to bet
///     - Game results and winner
///
pub async fn run_texas_game(
    clients: Arc<Mutex<HashMap<std::net::SocketAddr, ClientInfo>>>,
    player_names: Vec<String>,
    players_collection: Arc<Collection<Document>>,
    lobbies_collection: Arc<Collection<Document>>,
    games_collection: Arc<Collection<Document>>,
    history_collection: Arc<Collection<Document>>,

) {
    let variant = GAME_VARIANT.get().unwrap();
    broadcast_to_game_players(&clients, "Started the game: {}");

    let mut poker_game:TexasHoldemGame = TexasHoldemGame::new(player_names.clone());
    // poker_game.current_players = poker_game.players.clone();

    println!("[Game] Created PokerGame for variant: {}", variant);
    println!("[Game] Players: {:?}", poker_game.current_players.iter().map(|p| &p.id).collect::<Vec<_>>());



    // Collecting small and big blind
    let small_blind_index = 0;
    let big_blind_index = 1;
    let small_blind = 2;
    let big_blind = 4;

    let small_blind_player_id = poker_game.current_players[small_blind_index].id.clone();
    let big_blind_player_id = poker_game.current_players[big_blind_index].id.clone();

    if let Some(sb_player) = poker_game.current_players.get_mut(small_blind_index) {
        sb_player.bet_amount = small_blind;
        sb_player.money_lost += small_blind as i32;
        poker_game.pot += small_blind;
    }
    if let Some(bb_player) = poker_game.current_players.get_mut(big_blind_index) {
        bb_player.bet_amount = big_blind;
        bb_player.money_lost += big_blind as i32;
        poker_game.pot += big_blind;
    }

    poker_game.current_bet = big_blind;
    
    let blind_info = json!({
        "info": format!(
            "{} posted small blind ({} chips), {} posted big blind ({} chips).",
            small_blind_player_id, small_blind,
            big_blind_player_id, big_blind
        ),
        "pot": poker_game.pot
    });
    broadcast_to_game_players(&clients, &blind_info.to_string());

    update_game_state_field(&games_collection, "info", format!(
        "{} posted small blind ({} chips), {} posted big blind ({} chips).",
        small_blind_player_id, small_blind,
        big_blind_player_id, big_blind).into()).await.unwrap();
    update_game_state_field(&games_collection, "pot", poker_game.pot.clone().into()).await.unwrap();
    
    println!(
        "[Blinds] {} (SB) posts {}, {} (BB) posts {}. Pot = {}",
        small_blind_player_id, small_blind,
        big_blind_player_id, big_blind,
        poker_game.pot
    );

    thread::sleep(Duration::from_secs(3));



    // // Deal initial hole cards (2)
    poker_game.deal_hole_cards();
    let mut hands_map: HashMap<String, Vec<String>> = HashMap::new();
    for player in &poker_game.current_players {
        let cards = player.hand.iter().map(|card| format!("{}", card)).collect::<Vec<_>>();

        hands_map.insert(player.id.clone(), cards.clone());
        println!(" - {}: {}", player.id, cards.join(", "));
    }

    let mut message = json!({
        "cards": hands_map
    });
    
    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();

 

    //  pre flop (NO COMMUNITY CARDS) JUST A BETTING ROUND
    // First round of betting
    // betting to match the big blind
    let mut first_highest_bet = big_blind_player_id.clone();
    let mut player_bet_index:i32 = 2;

    loop {
        if player_bet_index >= poker_game.current_players.len() as i32{
            player_bet_index = 0;
        }

        let player_id = poker_game.current_players[player_bet_index as usize].id.clone();

        if player_id == first_highest_bet || poker_game.current_players.len() == 1  {
            break;
        }

        loop {
            let mut bet_amounts_map: HashMap<String, i32> = HashMap::new();
            for p in &poker_game.current_players {
                bet_amounts_map.insert(p.id.clone(), p.bet_amount);
            }


            message = json!({
                "cards": hands_map,
                "bet": player_id.clone(),
                "pot": poker_game.pot,
                "round current bet": poker_game.current_bet,
                "info": format!("Match the big blind, enter the amount of the big blind or -1 fold\n
                {} is small blind, {} is big blind", small_blind_player_id, big_blind_player_id),
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
            update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
            
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
                        update_game_state_field(&games_collection, "info", format!("Player {} folded", player_id).into()).await.unwrap();
                        break;
                    }

                    if player.bet_amount + bet_amount == poker_game.current_bet {
                        player.bet_amount += bet_amount;
                        poker_game.pot += bet_amount;
                        update_game_state_field(&games_collection, "info", format!("Player {} raised the bet by {}", player_id, bet_amount).into()).await.unwrap();
                        break;
                    }
                }  

            }        
        }
        player_bet_index += 1;
    }

    // Betting if the dealer wants to raise
    let mut first_highest_bet = "none".to_string();
    let mut player_bet_index = poker_game
                                                .current_players
                                                .iter()
                                                .position(|p| p.id == big_blind_player_id).unwrap() as i32;
    let mut big_blind_check = false;
    let pre_round_pot = poker_game.pot.clone();

    loop {
        if big_blind_check {
            break;
        }

        if player_bet_index >= poker_game.current_players.len() as i32{
            player_bet_index = 0;
        }

        let player_id = poker_game.current_players[player_bet_index as usize].id.clone();

        if player_id == first_highest_bet || poker_game.current_players.len() == 1  {
            break;
        }

        loop {
            let mut bet_amounts_map: HashMap<String, i32> = HashMap::new();
            for p in &poker_game.current_players {
                bet_amounts_map.insert(p.id.clone(), p.bet_amount);
            }


            if poker_game.pot == pre_round_pot && player_id == big_blind_player_id {
                message = json!({
                    "cards": hands_map,
                    "bet": player_id.clone(),
                    "pot": poker_game.pot,
                    "round current bet": poker_game.current_bet,
                    "info": format!("You are the big blind, call 0 now to end the betting round or raise the bet\n
                            {} is small blind, {} is big blind", small_blind_player_id, big_blind_player_id),
                    "player bet amount": bet_amounts_map,
                });
            }else {
                message = json!({
                    "cards": hands_map,
                    "bet": player_id.clone(),
                    "pot": poker_game.pot,
                    "round current bet": poker_game.current_bet,
                    "info": format!("{} is small blind, {} is big blind", small_blind_player_id, big_blind_player_id),
                    "player bet amount": bet_amounts_map,
                });
            }

            let json_str = message.to_string();
            println!("JSON to send: {}", json_str);
            broadcast_to_game_players(&clients, json_str.as_str());

            update_game_state_field(&games_collection, "pot", poker_game.pot.into()).await.unwrap();
            update_game_state_field(&games_collection, "round current bet", poker_game.current_bet.into()).await.unwrap();
            let bet_amounts_value = serde_json::to_value(&bet_amounts_map).unwrap();
            update_game_state_field(&games_collection, "player current bets", bet_amounts_value).await.unwrap();
            let hands_value = serde_json::to_value(&hands_map).unwrap();
            update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
            
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
                    if player.id == big_blind_player_id && poker_game.pot == pre_round_pot {
                        if !big_blind_check {
                            if bet_amount == 0 {
                                big_blind_check = true;
                                break;
                            }
                        }
                    }

                    if bet_amount == -1 {
                        println!("{} folds.", player_id.clone());
                        if let Some(player) = poker_game.current_players.iter_mut().find(|p| p.id == player_id) {
                            poker_game.players.push(player.clone());
                        }
    
                        poker_game.current_players.remove(player_bet_index as usize);
                        player_bet_index -= 1;
                        send_to_player_by_id(&clients, player_id.as_str(), "You folded");
                        update_game_state_field(&games_collection, "info", format!("Player {} folded", player_id).into()).await.unwrap();
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
                        update_game_state_field(&games_collection, "info", format!("Player {} raised the bet by {}", player_id, bet_amount).into()).await.unwrap();
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




    // flop: 3 community cards
    poker_game.deal_flop();

    let community_cards: Vec<String> = poker_game
        .community_cards
        .iter()
        .map(|card| format!("{}", card))
        .collect();
    println!("Community Cards: {}", community_cards.join(", "));

    let message = json!({
        "community": community_cards,
        "cards": hands_map
    });

    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
    let community_value = serde_json::to_value(&community_cards).unwrap();
    update_game_state_field(&games_collection, "community", community_value).await.unwrap();



    // 2nd beting
    for player in &mut poker_game.current_players {
        player.bet_amount = 0;
    }

    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();
    let mut player_bet_index:i32 = 0;
    
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
                "community": community_cards,
                "cards": hands_map,
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
            update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
            
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
                        update_game_state_field(&games_collection, "info", format!("Player {} folded", player_id).into()).await.unwrap();
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
                        update_game_state_field(&games_collection, "info", format!("Player {} raised the bet by {}", player_id, bet_amount).into()).await.unwrap();
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



    // turn: 4 community cards
    poker_game.deal_turn();

    let community_cards: Vec<String> = poker_game
        .community_cards
        .iter()
        .map(|card| format!("{}", card))
        .collect();
    println!("Community Cards: {}", community_cards.join(", "));

    let message = json!({
        "community": community_cards,
        "cards": hands_map,
        
    });

    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
    let community_value = serde_json::to_value(&community_cards).unwrap();
    update_game_state_field(&games_collection, "community", community_value).await.unwrap();



    // third round of betting
    // Resetting bet for all players to 0
    for player in &mut poker_game.current_players {
        player.bet_amount = 0;
    }

    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();
    let mut player_bet_index:i32 = 0;

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
                "cards": hands_map,
                "bet": player_id.clone(),
                "community": community_cards,
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
            update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
            
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
                        update_game_state_field(&games_collection, "info", format!("Player {} folded", player_id).into()).await.unwrap();
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
                        player.bet_amount += bet_amount;
                        player.money_lost += bet_amount;
                        poker_game.pot += bet_amount;
                        update_game_state_field(&games_collection, "info", format!("Player {} raised the bet by {}", player_id, bet_amount).into()).await.unwrap();
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
    


    // turn: 4 community cards
    poker_game.deal_river();

    let community_cards: Vec<String> = poker_game
        .community_cards
        .iter()
        .map(|card| format!("{}", card))
        .collect();
    println!("Community Cards: {}", community_cards.join(", "));

    let message = json!({
        "community": community_cards,
        "cards": hands_map
    });

    let json_str = message.to_string();
    println!("JSON to send: {}", json_str);
    broadcast_to_game_players(&clients, json_str.as_str());

    let hands_value = serde_json::to_value(&hands_map).unwrap();
    update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
    let community_value = serde_json::to_value(&community_cards).unwrap();
    update_game_state_field(&games_collection, "community", community_value).await.unwrap();
 
   

    // 4th betting round
    // Resetting bet for all players to 0
    for player in &mut poker_game.current_players {
        player.bet_amount = 0;
    }

    poker_game.current_bet = -2;
    let mut first_highest_bet = "none".to_string();
    let mut player_bet_index:i32 = 0;

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
                "cards": hands_map,
                "bet": player_id.clone(),
                "community": community_cards,
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
            update_game_state_field(&games_collection, "cards", hands_value).await.unwrap();
            
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
                        update_game_state_field(&games_collection, "info", format!("Player {} folded", player_id).into()).await.unwrap();
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
                        player.bet_amount += bet_amount;
                        poker_game.pot += bet_amount;
                        player.money_lost += bet_amount;
                        update_game_state_field(&games_collection, "info", format!("Player {} raised the bet by {}", player_id, bet_amount).into()).await.unwrap();
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


    
    // Showdown
    if let Some(winner) = poker_game.showdown() {
        println!("[Game] Winner determined: {}", winner);
        update_game_state_field(&games_collection, "winner", format!("Game is over, winner is {}", winner).as_str().into()).await.unwrap();
        
        // Create a showdown JSON message to send to all players.
        let showdown_msg = json!({
            "winner": winner,
            "showdown": hands_map,
            "pot": poker_game.pot,
            "community": community_cards
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