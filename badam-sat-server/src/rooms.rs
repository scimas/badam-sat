use std::time::Duration;

use badam_sat::games::{BadamSat, PlayingArea, Transition};
use card_deck::standard_deck::Card;
use pasetors::claims::Claims;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};

use crate::{errors::Error, server::ServerRoomMessage};

#[derive(Debug)]
pub struct Room {
    joined_players: usize,
    game: BadamSat,
    max_player_count: usize,
    last_move: Option<Action>,
}

impl Room {
    /// Create a new room that can accommodate given amount of players and card
    /// decks.
    pub fn spawn(players: usize, decks: usize, receiver: mpsc::Receiver<ServerRoomMessage>) {
        let game = BadamSat::with_player_and_deck_capacity(players, decks);
        let room = Room {
            joined_players: 0,
            game,
            max_player_count: players,
            last_move: None,
        };
        tokio::spawn(room.run(receiver));
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<ServerRoomMessage>) {
        fn respond<T>(responder: oneshot::Sender<T>, msg: T) -> bool {
            responder.send(msg).is_ok()
        }

        while let Ok(Some(msg)) = timeout(Duration::from_secs(5 * 60), receiver.recv()).await {
            let success = match msg {
                ServerRoomMessage::AddPlayer(responder) => respond(responder, self.join()),
                ServerRoomMessage::Play {
                    action,
                    player,
                    responder,
                } => respond(responder, self.play(action, player)),
                ServerRoomMessage::GameOver(responder) => respond(responder, self.is_game_over()),
                ServerRoomMessage::LastMove(responder) => respond(responder, self.last_move),
                ServerRoomMessage::Hand { player, responder } => {
                    respond(responder, self.hand_of_player(player))
                }
                ServerRoomMessage::GameState(responder) => respond(responder, self.game_state()),
            };
            if !success {
                log::warn!("sending data to server from room failed, exiting");
                break; // The server dropped?? Need to figure out how to handle this better. Logging?
            }
        }
        log::info!("no client activity for 5 minutes, exiting room");
    }

    /// Try to join the room.
    pub fn join(&mut self) -> Result<Claims, Error> {
        if self.is_full() {
            return Err(Error::RoomFull);
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
            return Err(Error::TooEarly);
        }
        let transition = match action {
            Action::Play(card) => Transition::Play { player, card },
            Action::Pass => Transition::Pass { player },
        };
        match self.game.update(transition) {
            Ok(_) => {
                if matches!(action, Action::Play(..)) {
                    self.last_move = Some(action);
                }
                Ok(())
            }
            Err(_) => Err(Error::InvalidMove),
        }
    }

    /// Get the room's playing area.
    pub fn playing_area(&self) -> &PlayingArea {
        self.game.playing_area()
    }

    /// Get the hand of a player.
    pub fn hand_of_player(&self, player: usize) -> Result<Vec<Card>, Error> {
        self.game
            .hand_of_player(player)
            .map(|cards| cards.to_vec())
            .ok_or(Error::InvalidPlayerId)
    }

    /// Check whether the game is over.
    pub fn is_game_over(&self) -> bool {
        self.game.winner().is_some()
    }

    pub fn game_state(&self) -> GameState {
        GameState {
            playing_area: self.playing_area().clone(),
            card_counts: (0..self.joined_players)
                .map(|player| self.game.hand_len(player).unwrap())
                .collect(),
        }
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

/// Game state that does not reveal players' cards, so can be communicated with everyone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameState {
    playing_area: PlayingArea,
    card_counts: Vec<usize>,
}
