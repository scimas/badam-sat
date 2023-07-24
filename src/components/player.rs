use std::{collections::HashMap, time::Duration};

use card_deck::standard_deck::{Card, Rank, Suit};
use futures_util::FutureExt;
use gloo_net::http::Request;
use serde::Serialize;
use uuid::Uuid;
use yew::{html, platform::time::sleep, Component, Html, Properties};

#[derive(Debug, PartialEq)]
pub struct Player {
    hand: HashMap<Suit, Vec<Card>>,
}

impl Default for Player {
    fn default() -> Self {
        let hand = Suit::all_suits()
            .into_iter()
            .map(|suit| (suit, Vec::new()))
            .collect();
        Player { hand }
    }
}

pub enum Msg {
    QueryHand,
    Hand(HashMap<Suit, Vec<Card>>),
    Play(Card),
    Pass,
    QueryWinner,
}

#[derive(Debug, PartialEq, Properties)]
pub struct Props {
    pub room_id: Uuid,
    pub token: String,
}

impl Component for Player {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &yew::Context<Self>) -> Self {
        ctx.link().send_message(Msg::QueryHand);
        ctx.link().send_message(Msg::QueryWinner);
        Player::default()
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
            Msg::QueryHand => {
                {
                    let token = ctx.props().token.clone();
                    ctx.link()
                        .send_future(async move { query_hand(&token).map(Msg::Hand).await });
                }
                false
            }
            Msg::Hand(hand) => {
                if self.hand == hand {
                    let token = ctx.props().token.clone();
                    ctx.link().send_future(async move {
                        sleep(Duration::from_secs(5)).await;
                        query_hand(&token).map(Msg::Hand).await
                    });
                    false
                } else {
                    self.hand = hand;
                    true
                }
            }
            Msg::Play(card) => {
                {
                    let token = ctx.props().token.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        play(&token, &Action::Play(card)).await
                    });
                    ctx.link().send_message(Msg::QueryHand);
                }
                false
            }
            Msg::Pass => {
                {
                    let token = ctx.props().token.clone();
                    wasm_bindgen_futures::spawn_local(
                        async move { play(&token, &Action::Pass).await },
                    );
                }
                false
            }
            Msg::QueryWinner => {
                {
                    let room_id = ctx.props().room_id.clone();
                    wasm_bindgen_futures::spawn_local(query_winner(room_id));
                }
                false
            }
        }
    }
}

async fn query_hand(token: &str) -> HashMap<Suit, Vec<Card>> {
    let response = Request::get("/api/my_hand")
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let mut cards: Vec<Card> = response.json().await.unwrap();
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
    hand
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

async fn query_winner(room_id: Uuid) {
    match Request::get("/api/winner")
        .query([("room_id", room_id.to_string())])
        .send()
        .await
    {
        Ok(response) => {
            let winner: serde_json::Value = response.json().await.unwrap();
            let winner_id = winner.get("id").unwrap().as_u64().unwrap() + 1;
            gloo_dialogs::alert(&format!("Player number {winner_id} won the game"));
        }
        Err(_) => gloo_dialogs::alert("Server error"),
    }
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
