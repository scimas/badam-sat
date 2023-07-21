use std::{collections::HashMap, time::Duration};

use card_deck::standard_deck::{Card, Rank, Suit};
use futures_util::{FutureExt, Stream, StreamExt};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use yew::{html, Component, Html};

pub struct Player {
    token: String,
    hand: HashMap<Suit, Vec<Card>>,
}

pub enum Msg {
    Joined(String),
    QueryHand,
    Hand(HashMap<Suit, Vec<Card>>),
    Play(Card),
    Pass,
    Ignore,
}

impl Player {
    async fn join() -> String {
        match Request::post("/api/join").send().await {
            Err(_) => String::new(),
            Ok(response) => match response.json::<JoinResponse>().await {
                Ok(JoinResponse::Payload { token, .. }) => token,
                Ok(_) | Err(_) => String::new(),
            },
        }
    }
}

impl Component for Player {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &yew::Context<Self>) -> Self {
        ctx.link().send_future(Player::join().map(Msg::Joined));
        let hand = Suit::all_suits()
            .into_iter()
            .map(|suit| (suit, Vec::new()))
            .collect();
        ctx.link()
            .send_stream(trigger_hand_query().map(|_| Msg::QueryHand));
        Player {
            token: String::new(),
            hand,
        }
    }

    fn view(&self, ctx: &yew::Context<Self>) -> yew::Html {
        html! {
            <>
                <div class="hand">
                {
                    self.hand.iter().map(|(suit, cards)| html!{
                        <div class={format!("hand_stack {}", suit.name())}>
                            {
                                cards.iter().map(|card| {
                                    let card = *card;
                                    html!{<button class="playable" onclick={ctx.link().callback(move |_| Msg::Play(card))}>{card.to_string()}</button>}}).collect::<Html>()
                            }
                        </div>
                    }).collect::<Html>()
                }
                </div>
                <button onclick={ctx.link().callback(|_| Msg::Pass)}>{"Pass"}</button>
            </>
        }
    }

    fn update(&mut self, ctx: &yew::Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Ignore => false,
            Msg::Joined(token) => {
                self.token = token;
                true
            }
            Msg::QueryHand => {
                {
                    let token = self.token.clone();
                    ctx.link().send_future(async move {
                        query_hand(&token)
                            .map(|maybe_hand| {
                                if let Some(h) = maybe_hand {
                                    Msg::Hand(h)
                                } else {
                                    Msg::Ignore
                                }
                            })
                            .await
                    });
                }
                false
            }
            Msg::Hand(hand) => {
                if self.hand == hand {
                    false
                } else {
                    self.hand = hand;
                    true
                }
            }
            Msg::Play(card) => {
                {
                    let token = self.token.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        play(&token, &Action::Play(card)).await
                    });
                    ctx.link().send_message(Msg::QueryHand);
                }
                false
            }
            Msg::Pass => {
                {
                    let token = self.token.clone();
                    wasm_bindgen_futures::spawn_local(
                        async move { play(&token, &Action::Pass).await },
                    );
                }
                false
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JoinResponse {
    Payload {
        token: String,
        #[serde(rename = "token_type")]
        _token_type: String,
    },
    Error {
        #[serde(rename = "error")]
        _error: String,
    },
}

fn trigger_hand_query() -> impl Stream<Item = ()> {
    yew::platform::time::interval(Duration::from_secs(5))
}

async fn query_hand(token: &str) -> Option<HashMap<Suit, Vec<Card>>> {
    match Request::get("/api/my_hand")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
    {
        Ok(response) => {
            let maybe_cards: Option<Vec<Card>> = response.json().await.ok();
            if let Some(mut cards) = maybe_cards {
                cards.sort_by(card_comparator);
                cards.reverse();
                let hand: HashMap<Suit, Vec<Card>> = Suit::all_suits()
                    .into_iter()
                    .map(|suit| {
                        (
                            suit,
                            cards
                                .iter()
                                .filter(|card| card.suit().unwrap() == &suit)
                                .cloned()
                                .collect::<Vec<Card>>(),
                        )
                    })
                    .collect();
                Some(hand)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

async fn play(token: &str, action: &Action) {
    match Request::post("/api/play")
        .header("Authorization", &format!("Bearer {token}"))
        .json(action)
        .unwrap()
        .send()
        .await
    {
        Ok(response) => {
            if !response.ok() {
                gloo_dialogs::alert("Invalid move")
            }
        }
        Err(_) => gloo_dialogs::alert("Server error"),
    }
}

#[derive(Debug, Serialize)]
enum Action {
    Play(Card),
    Pass,
}

fn card_comparator(c1: &Card, c2: &Card) -> std::cmp::Ordering {
    match (c1.suit().unwrap(), c2.suit().unwrap()) {
        (s1, s2) if s1 == s2 => match (c1.rank().unwrap(), c2.rank().unwrap()) {
            (r1, r2) if r1 == r2 => std::cmp::Ordering::Equal,
            (Rank::Ace, _) => std::cmp::Ordering::Less,
            (Rank::Jack, Rank::Queen | Rank::King) => std::cmp::Ordering::Less,
            (Rank::Jack, _) => std::cmp::Ordering::Greater,
            (Rank::Queen, Rank::King) => std::cmp::Ordering::Less,
            (Rank::Queen, _) => std::cmp::Ordering::Greater,
            (Rank::King, _) => std::cmp::Ordering::Greater,
            (Rank::Numeric(_), Rank::Jack | Rank::Queen | Rank::King) => std::cmp::Ordering::Less,
            (Rank::Numeric(_), Rank::Ace) => std::cmp::Ordering::Greater,
            (Rank::Numeric(r1), Rank::Numeric(r2)) => r1.cmp(r2),
        },
        (Suit::Clubs, _) => std::cmp::Ordering::Less,
        (Suit::Diamonds, Suit::Clubs) => std::cmp::Ordering::Greater,
        (Suit::Diamonds, _) => std::cmp::Ordering::Less,
        (Suit::Hearts, Suit::Spades) => std::cmp::Ordering::Less,
        (Suit::Hearts, _) => std::cmp::Ordering::Greater,
        (Suit::Spades, _) => std::cmp::Ordering::Greater,
    }
}
