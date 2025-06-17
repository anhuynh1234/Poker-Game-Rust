// src/poker/texas_holdem.rs
use crate::deck::{Card, Deck};
use crate::five_card_draw::{Player, evaluate_hand};
use itertools::Itertools;

/// Represents a Texas Hold'em poker game.
#[derive(Debug)]
pub struct TexasHoldemGame {
    /// List of all players who started
    pub players: Vec<Player>,
    /// Deck of cards used
    pub deck: Deck,
    /// Total pot size for the current game.
    pub pot: i32,
    /// The current highest bet in the round.
    pub current_bet: i32,
    /// Players who are still active.
    pub current_players: Vec<Player>,
    /// Community cards dealt to the table.
    pub community_cards: Vec<Card>,
}

impl TexasHoldemGame {
    /// Creates a new Texas Hold'em game with the given player IDs.
    ///
    /// # Arguments
    ///
    /// * `player_ids` - A vector of player identifiers.
    ///
    /// # Returns
    ///
    /// A new instance of `TexasHoldemGame`.
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
            current_bet: 0,
            current_players,
            community_cards: Vec::new(),
        }
    }

    /// Deals hole cards to each player (2 cards each).
    pub fn deal_hole_cards(&mut self) {
        self.deck = Deck::new();
        for player in &mut self.current_players {
            player.hand.clear();
            player.folded = false;
        }
        for _ in 0..2 {
            for player in &mut self.current_players {
                if let Some(card) = self.deck.deal_one() {
                    player.hand.push(card);
                }
            }
        }
    }

    /// Deals the flop: 3 community cards.
    pub fn deal_flop(&mut self) {
        self.deck.deal_one();
        for _ in 0..3 {
            if let Some(card) = self.deck.deal_one() {
                self.community_cards.push(card);
            }
        }
    }

    /// Deals the turn: 1 community card.
    pub fn deal_turn(&mut self) {
        self.deck.deal_one();
        if let Some(card) = self.deck.deal_one() {
            self.community_cards.push(card);
        }
    }

    /// Deals the river: last community card.
    pub fn deal_river(&mut self) {
        self.deck.deal_one();
        if let Some(card) = self.deck.deal_one() {
            self.community_cards.push(card);
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
            let mut seven_cards = player.hand.clone();
            seven_cards.extend(self.community_cards.clone());

            let eval = best_hand_from_seven(&seven_cards);
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

/// Evaluates the best 5-card hand from a set of 7 cards.
///
/// # Arguments
///
/// * `cards` - A slice of `Card` representing the combined hole and community cards.
///
/// # Returns
///
/// * `(u8, Vec<u8>)` - The hand ranking and kicker values for tiebreaking.
pub fn best_hand_from_seven(cards: &[Card]) -> (u8, Vec<u8>) {
    let mut best_eval = (0, Vec::new());
    for combo in cards.iter().combinations(5) {
        let five_cards: Vec<Card> = combo.into_iter().cloned().collect();
        let eval = evaluate_hand(&five_cards);
        if eval > best_eval {
            best_eval = eval;
        }
    }
    best_eval
}