use card_deck::standard_deck::{Card, Rank, StandardDeckBuilder, Suit};
use rand::thread_rng;
use std::collections::{HashMap, HashSet};

use crate::players::Player;

/// The Game.
#[derive(Debug)]
pub struct BadamSat {
    state: GameState,
    players: Vec<Player>,
    playing_area: PlayingArea,
    decks: usize,
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
pub enum Transition {
    DealCards,
    Play { player: usize, card: Card },
    Pass { player: usize },
}

/// Played [`Card`]s in a game.
#[derive(Debug)]
struct PlayingArea {
    card_stacks: HashMap<Suit, Vec<CardStack>>,
}

impl PlayingArea {
    /// Create a `PlayingArea` capable of holding cards from `decks` number of
    /// standard 52-card decks.
    fn with_deck_capacity(decks: usize) -> Self {
        let card_stacks = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades]
            .into_iter()
            .map(|suit| (suit, vec![CardStack::Empty; decks]))
            .collect();
        PlayingArea { card_stacks }
    }

    /// Try to play a [`Card`].
    fn try_play(&mut self, card: Card) -> Result<(), InvalidPlay> {
        let stacks = self.card_stacks.get_mut(&card.suit().unwrap()).unwrap();
        for stack in stacks.iter_mut() {
            match stack {
                CardStack::Empty => {
                    if card.rank().unwrap().value() == 7 {
                        *stack = CardStack::SevenOnly;
                        return Ok(());
                    }
                }
                CardStack::SevenOnly => {
                    if card.rank().unwrap().value() == 6 {
                        *stack = CardStack::LowOnly(card);
                        return Ok(());
                    } else if card.rank().unwrap().value() == 8 {
                        *stack = CardStack::HighOnly(card);
                        return Ok(());
                    }
                }
                CardStack::LowOnly(stack_card) => {
                    if card.rank().unwrap().value() == stack_card.rank().unwrap().value() - 1 {
                        *stack = CardStack::LowOnly(card);
                        return Ok(());
                    } else if card.rank().unwrap().value() == 8 {
                        *stack = CardStack::LowAndHigh {
                            low: *stack_card,
                            high: card,
                        };
                        return Ok(());
                    }
                }
                CardStack::HighOnly(stack_card) => {
                    if card.rank().unwrap().value() == stack_card.rank().unwrap().value() + 1 {
                        *stack = CardStack::HighOnly(card);
                        return Ok(());
                    } else if card.rank().unwrap().value() == 6 {
                        *stack = CardStack::LowAndHigh {
                            low: card,
                            high: *stack_card,
                        };
                        return Ok(());
                    }
                }
                CardStack::LowAndHigh { low, high } => {
                    if card.rank().unwrap().value() == low.rank().unwrap().value() - 1 {
                        *stack = CardStack::LowAndHigh {
                            low: card,
                            high: *high,
                        };
                        return Ok(());
                    } else if card.rank().unwrap().value() == high.rank().unwrap().value() + 1 {
                        *stack = CardStack::LowAndHigh {
                            low: *low,
                            high: card,
                        };
                        return Ok(());
                    }
                }
            }
        }
        Err(InvalidPlay)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("played card cannot be added to the playing area")]
struct InvalidPlay;

/// Played cards belonging to a single [`Suit`].
#[derive(Debug, Clone, Copy)]
enum CardStack {
    Empty,
    SevenOnly,
    LowOnly(Card),
    HighOnly(Card),
    LowAndHigh { low: Card, high: Card },
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
        let mut cards_taken = 0;
        for player in self.players.iter_mut() {
            let cards_to_take = player.capacity();
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
            .flat_map(|(suit, stacks)| {
                stacks.iter().flat_map(|stack| {
                    let mut cards = HashSet::with_capacity(2);
                    match stack {
                        CardStack::Empty => {
                            cards.insert(Card::new_normal(*suit, Rank::new(7)));
                        }
                        CardStack::SevenOnly => {
                            cards.insert(Card::new_normal(*suit, Rank::new(8)));
                            cards.insert(Card::new_normal(*suit, Rank::new(6)));
                        }
                        CardStack::LowOnly(card) => {
                            cards.insert(Card::new_normal(*suit, Rank::new(8)));
                            if card.rank().unwrap().value() != 1 {
                                cards.insert(Card::new_normal(
                                    *suit,
                                    Rank::new(card.rank().unwrap().value() - 1),
                                ));
                            }
                        }
                        CardStack::HighOnly(card) => {
                            cards.insert(Card::new_normal(*suit, Rank::new(6)));
                            if card.rank().unwrap().value() != 13 {
                                cards.insert(Card::new_normal(
                                    *suit,
                                    Rank::new(card.rank().unwrap().value() + 1),
                                ));
                            }
                        }
                        CardStack::LowAndHigh { low, high } => {
                            if low.rank().unwrap().value() != 1 {
                                cards.insert(Card::new_normal(
                                    *suit,
                                    Rank::new(low.rank().unwrap().value() - 1),
                                ));
                            }
                            if high.rank().unwrap().value() != 13 {
                                cards.insert(Card::new_normal(
                                    *suit,
                                    Rank::new(high.rank().unwrap().value() + 1),
                                ));
                            }
                        }
                    }
                    cards
                })
            })
            .collect();
        let player_cards = self.players[player_idx].unique_cards_in_hand();
        let mut actions: HashSet<Transition> = valid_cards
            .intersection(&player_cards)
            .into_iter()
            .map(|card| Transition::Play {
                player: player_idx,
                card: *card,
            })
            .collect();
        if actions.len() == 0 {
            actions.insert(Transition::Pass { player: player_idx });
        }
        Some(actions)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("attempted transition is not valid for the current game state")]
pub struct InvalidTransition;
