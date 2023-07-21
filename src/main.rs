use components::player::Player;
use components::playing_area::PlayingArea;

use yew::{function_component, html, Html};

mod components;

fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <>
            <PlayingArea/>
            <Player/>
        </>
    }
}
