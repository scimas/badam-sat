use std::time::Duration;

use badam_sat::games::{CardStack, StackState};
use card_deck::standard_deck::{Card, Rank, Suit};
use futures_util::FutureExt;
use gloo_net::http::Request;
use serde::Deserialize;
use uuid::Uuid;
use yew::{html, platform::time::sleep, Component, Html, Properties};

use super::player::Action;

#[derive(Debug, PartialEq)]
pub struct PlayingArea {
    card_stacks: Vec<CardStack>,
    glow: Option<Card>,
    card_counts: Vec<usize>,
}

impl Default for PlayingArea {
    fn default() -> Self {
        let card_stacks = Suit::all_suits().into_iter().map(CardStack::new).collect();
        PlayingArea {
            card_stacks,
            glow: None,
            card_counts: vec![],
        }
    }
}

pub enum Msg {
    QueryGameState,
    GameState(GameState),
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
        ctx.link().send_message(Msg::QueryGameState);
        PlayingArea::default()
    }

    fn view(&self, _ctx: &yew::Context<Self>) -> yew::Html {
        html! {
            <>
                <div class="card_counts">
                    {
                        self.card_counts
                            .iter()
                            .enumerate()
                            .map(|(idx, count)| html! {
                                <div class="card_count">{ format!("Player {idx}: {count}") }</div>
                            })
                            .collect::<Html>()
                    }
                </div>
                <div class="play_area">
                    {
                    Suit::all_suits().iter().map(|suit| html! {
                        <div class={suit.name().to_string() + " played_stacks"}>
                            {
                                self.card_stacks
                                    .iter()
                                    .filter(|stack| stack.suit() == suit)
                                    .map(|stack| {
                                        stack_to_html(suit, stack, self.glow.as_ref())
                                    })
                                    .collect::<Html>()
                            }
                        </div>
                    })
                    .collect::<Html>()
                }
                </div>
            </>
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::QueryGameState => {
                ctx.link()
                    .send_future(query_game_state(ctx.props().room_id).map(Msg::GameState));
                false
            }
            Msg::GameState(state) => {
                if self.card_counts != state.card_counts {
                    ctx.link().send_message(Msg::QueryLastMove);
                    self.card_counts = state.card_counts;
                    self.card_stacks = state.playing_area.stacks().to_vec();
                    if let Some(idx) = self
                        .card_counts
                        .iter()
                        .enumerate()
                        .find_map(|(idx, count)| if *count == 0 { Some(idx) } else { None })
                    {
                        gloo_dialogs::alert(&format!("Player {idx} won!"));
                    } else {
                        ctx.link().send_message(Msg::QueryGameState);
                    }
                    return true;
                }
                ctx.link().send_future(async {
                    sleep(Duration::from_secs(5)).await;
                    Msg::QueryGameState
                });
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

async fn query_game_state(room_id: Uuid) -> GameState {
    let response = Request::get("/badam_sat/api/game_state")
        .query([("room_id", room_id.to_string())])
        .send()
        .await
        .unwrap();
    response.json().await.unwrap()
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

#[derive(Debug, Deserialize)]
pub struct GameState {
    playing_area: badam_sat::games::PlayingArea,
    card_counts: Vec<usize>,
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
