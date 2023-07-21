use std::{collections::HashMap, time::Duration};

use badam_sat::games::CardStack;
use card_deck::standard_deck::{Card, Rank, Suit};
use futures_util::{FutureExt, Stream, StreamExt};
use gloo_net::http::Request;
use yew::{html, Component, Html};

pub struct PlayingArea {
    card_stacks: HashMap<Suit, Vec<CardStack>>,
}

pub enum Msg {
    QueryPlayArea,
    PlayArea(HashMap<Suit, Vec<CardStack>>),
    Ignore,
}

impl Component for PlayingArea {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &yew::Context<Self>) -> Self {
        ctx.link()
            .send_stream(trigger_play_area_query().map(|_| Msg::QueryPlayArea));
        let card_stacks = Suit::all_suits()
            .into_iter()
            .map(|suit| (suit, Vec::new()))
            .collect();
        PlayingArea { card_stacks }
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
                                        .map(|stack| stack_to_html(suit, stack))
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
            Msg::Ignore => false,
            Msg::QueryPlayArea => {
                ctx.link()
                    .send_future(query_play_area().map(|maybe_stacks| {
                        if let Some(stacks) = maybe_stacks {
                            Msg::PlayArea(stacks)
                        } else {
                            Msg::Ignore
                        }
                    }));
                false
            }
            Msg::PlayArea(stacks) => {
                if self.card_stacks == stacks {
                    false
                } else {
                    self.card_stacks = stacks;
                    true
                }
            }
        }
    }
}

fn trigger_play_area_query() -> impl Stream<Item = ()> {
    yew::platform::time::interval(Duration::from_secs(1))
}

async fn query_play_area() -> Option<HashMap<Suit, Vec<CardStack>>> {
    match Request::get("/api/playing_area").send().await {
        Ok(response) => {
            let maybe_stacks: Result<badam_sat::games::PlayingArea, _> = response.json().await;
            match maybe_stacks {
                Ok(area) => Some(area.stacks().clone()),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

fn stack_to_html(suit: &Suit, stack: &CardStack) -> Html {
    match stack {
        CardStack::Empty => {
            html! {<div class="stack">{"\u{1f0a0}"}</div>}
        }
        CardStack::SevenOnly => {
            let card = Card::new_normal(*suit, Rank::new(7));
            html! {<div class="stack"><p>{card.to_string()}</p></div>}
        }
        CardStack::LowOnly(card) => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            html! {
                <div class="stack">
                    <p>{seven.to_string()}</p>
                    <p>{card.to_string()}</p>
                </div>
            }
        }
        CardStack::HighOnly(card) => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            html! {
                <div class="stack">
                    <p>{card.to_string()}</p>
                    <p>{seven.to_string()}</p>
                </div>
            }
        }
        CardStack::LowAndHigh { low, high } => {
            let seven = Card::new_normal(*suit, Rank::new(7));
            html! {
                <div class="stack">
                    <p>{high.to_string()}</p>
                    <p>{seven.to_string()}</p>
                    <p>{low.to_string()}</p>
                </div>
            }
        }
    }
}
