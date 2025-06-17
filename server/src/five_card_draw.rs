// src/poker/five_card_draw.rs
use crate::deck::{Card, Deck};
use std::collections::HashMap;

/// Represents a player in the poker game.
#[derive(Debug, Clone)]
pub struct Player {
    /// Player's unique identifier.
    pub id: String,
    /// The player's current hand of cards.
    pub hand: Vec<Card>,
    /// Indicates if the player has folded.
    pub folded: bool,
    /// Total money won by the player.
    pub money_won: i32,
    /// Total money lost by the player.
    pub money_lost: i32,
    /// Current amount the player has bet in this round.
    pub bet_amount: i32,
}


/// State of a poker game.
#[derive(Debug)]
pub struct PokerGame {
    /// List of all players who started.
    pub players: Vec<Player>,
    /// Deck of cards used.
    pub deck: Deck,
    /// Total of chips in the pot.
    pub pot: i32,
    /// Current hihgest bet in round.
    pub current_bet: i32,
    /// List of players still active in  current round.
    pub current_players: Vec<Player>,
}

impl PokerGame {
    /// Creates a new poker game with the given player IDs.
    ///
    /// # Arguments
    ///
    /// * `player_ids` - A vector of player identifiers.
    pub fn new(player_ids: Vec<String>) -> Self {
        let deck = Deck::new();
        let current_players = player_ids
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
        }
    }

    /// Deals 5 cards to each active player and resets player states.
    pub fn deal_cards(&mut self) {
        self.deck = Deck::new();
        for player in &mut self.current_players {
            player.hand.clear();
            player.folded = false;
        }
        for _ in 0..5 {
            for player in &mut self.current_players {
                if let Some(card) = self.deck.deal_one() {
                    player.hand.push(card);
                }
            }
        }
    }

    /// Determines the winner of the game based on the best hand.
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The ID of the winning player, if any.
    pub fn determine_winner_id(&self) -> Option<String> {
        self.current_players
            .iter()
            .filter(|p| !p.folded)
            .max_by_key(|p| evaluate_hand(&p.hand))
            .map(|p| p.id.clone())
    }

    // pub fn distribute_rewards(&mut self, winner_id: &str) {
    //     for player in &mut self.current_players {
    //         // if player.id == winner_id {
    //         //     player.chips += self.pot;
    //         // } else {
    //         //     player.losses += 1;
    //         // }
    //     }
    //     self.pot = 0;
    // }

    /// Replaces selected cards with new cards from the deck.
    ///
    /// # Arguments
    ///
    /// * `player_id` - The ID of the player whose cards are to be replaced.
    /// * `deck_indices` - Indices of cards in the player's hand to replace.
    pub fn replace_cards(&mut self, player_id: &str, deck_indices: &[usize]) {
        for player in &mut self.current_players {
            if player.id == player_id {
                for &idx in deck_indices {
                    if let Some(new_card) = self.deck.deal_one() {
                        if idx < player.hand.len() {
                            player.hand[idx] = new_card;
                        }
                    }
                }
            }
        }
    }
}

/// Evaluates a hand and returns its strength.
///
/// # Arguments
///
/// * `hand` - A slice of cards representing the player's hand.
///
/// # Returns
///
/// * `(u8, Vec<u8>)` - A tuple where:
///   - The first element is the rank of the hand (higher is better).
///   - The second element is list of tiebreaker card ranks.
pub fn evaluate_hand(hand: &[Card]) -> (u8, Vec<u8>) {
    let mut ranks: Vec<u8> = hand.iter().map(|card| card.rank).collect();
    ranks.sort_unstable();

    let is_flush = hand.iter().all(|card| card.suit == hand[0].suit);
    let is_straight = if ranks == vec![2, 3, 4, 5, 14] {
        true
    } else {
        ranks.windows(2).all(|w| w[1] == w[0] + 1)
    };

    let mut counts: HashMap<u8, u8> = HashMap::new();
    for &r in &ranks {
        *counts.entry(r).or_insert(0) += 1;
    }
    let mut count_values: Vec<u8> = counts.values().cloned().collect();
    count_values.sort_unstable_by(|a, b| b.cmp(a));

    if is_flush && is_straight {
        if ranks[0] == 10 {
            return (10, vec![]);
        } else {
            return (9, vec![*ranks.last().unwrap()]);
        }
    }
    if count_values == vec![4, 1] {
        let (&quad, _) = counts.iter().find(|&(_, &v)| v == 4).unwrap();
        let (&kicker, _) = counts.iter().find(|&(_, &v)| v == 1).unwrap();
        return (8, vec![quad, kicker]);
    }
    if count_values == vec![3, 2] {
        let (&three, _) = counts.iter().find(|&(_, &v)| v == 3).unwrap();
        let (&pair, _) = counts.iter().find(|&(_, &v)| v == 2).unwrap();
        return (7, vec![three, pair]);
    }
    if is_flush {
        let mut tiebreakers = ranks.clone();
        tiebreakers.sort_unstable_by(|a, b| b.cmp(a));
        return (6, tiebreakers);
    }
    if is_straight {
        return (5, vec![*ranks.last().unwrap()]);
    }
    if count_values == vec![3, 1, 1] {
        let (&three, _) = counts.iter().find(|&(_, &v)| v == 3).unwrap();
        let mut kickers: Vec<u8> = counts.iter().filter(|&(_, &v)| v == 1).map(|(&r, _)| r).collect();
        kickers.sort_unstable_by(|a, b| b.cmp(a));
        let mut tiebreakers = vec![three];
        tiebreakers.extend(kickers);
        return (4, tiebreakers);
    }
    if count_values == vec![2, 2, 1] {
        let mut pairs: Vec<u8> = counts.iter().filter(|&(_, &v)| v == 2).map(|(&r, _)| r).collect();
        pairs.sort_unstable_by(|a, b| b.cmp(a));
        let kicker = *counts.iter().find(|&(_, &v)| v == 1).unwrap().0;
        let mut tiebreakers = pairs;
        tiebreakers.push(kicker);
        return (3, tiebreakers);
    }
    if count_values == vec![2, 1, 1, 1] {
        let (&pair, _) = counts.iter().find(|&(_, &v)| v == 2).unwrap();
        let mut kickers: Vec<u8> = counts.iter().filter(|&(_, &v)| v == 1).map(|(&r, _)| r).collect();
        kickers.sort_unstable_by(|a, b| b.cmp(a));
        let mut tiebreakers = vec![pair];
        tiebreakers.extend(kickers);
        return (2, tiebreakers);
    }
    ranks.sort_unstable_by(|a, b| b.cmp(a));
    (1, ranks)
}