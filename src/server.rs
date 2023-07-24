use std::collections::HashMap;

use badam_sat::games::{BadamSat, PlayingArea, Transition};
use card_deck::standard_deck::Card;
use pasetors::{
    claims::{Claims, ClaimsValidationRules},
    keys::AsymmetricKeyPair,
    version4::V4,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::watch;

use crate::errors::{ClientError, InvalidToken, JoinFail};

pub struct Server {
    // ED25519 key for signing PASETO tokens
    key_pair: AsymmetricKeyPair<V4>,
    // mapping from PASETO token to player index
    players: HashMap<String, usize>,
    // PASETO tokens for player indices
    tokens: Vec<String>,
    game: BadamSat,
    max_player_count: usize,
    play_area_sender: watch::Sender<PlayingArea>,
    winner_sender: watch::Sender<serde_json::Value>,
}

impl Server {
    pub fn new(players: usize, decks: usize, key_pair: AsymmetricKeyPair<V4>) -> Self {
        let game = BadamSat::with_player_and_deck_capacity(players, decks);
        let (play_area_sender, _) = watch::channel(game.playing_area().clone());
        let (winner_sender, _) = watch::channel(json!({}));
        Server {
            key_pair,
            players: HashMap::with_capacity(players),
            tokens: Vec::with_capacity(players),
            game,
            max_player_count: players,
            play_area_sender,
            winner_sender,
        }
    }

    pub fn verify(&self, token: &str) -> Result<usize, InvalidToken> {
        let untrusted_token =
            pasetors::token::UntrustedToken::<pasetors::Public, V4>::try_from(token)
                .map_err(|_| InvalidToken)?;
        let validation_rules = ClaimsValidationRules::new();
        let trusted_token = pasetors::public::verify(
            &self.key_pair.public,
            &untrusted_token,
            &validation_rules,
            None,
            None,
        )
        .map_err(|_| InvalidToken)?;
        Ok(trusted_token
            .payload_claims()
            .unwrap()
            .get_claim("sub")
            .unwrap()
            .as_str()
            .unwrap()
            .parse()
            .unwrap())
    }

    pub fn join(&mut self) -> Result<String, JoinFail> {
        if self.is_full() {
            return Err(JoinFail::new("server is full".into()));
        }
        let mut claim = Claims::new().unwrap();
        claim.subject(&self.players.len().to_string()).unwrap();
        let token = pasetors::public::sign(&self.key_pair.secret, &claim, None, None).unwrap();
        let current_player = self.players.len();
        self.players.insert(token.clone(), current_player);
        self.tokens.push(token.clone());
        if self.players.len() == self.max_player_count {
            self.game.update(Transition::DealCards).unwrap();
        }
        Ok(token)
    }

    pub fn is_full(&self) -> bool {
        self.max_player_count == self.players.len()
    }

    pub fn play(&mut self, action: Action, player: usize) -> Result<(), ClientError> {
        if !self.is_full() {
            return Err(ClientError::TooEarly);
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
                    self.winner_sender.send_replace(json!({ "id": id }));
                }
                Ok(())
            }
            Err(_) => Err(ClientError::InvalidMove),
        }
    }

    pub fn playing_area(&self) -> &PlayingArea {
        self.game.playing_area()
    }

    pub fn hand_of_player(&self, player: usize) -> Vec<Card> {
        self.game.hand_of_player(player).to_vec()
    }

    pub fn play_area_sender(&self) -> &watch::Sender<PlayingArea> {
        &self.play_area_sender
    }

    pub fn winner_sender(&self) -> &watch::Sender<serde_json::Value> {
        &self.winner_sender
    }
}

#[derive(Debug, Deserialize)]
pub enum Action {
    Play(Card),
    Pass,
}
