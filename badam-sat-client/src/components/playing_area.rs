use badam_sat::games::{CardStack, StackState};
use card_deck::standard_deck::{Card, Rank, Suit};
use futures_util::FutureExt;
use gloo_net::http::Request;
use serde::Deserialize;
use uuid::Uuid;
use yew::{html, Component, Html, Properties};

use super::player::Action;

#[derive(Debug, PartialEq)]
pub struct PlayingArea {
    card_stacks: Vec<CardStack>,
    glow: Option<Card>,
}

impl Default for PlayingArea {
    fn default() -> Self {
        let card_stacks = Suit::all_suits().into_iter().map(CardStack::new).collect();
        PlayingArea {
            card_stacks,
            glow: None,
        }
    }
}

pub enum Msg {
    QueryPlayArea,
    PlayArea(Vec<CardStack>),
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
                    .map(|stack| {
                        html! {
                            <div class={stack.suit().name().to_string()}>
                                {
                                    stack_to_html(stack.suit(), stack, self.glow.as_ref())
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
                    .send_future(query_play_area(ctx.props().room_id).map(Msg::PlayArea));
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
                    .send_future(query_last_move(ctx.props().room_id).map(Msg::LastMove));
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

async fn query_play_area(room_id: Uuid) -> Vec<CardStack> {
    let response = Request::get("/badam_sat/api/playing_area")
        .query([("room_id", room_id.to_string())])
        .send()
        .await
        .unwrap();
    let stacks: badam_sat::games::PlayingArea = response.json().await.unwrap();
    stacks.stacks().to_vec()
}

async fn query_last_move(room_id: Uuid) -> Option<Action> {
    let response = Request::get("/badam_sat/api/last_move")
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
    match stack.stack_state() {
        StackState::Empty => {
            html! {<div class="stack">{"\u{1f0a0}"}</div>}
        }
        StackState::SevenOnly => {
            let card = Card::new_normal(*suit, Rank::new(7));
            let class = if glow.is_some_and(|glower| glower == &card) {
                "seven glow"
            } else {
                "seven"
            };
            html! {<div class="stack"><p class={class}>{card.to_string()}</p></div>}
        }
        StackState::LowOnly(card) => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            let class = if glow.is_some_and(|glower| glower == card) {
                "low glow"
            } else {
                "low"
            };
            let seven_class = if glow.is_some_and(|glower| glower == &seven) {
                "seven glow"
            } else {
                "seven"
            };
            html! {
                <div class="stack">
                    <p class={seven_class}>{seven.to_string()}</p>
                    <p class={class}>{card.to_string()}</p>
                </div>
            }
        }
        StackState::HighOnly(card) => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            let class = if glow.is_some_and(|glower| glower == card) {
                "high glow"
            } else {
                "high"
            };
            let seven_class = if glow.is_some_and(|glower| glower == &seven) {
                "seven glow"
            } else {
                "seven"
            };
            html! {
                <div class="stack">
                    <p class={class}>{card.to_string()}</p>
                    <p class={seven_class}>{seven.to_string()}</p>
                </div>
            }
        }
        StackState::LowAndHigh { low, high } => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            let low_class = if glow.is_some_and(|glower| glower == low) {
                "low glow"
            } else {
                "low"
            };
            let seven_class = if glow.is_some_and(|glower| glower == &seven) {
                "seven glow"
            } else {
                "seven"
            };
            let high_class = if glow.is_some_and(|glower| glower == high) {
                "high glow"
            } else {
                "high"
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
