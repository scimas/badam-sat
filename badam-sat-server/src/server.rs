use std::{collections::HashMap, sync::Arc};

use axum::{
    async_trait,
    extract::FromRequestParts,
    headers::{authorization::Bearer, Authorization},
    http::request::Parts,
    RequestPartsExt, TypedHeader,
};
use pasetors::{claims::ClaimsValidationRules, keys::AsymmetricKeyPair, version4::V4};
use serde::Serialize;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    errors::{ClientError, Error},
    rooms::{Action, Room},
};

#[derive(Debug)]
pub struct Server {
    // ED25519 key for signing PASETO tokens
    key_pair: AsymmetricKeyPair<V4>,
    rooms: HashMap<Uuid, Room>,
    finished_rooms: Vec<Uuid>,
    max_rooms: usize,
}

impl Server {
    pub fn new(key_pair: AsymmetricKeyPair<V4>, max_rooms: usize) -> Self {
        Server {
            key_pair,
            rooms: HashMap::new(),
            finished_rooms: Vec::new(),
            max_rooms,
        }
    }

    pub fn verify(&self, token: &str) -> Result<Player, Error> {
        let untrusted_token =
            pasetors::token::UntrustedToken::<pasetors::Public, V4>::try_from(token)
                .map_err(|_| Error::ClientError(ClientError::InvalidToken))?;
        let validation_rules = ClaimsValidationRules::new();
        let trusted_token = pasetors::public::verify(
            &self.key_pair.public,
            &untrusted_token,
            &validation_rules,
            None,
            None,
        )
        .map_err(|_| Error::ClientError(ClientError::InvalidToken))?;
        let player = Player {
            token: token.to_owned(),
            player_id: trusted_token
                .payload_claims()
                .unwrap()
                .get_claim("sub")
                .unwrap()
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            room_id: serde_json::from_value::<Uuid>(
                trusted_token
                    .payload_claims()
                    .unwrap()
                    .get_claim("room_id")
                    .unwrap()
                    .clone(),
            )
            .unwrap(),
        };
        Ok(player)
    }

    pub fn create_room(&mut self, players: usize, decks: usize) -> Result<Uuid, Error> {
        if self.max_rooms == self.rooms.len() {
            return Err(Error::ClientError(ClientError::ServerFull));
        }
        let room = Room::new(players, decks);
        let room_id = Uuid::new_v4();
        self.rooms.insert(room_id, room);
        Ok(room_id)
    }

    pub fn join(&mut self, room_id: &Uuid) -> Result<String, Error> {
        match self.rooms.get_mut(room_id) {
            Some(room) => {
                let mut claim = room.join()?;
                claim
                    .add_additional("room_id", serde_json::to_value(room_id).unwrap())
                    .unwrap();
                let token =
                    pasetors::public::sign(&self.key_pair.secret, &claim, None, None).unwrap();
                Ok(token)
            }
            None => Err(Error::ClientError(ClientError::InvalidRoomId)),
        }
    }

    pub fn play(&mut self, action: Action, player: usize, room_id: &Uuid) -> Result<(), Error> {
        match self
            .rooms
            .get_mut(room_id)
            .map(|room| room.play(action, player))
            .unwrap_or_else(|| Err(Error::ClientError(ClientError::InvalidRoomId)))
        {
            Ok(_) => {
                if self.rooms[room_id].is_game_over() {
                    self.finished_rooms.push(*room_id);
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn room(&self, room_id: &Uuid) -> Result<&Room, Error> {
        self.rooms
            .get(room_id)
            .ok_or(Error::ClientError(ClientError::InvalidRoomId))
    }

    pub fn remove_finished_rooms(&mut self) {
        for room_id in self.finished_rooms.drain(..) {
            self.rooms.remove(&room_id);
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Player {
    token: String,
    pub player_id: usize,
    pub room_id: Uuid,
}

#[async_trait]
impl FromRequestParts<Arc<RwLock<Server>>> for Player {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<RwLock<Server>>,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(token)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| Error::ClientError(ClientError::InvalidToken))?;
        state.read().await.verify(token.token())
    }
}
