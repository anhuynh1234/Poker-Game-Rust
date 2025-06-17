//! # Deck and Hand Evaluation
//!
//! This module handles everything related to the deck of cards and poker hand rankings.
//!
//! Includes:
//! - Card and deck structures
//! - Hand ranking evaluation logic
//! - Utility functions for cards and suits
//!
//! Used by the server to deal cards and determine winning hands.
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;
use std::fmt;


/// Represents suit of a playing card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Suit::Hearts => write!(f, "Hearts"),
            Suit::Diamonds => write!(f, "Diamonds"),
            Suit::Spades => write!(f, "Spades"),
            Suit::Clubs => write!(f, "Clubs"),
        }
    }
}

/// Represents a playing card with a rank and suit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    pub rank: u8,  // 2–14 (2..10, J=11, Q=12, K=13, A=14)
    pub suit: Suit,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank_str = value_to_rank_str(self.rank);
        write!(f, "{} of {}", rank_str, self.suit)
    }
}

/// Represents a deck of playing cards.
#[derive(Debug, Clone)]
pub struct Deck {
    pub cards: Vec<Card>,
}

impl Deck {
    /// Creates a new, shuffled deck of 52 cards.
    ///
    /// # Example
    /// ```
    /// let mut deck = Deck::new();
    /// let card = deck.deal_one();
    /// ```
    pub fn new() -> Self {
        // Here, define the rank strings in the order you want to use them
        let rank_strings = ["A", "2", "3", "4", "5", "6", "7", 
                            "8", "9", "10", "J", "Q", "K"];

        let mut cards = Vec::with_capacity(52);

        for suit in [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades].iter().copied() {
            for rank_str in &rank_strings {
                // Convert from rank string (e.g. "Q") to numeric (12)
                let rank_val = rank_str_to_value(rank_str)
                    .expect("Invalid rank string encountered!");
                
                cards.push(Card { rank: rank_val, suit });
            }
        }

        let mut deck = Self { cards };
        deck.shuffle();
        deck
    }

    /// Shuffles the deck.
    pub fn shuffle(&mut self) {
        let mut rng = thread_rng();
        self.cards.shuffle(&mut rng);
    }

    /// Deals a single card from the deck (removes and returns top card).
    pub fn deal_one(&mut self) -> Option<Card> {
        self.cards.pop()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard(u8, u8, u8, u8, u8),       // "High Card: Ace, King, 10, 5, 3"
    OnePair(u8, u8, u8, u8),            // store pair rank + descending kickers
    TwoPairs(u8, u8, u8),               // top pair, lower pair, kicker
    ThreeOfAKind(u8, u8, u8),           // triple rank + remaining two cards
    Straight(u8),                       // high card of the straight (5 in [5,4,3,2,Ace-low] would be '5')
    Flush(u8, u8, u8, u8, u8),          // all same suit, ranks sorted
    FullHouse(u8, u8),                  // triple rank + pair rank
    FourOfAKind(u8, u8),                // quad rank + kicker
    StraightFlush(u8),                  // high card of straight flush
    RoyalFlush,                         // Ten, J, Q, K, A all same suit
}

/// Ranks a 5-card poker hand and returns its classification.
///
/// # Arguments
/// * `cards` - An array of 5 `Card` objects.
///
/// # Returns
/// The `HandRank` describing the best hand.
///
/// # Example
/// ```
/// let cards = [/* your 5 Card instances */];
/// let rank = rank_poker_hand(cards);
/// ```
pub fn rank_poker_hand(mut cards: [Card; 5]) -> HandRank {
    // 1) Sort by rank descending (Ace = 14 is highest).
    cards.sort_by_key(|c| c.rank);
    cards.reverse(); // highest card is first

    let ranks: Vec<u8> = cards.iter().map(|c| c.rank).collect();
    let suits: Vec<Suit> = cards.iter().map(|c| c.suit).collect();

    // 2) Check for flush
    let is_flush = suits.iter().all(|&s| s == suits[0]); // all suits the same?

    // 3) Check for straight
    let is_straight = is_5card_straight(&ranks);

    // 4) Count occurrences of each rank to detect pairs, three-of-a-kind, etc.
    //    store them in a small frequency map (rank -> count).
    let mut freq = std::collections::HashMap::new();
    for &r in &ranks {
        *freq.entry(r).or_insert(0) += 1;
    }
    
    // want them sorted by frequency, then by rank:
    // 4 of a kind => one rank with frequency 4
    // Full House => one rank freq 3, another freq 2
    // two pairs => two ranks freq 2, leftover freq 1, etc.
    let mut freq_vec: Vec<(u8, usize)> = freq.into_iter().collect();
    freq_vec.sort_by(|&(r1, c1), &(r2, c2)| {
        // sort by count desc, then rank desc
        c2.cmp(&c1).then(r2.cmp(&r1))
    });

    // 5) Identify the hand
    match (is_straight, is_flush, &freq_vec[..]) {
        // Royal Flush (straight flush, high card = Ace (14))
        (true, true, _) if ranks[0] == 14 && ranks[1] == 13 => {
            HandRank::RoyalFlush
        }
        // Straight Flush
        (true, true, _) => {
            HandRank::StraightFlush(ranks[0])
        }
        // Four of a Kind
        (_, _, &[(r_main, 4), (r_kicker, 1)]) => {
            HandRank::FourOfAKind(r_main, r_kicker)
        }
        // Full House
        (_, _, &[(r_trip, 3), (r_pair, 2)]) => {
            HandRank::FullHouse(r_trip, r_pair)
        }
        // Flush
        (false, true, _) => {
            // Return flush + the sorted ranks
            HandRank::Flush(ranks[0], ranks[1], ranks[2], ranks[3], ranks[4])
        }
        // Straight
        (true, false, _) => {
            HandRank::Straight(ranks[0])
        }
        // Three of a Kind
        (_, _, &[(r_trip, 3), (k1, 1), (k2, 1)]) => {
            // Sort your kickers descending
            let (hi_k, lo_k) = if k1 > k2 { (k1, k2) } else { (k2, k1) };
            HandRank::ThreeOfAKind(r_trip, hi_k, lo_k)
        }
        // Two Pairs
        (_, _, &[(r_hi_pair, 2), (r_lo_pair, 2), (r_kicker, 1)]) => {
            HandRank::TwoPairs(r_hi_pair, r_lo_pair, r_kicker)
        }
        // One Pair
        (_, _, &[(r_pair, 2), (k1, 1), (k2, 1), (k3, 1)]) => {
            // Sort the 3 kickers descending
            let mut kickers = [k1, k2, k3];
            kickers.sort();
            kickers.reverse();
            HandRank::OnePair(r_pair, kickers[0], kickers[1], kickers[2])
        }
        // High Card (no combos)
        _ => {
            HandRank::HighCard(ranks[0], ranks[1], ranks[2], ranks[3], ranks[4])
        }
    }
}

/// A helper function to check if five descending ranks are a straight.
fn is_5card_straight(ranks: &[u8]) -> bool {
    // For a normal descending sequence:  (e.g. [14, 13, 12, 11, 10])
    // we check if each subsequent rank is exactly 1 less than the previous
    for i in 0..4 {
        if ranks[i] != ranks[i+1] + 1 {
            return false;
        }
    }
    true
}


/// Converts a rank string (e.g. "A", "10", "J") to its numeric value.
///
/// # Returns
/// - `Some(u8)` if the string is valid.
/// - `None` if the string is invalid.
fn rank_str_to_value(rank_str: &str) -> Option<u8> {
    match rank_str {
        "2"  => Some(2),
        "3"  => Some(3),
        "4"  => Some(4),
        "5"  => Some(5),
        "6"  => Some(6),
        "7"  => Some(7),
        "8"  => Some(8),
        "9"  => Some(9),
        "10" => Some(10),
        "J"  => Some(11),
        "Q"  => Some(12),
        "K"  => Some(13),
        "A"  => Some(14),
        _    => None,
    }
}

/// Converts a numeric rank (2–14) back to a string representation.
///
/// # Panics
/// Panics if the number is outside the range 2–14. 
fn value_to_rank_str(value: u8) -> &'static str {
    match value {
        2  => "2",
        3  => "3",
        4  => "4",
        5  => "5",
        6  => "6",
        7  => "7",
        8  => "8",
        9  => "9",
        10 => "10",
        11 => "J",
        12 => "Q",
        13 => "K",
        14 => "A",
        _  => panic!("Invalid card rank"),
    }
}
