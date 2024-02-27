use card_deck::standard_deck::{Card, Rank, StandardDeckBuilder, Suit};
use rand::thread_rng;
use std::collections::HashSet;

use crate::players::Player;

/// The Game.
#[derive(Debug)]
pub struct BadamSat {
    state: GameState,
    players: Vec<Player>,
    playing_area: PlayingArea,
    decks: usize,
    player_count: usize,
}

/// State of the [`BadamSat`].
#[derive(Debug, Clone)]
enum GameState {
    PrePlay,
    InPlay {
        player: usize,
        valid_actions: HashSet<Transition>,
    },
    Over {
        winner: usize,
    },
}

/// Transitions between [`GameState`]s.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Transition {
    DealCards,
    Play { player: usize, card: Card },
    Pass { player: usize },
}

/// Played [`Card`]s in a game.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayingArea {
    card_stacks: Vec<CardStack>,
}

impl PlayingArea {
    /// Create a `PlayingArea` capable of holding cards from `decks` number of
    /// standard 52-card decks.
    fn with_deck_capacity(decks: usize) -> Self {
        let card_stacks = Suit::all_suits()
            .into_iter()
            .flat_map(|suit| vec![CardStack::new(suit); decks])
            .collect();
        PlayingArea { card_stacks }
    }

    /// Try to play a [`Card`].
    fn try_play(&mut self, card: Card) -> Result<(), InvalidPlay> {
        for stack in self.card_stacks.iter_mut() {
            if let Ok(new_stack) = stack.add(card) {
                *stack = new_stack;
                return Ok(());
            }
        }
        Err(InvalidPlay::CardMismatch)
    }

    fn is_empty(&self) -> bool {
        self.card_stacks
            .iter()
            .all(|stack| matches!(stack.stack_state, StackState::Empty))
    }

    /// Get a reference to the internal data structure.
    pub fn stacks(&self) -> &[CardStack] {
        &self.card_stacks
    }
}

#[derive(Debug, thiserror::Error)]
#[error("played card cannot be added to the playing area")]
enum InvalidPlay {
    StackFull,
    SuitMismatch,
    RankMismatch,
    CardMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CardStack {
    suit: Suit,
    stack_state: StackState,
}

/// Played cards belonging to a single [`Suit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StackState {
    Empty,
    SevenOnly,
    LowOnly(Card),
    HighOnly(Card),
    LowAndHigh { low: Card, high: Card },
}

impl CardStack {
    /// Create a new stack for `suit` cards.
    pub fn new(suit: Suit) -> Self {
        CardStack {
            suit,
            stack_state: StackState::Empty,
        }
    }

    /// Create a new stack for `suit` cards with the initial `stack_state`.
    fn new_with_stack_state(suit: Suit, stack_state: StackState) -> Self {
        CardStack { suit, stack_state }
    }

    /// Add a card to the stack.
    fn add(&self, card: Card) -> Result<Self, InvalidPlay> {
        match (&self.suit, card.suit().unwrap()) {
            (s1, s2) if s1 == s2 => match &self.stack_state {
                StackState::Empty => {
                    if card.rank().unwrap().value() == 7 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::SevenOnly,
                        ))
                    } else {
                        Err(InvalidPlay::RankMismatch)
                    }
                }
                StackState::SevenOnly => {
                    if card.rank().unwrap().value() == 6 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::LowOnly(card),
                        ))
                    } else if card.rank().unwrap().value() == 8 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::HighOnly(card),
                        ))
                    } else {
                        Err(InvalidPlay::RankMismatch)
                    }
                }
                StackState::LowOnly(stack_card) => {
                    if card.rank().unwrap().value() == stack_card.rank().unwrap().value() - 1 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::LowOnly(card),
                        ))
                    } else if card.rank().unwrap().value() == 8 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::LowAndHigh {
                                low: *stack_card,
                                high: card,
                            },
                        ))
                    } else {
                        Err(InvalidPlay::RankMismatch)
                    }
                }
                StackState::HighOnly(stack_card) => {
                    if card.rank().unwrap().value() == stack_card.rank().unwrap().value() + 1 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::HighOnly(card),
                        ))
                    } else if card.rank().unwrap().value() == 6 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::LowAndHigh {
                                low: card,
                                high: *stack_card,
                            },
                        ))
                    } else {
                        Err(InvalidPlay::RankMismatch)
                    }
                }
                StackState::LowAndHigh { low, high } => {
                    if card.rank().unwrap().value() == low.rank().unwrap().value() - 1 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::LowAndHigh {
                                low: card,
                                high: *high,
                            },
                        ))
                    } else if card.rank().unwrap().value() == high.rank().unwrap().value() + 1 {
                        Ok(CardStack::new_with_stack_state(
                            self.suit,
                            StackState::LowAndHigh {
                                low: *low,
                                high: card,
                            },
                        ))
                    } else if low.rank().unwrap() == &Rank::Ace
                        && high.rank().unwrap() == &Rank::King
                    {
                        Err(InvalidPlay::StackFull)
                    } else {
                        Err(InvalidPlay::RankMismatch)
                    }
                }
            },
            _ => Err(InvalidPlay::SuitMismatch),
        }
    }

    /// Get the suit of the stack.
    pub fn suit(&self) -> &Suit {
        &self.suit
    }

    /// Get the state of the stack.
    pub fn stack_state(&self) -> &StackState {
        &self.stack_state
    }
}

impl BadamSat {
    /// Create a game of बदाम सात (Badam Sat) for `players` number of players
    /// played with `decks` number of decks.
    pub fn with_player_and_deck_capacity(players: usize, decks: usize) -> Self {
        let num_cards = decks * 52;
        let (cards_per_player, leftover) = (num_cards / players, num_cards % players);
        // assign cards_per_player + 1 card for every leftover card to leftover number of players
        // then assign cards_per_player to the remaining players
        let player_vec = (0..leftover)
            .map(|_| Player::with_capacity(cards_per_player + 1))
            .chain((leftover..players).map(|_| Player::with_capacity(cards_per_player)))
            .collect();
        BadamSat {
            state: GameState::PrePlay,
            players: player_vec,
            playing_area: PlayingArea::with_deck_capacity(decks),
            decks,
            player_count: players,
        }
    }

    /// Attempt to advance the game with the `action`.
    pub fn update(&mut self, action: Transition) -> Result<(), InvalidTransition> {
        match (&self.state, &action) {
            (GameState::PrePlay, Transition::DealCards) => {
                self.deal();
                self.state = GameState::InPlay {
                    player: 0,
                    valid_actions: self.find_valid_actions().expect(
                        "in pre-play stage there must be at least one valid action after dealing",
                    ),
                };
                Ok(())
            }
            (GameState::PrePlay, _) => Err(InvalidTransition),
            (GameState::InPlay { .. }, Transition::DealCards) => Err(InvalidTransition),
            (
                GameState::InPlay {
                    player,
                    valid_actions,
                },
                Transition::Play {
                    player: transition_player,
                    card,
                },
            ) => {
                if (player != transition_player) || !valid_actions.contains(&action) {
                    Err(InvalidTransition)
                } else {
                    self.playing_area.try_play(*card).unwrap();
                    self.players[*player].remove_card(card);
                    self.state = match self.find_valid_actions() {
                        Some(valid_actions) => GameState::InPlay {
                            player: (player + 1) % self.players.len(),
                            valid_actions,
                        },
                        None => GameState::Over { winner: *player },
                    };
                    Ok(())
                }
            }
            (
                GameState::InPlay {
                    player,
                    valid_actions,
                },
                Transition::Pass {
                    player: transition_player,
                },
            ) => {
                if (player != transition_player) || !valid_actions.contains(&action) {
                    Err(InvalidTransition)
                } else {
                    self.state = match self.find_valid_actions() {
                        Some(valid_actions) => GameState::InPlay {
                            player: (player + 1) % self.players.len(),
                            valid_actions,
                        },
                        None => GameState::Over { winner: *player },
                    };
                    Ok(())
                }
            }
            (GameState::Over { .. }, _) => Err(InvalidTransition),
        }
    }

    /// Retrieve the winner of the game, if any.
    pub fn winner(&self) -> Option<usize> {
        match self.state {
            GameState::Over { winner } => Some(winner),
            _ => None,
        }
    }

    /// Deal cards to the players.
    fn deal(&mut self) {
        let mut deck = StandardDeckBuilder::new().subdecks(self.decks).build();
        let mut rng = thread_rng();
        deck.shuffle(&mut rng);
        let num_cards = self.decks * 52;
        let (cards_per_player, leftover) =
            (num_cards / self.player_count, num_cards % self.player_count);
        let mut cards_taken = 0;
        for (idx, player) in self.players.iter_mut().enumerate() {
            let cards_to_take = if idx < leftover {
                cards_per_player + 1
            } else {
                cards_per_player
            };
            player.assign_cards(deck.iter().skip(cards_taken).take(cards_to_take).cloned());
            cards_taken += cards_to_take;
        }
    }

    /// Find all valid [`Transition`]s for the current state of the game.
    fn find_valid_actions(&self) -> Option<HashSet<Transition>> {
        let player_idx = match self.state {
            GameState::PrePlay => 0,
            GameState::InPlay { player, .. } => {
                if self.players[player].hand_len() == 0 {
                    return None;
                }
                (player + 1) % self.players.len()
            }
            GameState::Over { .. } => return None,
        };
        let valid_cards: HashSet<Card> = self
            .playing_area
            .card_stacks
            .iter()
            .flat_map(|stack| {
                let mut cards = HashSet::with_capacity(2);
                match stack.stack_state {
                    StackState::Empty => {
                        cards.insert(Card::new_normal(stack.suit, Rank::new(7)));
                    }
                    StackState::SevenOnly => {
                        cards.insert(Card::new_normal(stack.suit, Rank::new(8)));
                        cards.insert(Card::new_normal(stack.suit, Rank::new(6)));
                    }
                    StackState::LowOnly(card) => {
                        cards.insert(Card::new_normal(stack.suit, Rank::new(8)));
                        if card.rank().unwrap().value() != 1 {
                            cards.insert(Card::new_normal(
                                stack.suit,
                                Rank::new(card.rank().unwrap().value() - 1),
                            ));
                        }
                    }
                    StackState::HighOnly(card) => {
                        cards.insert(Card::new_normal(stack.suit, Rank::new(6)));
                        if card.rank().unwrap().value() != 13 {
                            cards.insert(Card::new_normal(
                                stack.suit,
                                Rank::new(card.rank().unwrap().value() + 1),
                            ));
                        }
                    }
                    StackState::LowAndHigh { low, high } => {
                        if low.rank().unwrap().value() != 1 {
                            cards.insert(Card::new_normal(
                                stack.suit,
                                Rank::new(low.rank().unwrap().value() - 1),
                            ));
                        }
                        if high.rank().unwrap().value() != 13 {
                            cards.insert(Card::new_normal(
                                stack.suit,
                                Rank::new(high.rank().unwrap().value() + 1),
                            ));
                        }
                    }
                }
                cards
            })
            .collect();
        let player_cards = self.players[player_idx].unique_cards_in_hand();
        let mut actions: HashSet<Transition> = valid_cards
            .intersection(&player_cards)
            .map(|card| Transition::Play {
                player: player_idx,
                card: *card,
            })
            .collect();
        // first move must be 7 of hearts
        if self.playing_area.is_empty() {
            actions.retain(|action| match action {
                Transition::DealCards => false,
                Transition::Play { card, .. } => {
                    card == &Card::new_normal(Suit::Hearts, Rank::new(7))
                }
                Transition::Pass { .. } => true,
            })
        }
        if actions.is_empty() {
            actions.insert(Transition::Pass { player: player_idx });
        }
        Some(actions)
    }

    /// Get the [`PlayingArea`] of this game.
    pub fn playing_area(&self) -> &PlayingArea {
        &self.playing_area
    }

    /// Get the hand of the `player`.
    pub fn hand_of_player(&self, player: usize) -> Option<&[Card]> {
        self.players.get(player).map(|player| player.hand())
    }

    pub fn hand_len(&self, player: usize) -> Option<usize> {
        self.players.get(player).map(|player| player.hand_len())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("attempted transition is not valid for the current game state")]
pub struct InvalidTransition;
