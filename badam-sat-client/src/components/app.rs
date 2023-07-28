use futures_util::FutureExt;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlInputElement};
use yew::{html, Component};

use super::{player::Player, playing_area::PlayingArea};

pub struct App {
    room_id: Option<Uuid>,
    token: String,
}

pub enum Msg {
    CreateRoom { players: usize, decks: usize },
    RoomCreated(Uuid),
    JoinRoom(String),
    JoinedRoom(Uuid, String),
    Error(String),
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &yew::Context<Self>) -> Self {
        Self {
            room_id: None,
            token: String::new(),
        }
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        if let Some(room_id) = self.room_id {
            html! {
                <div class="app">
                    <PlayingArea room_id={room_id}/>
                    <Player room_id={room_id} token={self.token.clone()}/>
                    <details>
                        <summary>{"Room ID"}</summary>
                        {room_id}
                    </details>
                </div>
            }
        } else {
            let create_callback = ctx.link().callback(|_| {
                let players_element = gloo_utils::document().get_element_by_id("players").unwrap();
                let players_input = HtmlInputElement::unchecked_from_js(
                    <Element as AsRef<JsValue>>::as_ref(&players_element).clone(),
                );
                let decks_element = gloo_utils::document().get_element_by_id("decks").unwrap();
                let decks_input = HtmlInputElement::unchecked_from_js(
                    <Element as AsRef<JsValue>>::as_ref(&decks_element).clone(),
                );
                Msg::CreateRoom {
                    players: players_input.value().parse().unwrap(),
                    decks: decks_input.value().parse().unwrap(),
                }
            });
            let join_callback = ctx.link().callback(|_| {
                let room_id_element = gloo_utils::document().get_element_by_id("room_id").unwrap();
                let room_id_input = HtmlInputElement::unchecked_from_js(
                    <Element as AsRef<JsValue>>::as_ref(&room_id_element).clone(),
                );
                Msg::JoinRoom(room_id_input.value())
            });
            html! {
                <div class="app">
                    <label for="room_id">{"Room ID: "}</label>
                    <input type="text" id="room_id" minlength=32 maxlength=36 size=40 placeholder="Room ID to join existing room"/>
                    <br/>
                    <button type="button" onclick={join_callback}>{"Join"}</button>
                    <br/>
                    <label for="players">{"Players: "}</label>
                    <input type="number" id="players" min=2 max=12 placeholder="Number of players"/>
                    <label for="decks">{"Decks: "}</label>
                    <input type="number" id="decks" min=1 max=4 placeholder="Number of card decks"/>
                    <br/>
                    <button type="button" onclick={create_callback}>{"Create Room"}</button>

                </div>
            }
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::CreateRoom { players, decks } => {
                ctx.link().send_future({
                    create_room(players, decks).map(|maybe_payload| match maybe_payload {
                        Ok(payload) => Msg::RoomCreated(payload.room_id),
                        Err(err) => Msg::Error(err.to_string()),
                    })
                });
                false
            }
            Msg::RoomCreated(room_id) => {
                ctx.link().send_message(Msg::JoinRoom(room_id.to_string()));
                false
            }
            Msg::JoinRoom(room_id) => {
                match Uuid::try_parse(&room_id) {
                    Ok(room_id) => {
                        let payload = RoomPayload { room_id };
                        ctx.link().send_future(async move {
                            join_room(payload)
                                .map(|maybe_join| match maybe_join {
                                    Ok(join_response) => match join_response {
                                        JoinResponse::Success { _token_type, token } => {
                                            Msg::JoinedRoom(room_id, token)
                                        }
                                        JoinResponse::ClientError(err) => Msg::Error(err),
                                    },
                                    Err(err) => Msg::Error(err.to_string()),
                                })
                                .await
                        });
                    }
                    Err(_) => ctx
                        .link()
                        .send_message(Msg::Error(AppError::NotARoomId.to_string())),
                };
                false
            }
            Msg::JoinedRoom(room_id, token) => {
                self.room_id = Some(room_id);
                self.token = token;
                true
            }
            Msg::Error(err) => {
                gloo_dialogs::alert(&err);
                false
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    GlooError(#[from] gloo_net::Error),
    #[error("not a valid room id")]
    NotARoomId,
}

#[derive(Debug, Deserialize, Serialize)]
struct RoomPayload {
    room_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JoinResponse {
    Success {
        #[serde(rename = "token_type")]
        _token_type: String,
        token: String,
    },
    ClientError(String),
}

async fn create_room(players: usize, decks: usize) -> Result<RoomPayload, AppError> {
    let response = Request::post("/badam_sat/api/create_room")
        .json(&json!({ "players": players, "decks": decks }))
        .unwrap()
        .send()
        .await?;
    let room_payload: RoomPayload = response.json().await?;
    Ok(room_payload)
}

async fn join_room(payload: RoomPayload) -> Result<JoinResponse, AppError> {
    let response = Request::post("/badam_sat/api/join")
        .json(&payload)
        .unwrap()
        .send()
        .await?;
    let join_response: JoinResponse = response.json().await?;
    Ok(join_response)
}
