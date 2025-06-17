// src/poker/seven_card_stud.rs
use crate::deck::{Card, Deck};
use crate::five_card_draw::{Player, evaluate_hand};
use crate::texas_holdem::best_hand_from_seven;

/// Represents a game of Seven Card Stud poker.
pub struct SevenCardStudGame {
    /// List of all players who started.
    pub players: Vec<Player>,
    /// The deck of cards used.
    pub deck: Deck,
    /// total chips in the pot.
    pub pot: i32,
    /// Players active in the game.
    pub current_players: Vec<Player>,
    /// The current highest bet in the round.
    pub current_bet: i32,
}

impl SevenCardStudGame {
    /// Creates a new Seven Card Stud game with the given player IDs.
    ///
    /// # Arguments
    ///
    /// * `player_ids` - A vector of player identifiers.
    pub fn new(player_ids: Vec<String>) -> Self {
        let deck = Deck::new();
        let current_players: Vec<Player> = player_ids
            .into_iter()
            .map(|id| Player {
                id,
                hand: Vec::new(),
                folded: false,
                money_won: 0,
                money_lost: 0,
                bet_amount: 0
            })
            .collect();

        Self {
            players: Vec::new(),
            deck,
            pot: 0,
            current_players,
            current_bet: 0,
        }
    }

    /// Deals Third Street: 2 face-down cards and 1 face-up card to each player.
    pub fn deal_third_street(&mut self) {
        self.deck = Deck::new();
        for player in &mut self.current_players {
            player.hand.clear();
            player.folded = false;

            // 2 face-down, 1 face-up
            for _ in 0..2 {
                if let Some(card) = self.deck.deal_one() {
                    player.hand.push(card); // face-down (hidden in UI)
                }
            }
            if let Some(card) = self.deck.deal_one() {
                player.hand.push(card); // face-up
            }
        }
    }

    /// Determines the player required to post the bring-in bet.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - ID of player who must post the bring-in.
    pub fn determine_bring_in(&self) -> Option<String> {
        self.current_players
            .iter()
            .filter(|p| !p.folded && p.hand.len() >= 3) // only active players with 3+ cards
            .min_by_key(|p| {
                let third_card = &p.hand[2]; // third card is the face-up one
                (third_card.rank, third_card.suit as u8) // compare rank, then suit for tie-break
            })
            .map(|p| p.id.clone())
    }

    /// Deals Fourth Street: one additional face-up card to each player.
    pub fn deal_fourth_street(&mut self) {
        for player in &mut self.current_players {
            if let Some(card) = self.deck.deal_one() {
                player.hand.push(card);
            }
        }
    }

    /// Determines the player with the best face-up hand after Fourth Street.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - ID of player with the best face-up hand
    pub fn determine_best_faceup_hand_id(&self) -> Option<String> {
        self.current_players
            .iter()
            .filter(|player| !player.folded && player.hand.len() >= 4)
            .map(|player| {
                // Only evaluate 3rd and 4th cards (face-up)
                let face_up = vec![player.hand[2].clone(), player.hand[3].clone()];
                let score = evaluate_hand(&face_up);
                (player.id.clone(), score)
            })
            .max_by_key(|&(_, ref score)| score.clone())
            .map(|(id, _)| id)
    }

    /// Deals Fifth Street: one additional face-up card to each player.
    pub fn deal_fifth_street(&mut self) {
        for player in &mut self.current_players {
            if let Some(card) = self.deck.deal_one() {
                player.hand.push(card);
            }
        }
    }

    /// Determines the player with the best face-up hand after Fifth Street.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - ID of player with the best face-up hand
    pub fn determine_best_faceup_hand_after_fifth_street(&self) -> Option<String> {
        self.current_players
            .iter()
            .filter(|player| !player.folded && player.hand.len() >= 5)
            .map(|player| {
                let face_up = vec![
                    player.hand[2].clone(),
                    player.hand[3].clone(),
                    player.hand[4].clone(),
                ];
                let score = evaluate_hand(&face_up);
                (player.id.clone(), score)
            })
            .max_by_key(|(_, score)| score.clone())
            .map(|(id, _)| id)
    }

    /// Deals Sixth Street: one additional face-up card to each player.
    pub fn deal_sixth_street(&mut self) {
        for player in &mut self.current_players {
            if let Some(card) = self.deck.deal_one() {
                player.hand.push(card);
            }
        }
    }

    /// Determines the player with the best face-up hand after Sixth Street.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - ID of player with the best face-up hand.
    pub fn determine_best_faceup_hand_after_sixth_street(&self) -> Option<String> {
        self.current_players
            .iter()
            .filter(|player| !player.folded && player.hand.len() >= 6)
            .map(|player| {
                let face_up = vec![
                    player.hand[2].clone(),
                    player.hand[3].clone(),
                    player.hand[4].clone(),
                    player.hand[5].clone(),
                ];
                let score = evaluate_hand(&face_up);
                (player.id.clone(), score)
            })
            .max_by_key(|(_, score)| score.clone())
            .map(|(id, _)| id)
    }

    /// Deals Seventh Street: one additional face-down card to each player.
    pub fn deal_seventh_street(&mut self) {
        for player in &mut self.current_players {
            if let Some(card) = self.deck.deal_one() {
                player.hand.push(card); // face-down (hidden in UI)
            }
        }
    }

    /// Determines the winner of the game at showdown.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - ID of the winning player.
    pub fn showdown(&self) -> Option<String> {
        let mut best_eval: Option<((u8, Vec<u8>), &Player)> = None;
        for player in &self.current_players {
            if player.folded {
                continue;
            }
            let eval = best_hand_from_seven(&player.hand);
            best_eval = match best_eval {
                Some((ref best, _)) if eval > *best => Some((eval, player)),
                None => Some((eval, player)),
                _ => best_eval,
            };
        }
        best_eval.map(|(_, player)| player.id.clone())
    }

    // pub fn distribute_rewards(&mut self, winner_id: &str) {
    //     for player in &mut self.current_players {
    //         player.games_played += 1;
    //         if player.id == winner_id {
    //             player.wins += 1;
    //             player.chips += self.pot;
    //         } else {
    //             player.losses += 1;
    //         }
    //     }
    //     self.pot = 0;
    // }
}