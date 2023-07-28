use badam_sat::games::{BadamSat, PlayingArea, Transition};
use card_deck::standard_deck::Card;
use pasetors::claims::Claims;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::errors::{ClientError, Error};

#[derive(Debug)]
pub struct Room {
    joined_players: usize,
    game: BadamSat,
    max_player_count: usize,
    play_area_sender: watch::Sender<PlayingArea>,
    winner_sender: watch::Sender<Winner>,
    last_move: Option<Action>,
}

impl Room {
    /// Create a new room that can accommodate given amount of players and card
    /// decks.
    pub fn new(players: usize, decks: usize) -> Self {
        let game = BadamSat::with_player_and_deck_capacity(players, decks);
        let (play_area_sender, _) = watch::channel(game.playing_area().clone());
        let (winner_sender, _) = watch::channel(Winner { id: usize::MAX });
        Room {
            joined_players: 0,
            game,
            max_player_count: players,
            play_area_sender,
            winner_sender,
            last_move: None,
        }
    }

    /// Try to join the room.
    pub fn join(&mut self) -> Result<Claims, Error> {
        if self.is_full() {
            return Err(Error::ClientError(ClientError::RoomFull));
        }
        let mut claim = Claims::new().unwrap();
        claim.subject(&self.joined_players.to_string()).unwrap();
        self.joined_players += 1;
        if self.is_full() {
            self.game.update(Transition::DealCards).unwrap();
        }
        Ok(claim)
    }

    /// Check whether the room's player capacity is full.
    pub fn is_full(&self) -> bool {
        self.max_player_count == self.joined_players
    }

    /// Attempt to play a card.
    pub fn play(&mut self, action: Action, player: usize) -> Result<(), Error> {
        if !self.is_full() {
            return Err(Error::ClientError(ClientError::TooEarly));
        }
        let transition = match action {
            Action::Play(card) => Transition::Play { player, card },
            Action::Pass => Transition::Pass { player },
        };
        match self.game.update(transition) {
            Ok(_) => {
                self.play_area_sender
                    .send_replace(self.playing_area().clone());
                if let Some(id) = self.game.winner() {
                    self.winner_sender.send_replace(Winner { id });
                }
                if matches!(action, Action::Play(..)) {
                    self.last_move = Some(action);
                }
                Ok(())
            }
            Err(_) => Err(Error::ClientError(ClientError::InvalidMove)),
        }
    }

    /// Get the room's playing area.
    pub fn playing_area(&self) -> &PlayingArea {
        self.game.playing_area()
    }

    /// Get the hand of a player.
    pub fn hand_of_player(&self, player: usize) -> Result<&[Card], Error> {
        self.game
            .hand_of_player(player)
            .ok_or(Error::ClientError(ClientError::InvalidPlayerId))
    }

    /// Get the notifier channel that communicates when the room's playing area
    /// changes.
    pub fn play_area_sender(&self) -> &watch::Sender<PlayingArea> {
        &self.play_area_sender
    }

    /// Get the notifier channel that communicates when the room has a winner.
    pub fn winner_sender(&self) -> &watch::Sender<Winner> {
        &self.winner_sender
    }

    /// Check whether the game is over.
    pub fn is_game_over(&self) -> bool {
        self.game.winner().is_some()
    }

    /// Get the last played valid move.
    pub fn last_move(&self) -> Option<&Action> {
        self.last_move.as_ref()
    }
}

/// An action that a player can take; either play a card or pass their turn.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Action {
    Play(Card),
    Pass,
}

/// Winning player Id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Winner {
    id: usize,
}
