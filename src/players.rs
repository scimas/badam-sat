use card_deck::standard_deck::Card;
use std::collections::HashSet;

/// A player playing a card game
#[derive(Debug, Clone)]
pub struct Player {
    hand: Vec<Card>,
    max_card_count: usize,
}

impl Player {
    /// Create a new `Player`.
    pub fn new() -> Self {
        Player {
            hand: Vec::new(),
            max_card_count: 0,
        }
    }

    /// Create a new `Player` and assign cards at the same time.
    pub fn new_with_hand(cards: Vec<Card>) -> Self {
        Player {
            max_card_count: cards.len(),
            hand: cards,
        }
    }

    /// Create a new `Player` with `hand_size` capacity for cards.
    pub fn with_capacity(hand_size: usize) -> Self {
        Player {
            hand: Vec::with_capacity(hand_size),
            max_card_count: hand_size,
        }
    }

    /// Assign cards to the `Player` from the `cards` iterator.
    ///
    /// # Panics
    /// Panics if the number of cards being assigned to the player does not
    /// match with the player's card capacity. Does not panic if the capacity
    /// was zero.
    ///
    /// # Example
    /// ```rust
    /// use badam_sat::players::Player;
    /// use badam_sat::cards::{Suit, cards_for_suit};
    /// let mut player = Player::with_capacity(13);
    /// let cards = cards_for_suit(&Suit::Clubs);
    /// player.assign_cards(cards.into_iter());
    /// ```
    pub fn assign_cards<T>(&mut self, cards: T)
    where
        T: Iterator<Item = Card>,
    {
        self.hand.extend(cards);
        if self.max_card_count != 0 {
            assert_eq!(
                self.max_card_count,
                self.hand.len(),
                "tried to assign different number of cards than the capacity of the player"
            );
        } else {
            self.max_card_count = self.hand.len();
        }
    }

    pub fn capacity(&self) -> usize {
        self.max_card_count
    }

    pub fn has_card(&self, card: &Card) -> bool {
        self.hand.contains(card)
    }

    pub fn unique_cards_in_hand(&self) -> HashSet<Card> {
        self.hand.iter().cloned().collect()
    }

    pub fn hand_len(&self) -> usize {
        self.hand.len()
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
