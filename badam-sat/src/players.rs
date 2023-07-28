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
    /// use card_deck::standard_deck::{Card, Suit, Rank};
    /// let mut player = Player::with_capacity(2);
    /// let cards = [Card::new_normal(Suit::Hearts, Rank::new(12)), Card::new_normal(Suit::Spades, Rank::new(2))];
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

    /// Remove `card` from the player's hand.
    ///
    /// # Panics
    /// Panics if the player does not have this card.
    pub fn remove_card(&mut self, card: &Card) {
        let idx = self.hand.iter().position(|hcard| hcard == card).unwrap();
        self.hand.remove(idx);
    }

    /// Get the maximum number of cards this player can hold.
    pub fn capacity(&self) -> usize {
        self.max_card_count
    }

    /// Check whether this player has the `card`.
    pub fn has_card(&self, card: &Card) -> bool {
        self.hand.contains(card)
    }

    /// Get the unique set of cards from the player's hand.
    pub fn unique_cards_in_hand(&self) -> HashSet<Card> {
        self.hand.iter().cloned().collect()
    }

    /// Get the current number of cards in the hand.
    pub fn hand_len(&self) -> usize {
        self.hand.len()
    }

    /// Get a reference to the player's cards.
    pub fn hand(&self) -> &[Card] {
        &self.hand
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use super::Player;
    use card_deck::standard_deck::{Card, Rank, Suit};

    #[test]
    #[should_panic]
    fn test_remove_non_existent_card() {
        let mut player = Player::new();
        player.assign_cards(once(Card::new_normal(Suit::Spades, Rank::Jack)));
        player.remove_card(&Card::new_normal(Suit::Clubs, Rank::Queen))
    }
}
