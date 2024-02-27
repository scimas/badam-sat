use std::collections::HashMap;

use card_deck::standard_deck::Card;
use pasetors::{claims::Claims, keys::AsymmetricSecretKey, version4::V4};

use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{
    errors::Error,
    rooms::{Action, GameState, Room},
    RouterServerMessage,
};

#[derive(Debug)]
pub(crate) struct Server {
    rooms: HashMap<Uuid, mpsc::Sender<ServerRoomMessage>>,
    max_rooms: usize,
}

pub(crate) enum ServerRoomMessage {
    AddPlayer(oneshot::Sender<Result<Claims, Error>>),
    Play {
        action: Action,
        player: usize,
        responder: oneshot::Sender<Result<(), Error>>,
    },
    GameOver(oneshot::Sender<bool>),
    LastMove(oneshot::Sender<Option<Action>>),
    Hand {
        player: usize,
        responder: oneshot::Sender<Result<Vec<Card>, Error>>,
    },
    GameState(oneshot::Sender<GameState>),
}

impl Server {
    /// Create a server that can support `max_rooms` concurrent games and uses
    /// the ED25519 `key_pair` keys for player token signing.
    pub fn spawn(max_rooms: usize, receiver: mpsc::Receiver<RouterServerMessage>) {
        let server = Server {
            rooms: HashMap::new(),
            max_rooms,
        };
        tokio::spawn(server.run(receiver));
    }

    pub async fn run(mut self, mut receiver: mpsc::Receiver<RouterServerMessage>) {
        fn respond<T>(responder: oneshot::Sender<T>, msg: T) -> bool {
            responder.send(msg).is_ok()
        }

        while let Some(msg) = receiver.recv().await {
            let success = match msg {
                RouterServerMessage::CreateRoom {
                    players,
                    decks,
                    responder,
                } => respond(responder, self.create_room(players, decks)),
                RouterServerMessage::JoinRoom {
                    room,
                    secret_key,
                    responder,
                } => respond(responder, self.join(&room, &secret_key).await),
                RouterServerMessage::Play {
                    action,
                    player,
                    room,
                    responder,
                } => respond(responder, self.play(action, player, &room).await),
                RouterServerMessage::GetHand {
                    player,
                    room,
                    responder,
                } => respond(responder, self.hand(&room, player).await),
                RouterServerMessage::LastMove { room, responder } => {
                    respond(responder, self.last_move(&room).await)
                }
                RouterServerMessage::GameState { room, responder } => {
                    respond(responder, self.game_state(&room).await)
                }
            };
            if !success {
                log::warn!("failed to send to api, exiting");
                break;
            }
            self.rooms.retain(|_, sender| !sender.is_closed());
        }
    }

    /// Create a room in the server.
    ///
    /// Currently [`ClientError::ServerFull`] is the only error this method can
    /// return.
    pub fn create_room(&mut self, players: usize, decks: usize) -> Result<Uuid, Error> {
        if self.max_rooms == self.rooms.len() {
            return Err(Error::ServerFull);
        }
        let (sender, receiver) = mpsc::channel(10);
        Room::spawn(players, decks, receiver);
        let room_id = Uuid::new_v4();
        self.rooms.insert(room_id, sender);
        Ok(room_id)
    }

    /// Join the room `room_id` in this server as a player.
    ///
    /// Currently [`ClientError::RoomFull`] and [`ClientError::InvalidRoomId`]
    /// are the only errors this method can return.
    pub async fn join(
        &self,
        room_id: &Uuid,
        secret_key: &AsymmetricSecretKey<V4>,
    ) -> Result<String, Error> {
        match self.rooms.get(room_id) {
            Some(room_sender) => {
                let (sender, receiver): (oneshot::Sender<Result<Claims, Error>>, _) =
                    oneshot::channel();
                room_sender
                    .send(ServerRoomMessage::AddPlayer(sender))
                    .await
                    .map_err(|_| Error::InvalidRoomId)?;
                let mut claim = receiver.await.map_err(|_| Error::InvalidRoomId)??;
                claim
                    .add_additional("room_id", serde_json::to_value(room_id).unwrap())
                    .unwrap();
                let token = pasetors::public::sign(secret_key, &claim, None, None).unwrap();
                Ok(token)
            }
            None => Err(Error::InvalidRoomId),
        }
    }

    /// Make the `action` playe for the `player` in the room `room_id`.
    pub async fn play(
        &mut self,
        action: Action,
        player: usize,
        room_id: &Uuid,
    ) -> Result<(), Error> {
        match self.rooms.get(room_id) {
            Some(room_sender) => {
                let (sender, receiver) = oneshot::channel();
                room_sender
                    .send(ServerRoomMessage::Play {
                        action,
                        player,
                        responder: sender,
                    })
                    .await
                    .map_err(|_| Error::InvalidRoomId)?;
                let resp: Result<(), Error> = receiver.await.map_err(|_| Error::InvalidRoomId)?;
                resp?;
                let (sender, receiver) = oneshot::channel();
                room_sender
                    .send(ServerRoomMessage::GameOver(sender))
                    .await?;
                receiver.await?;
                Ok(())
            }
            None => Err(Error::InvalidRoomId),
        }
    }

    pub async fn hand(&self, room_id: &Uuid, player: usize) -> Result<Vec<Card>, Error> {
        match self.rooms.get(room_id) {
            Some(room_sender) => {
                let (sender, receiver) = oneshot::channel();
                room_sender
                    .send(ServerRoomMessage::Hand {
                        player,
                        responder: sender,
                    })
                    .await
                    .map_err(|_| Error::InvalidRoomId)?;
                receiver.await.map_err(|_| Error::InvalidRoomId)?
            }
            None => Err(Error::InvalidRoomId),
        }
    }

    pub async fn last_move(&self, room_id: &Uuid) -> Result<Action, Error> {
        match self.rooms.get(room_id) {
            Some(room_sender) => {
                let (sender, receiver) = oneshot::channel();
                room_sender
                    .send(ServerRoomMessage::LastMove(sender))
                    .await
                    .map_err(|_| Error::InvalidRoomId)?;
                let maybe_move = receiver.await.map_err(|_| Error::InvalidRoomId)?;
                maybe_move.ok_or(Error::NoMove)
            }
            None => Err(Error::InvalidRoomId),
        }
    }

    pub async fn game_state(&self, room_id: &Uuid) -> Result<GameState, Error> {
        match self.rooms.get(room_id) {
            Some(room_sender) => {
                let (sender, receiver) = oneshot::channel();
                room_sender
                    .send(ServerRoomMessage::GameState(sender))
                    .await
                    .map_err(|_| Error::InvalidRoomId)?;
                let maybe_state = receiver.await.map_err(|_| Error::InvalidRoomId)?;
                Ok(maybe_state)
            }
            None => Err(Error::InvalidRoomId),
        }
    }
}
