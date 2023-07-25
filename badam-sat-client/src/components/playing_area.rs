use std::collections::HashMap;

use badam_sat::games::CardStack;
use card_deck::standard_deck::{Card, Rank, Suit};
use futures_util::FutureExt;
use gloo_net::http::Request;
use serde::Deserialize;
use uuid::Uuid;
use yew::{html, Component, Html, Properties};

use super::player::Action;

#[derive(Debug, PartialEq)]
pub struct PlayingArea {
    card_stacks: HashMap<Suit, Vec<CardStack>>,
    glow: Option<Card>,
}

impl Default for PlayingArea {
    fn default() -> Self {
        let card_stacks = Suit::all_suits()
            .into_iter()
            .map(|suit| (suit, Vec::new()))
            .collect();
        PlayingArea {
            card_stacks,
            glow: None,
        }
    }
}

pub enum Msg {
    QueryPlayArea,
    PlayArea(HashMap<Suit, Vec<CardStack>>),
    QueryLastMove,
    LastMove(Option<Action>),
}

#[derive(Debug, PartialEq, Properties)]
pub struct Props {
    pub room_id: Uuid,
}

impl Component for PlayingArea {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &yew::Context<Self>) -> Self {
        ctx.link().send_message(Msg::QueryPlayArea);
        PlayingArea::default()
    }

    fn view(&self, _ctx: &yew::Context<Self>) -> yew::Html {
        html! {
            <div class="play_area">
            {
                self.card_stacks
                    .iter()
                    .map(|(suit, stacks)| {
                        html! {
                            <div class={suit.name().to_string()}>
                                {
                                    stacks
                                        .iter()
                                        .map(|stack| stack_to_html(suit, stack, self.glow.as_ref()))
                                        .collect::<Html>()
                                }
                            </div>
                        }
                    })
                    .collect::<Html>()
            }
            </div>
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::QueryPlayArea => {
                ctx.link()
                    .send_future(query_play_area(ctx.props().room_id.clone()).map(Msg::PlayArea));
                false
            }
            Msg::PlayArea(stacks) => {
                ctx.link().send_message(Msg::QueryPlayArea);
                if self.card_stacks != stacks {
                    ctx.link().send_message(Msg::QueryLastMove);
                    self.card_stacks = stacks;
                    return true;
                }
                false
            }
            Msg::QueryLastMove => {
                ctx.link()
                    .send_future(query_last_move(ctx.props().room_id.clone()).map(Msg::LastMove));
                false
            }
            Msg::LastMove(maybe_action) => {
                if let Some(action) = maybe_action {
                    match action {
                        Action::Play(card) => {
                            self.glow = Some(card);
                            true
                        }
                        Action::Pass => false,
                    }
                } else {
                    false
                }
            }
        }
    }
}

async fn query_play_area(room_id: Uuid) -> HashMap<Suit, Vec<CardStack>> {
    let response = Request::get("/api/playing_area")
        .query([("room_id", room_id.to_string())])
        .send()
        .await
        .unwrap();
    let stacks: badam_sat::games::PlayingArea = response.json().await.unwrap();
    stacks.stacks().clone()
}

async fn query_last_move(room_id: Uuid) -> Option<Action> {
    let response = Request::get("/api/last_move")
        .query([("room_id", room_id.to_string())])
        .send()
        .await
        .unwrap();
    let deserialized: LastMoveResponse = response.json().await.unwrap();
    match deserialized {
        LastMoveResponse::Action(action) => Some(action),
        LastMoveResponse::Error { .. } => None,
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LastMoveResponse {
    Action(Action),
    Error {
        #[serde(rename = "error")]
        _error: String,
    },
}

fn stack_to_html(suit: &Suit, stack: &CardStack, glow: Option<&Card>) -> Html {
    match stack {
        CardStack::Empty => {
            html! {<div class="stack">{"\u{1f0a0}"}</div>}
        }
        CardStack::SevenOnly => {
            let card = Card::new_normal(*suit, Rank::new(7));
            let class = if glow.is_some_and(|glower| glower == &card) {
                "glow"
            } else {
                ""
            };
            html! {<div class="stack"><p class={class}>{card.to_string()}</p></div>}
        }
        CardStack::LowOnly(card) => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            let class = if glow.is_some_and(|glower| glower == card) {
                "glow"
            } else {
                ""
            };
            let seven_class = if glow.is_some_and(|glower| glower == &seven) {
                "glow"
            } else {
                ""
            };
            html! {
                <div class="stack">
                    <p class={seven_class}>{seven.to_string()}</p>
                    <p class={class}>{card.to_string()}</p>
                </div>
            }
        }
        CardStack::HighOnly(card) => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            let class = if glow.is_some_and(|glower| glower == card) {
                "glow"
            } else {
                ""
            };
            let seven_class = if glow.is_some_and(|glower| glower == &seven) {
                "glow"
            } else {
                ""
            };
            html! {
                <div class="stack">
                    <p class={class}>{card.to_string()}</p>
                    <p class={seven_class}>{seven.to_string()}</p>
                </div>
            }
        }
        CardStack::LowAndHigh { low, high } => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            let low_class = if glow.is_some_and(|glower| glower == low) {
                "glow"
            } else {
                ""
            };
            let seven_class = if glow.is_some_and(|glower| glower == &seven) {
                "glow"
            } else {
                ""
            };
            let high_class = if glow.is_some_and(|glower| glower == high) {
                "glow"
            } else {
                ""
            };
            html! {
                <div class="stack">
                    <p class={high_class}>{high.to_string()}</p>
                    <p class={seven_class}>{seven.to_string()}</p>
                    <p class={low_class}>{low.to_string()}</p>
                </div>
            }
        }
    }
}
