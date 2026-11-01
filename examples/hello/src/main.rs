use dioxus::prelude::*;

#[derive(Debug, Clone, Routable, PartialEq)]
enum Route {
    #[layout(Nav)]
    #[route("/")]
    Home {},
    #[route("/about")]
    About {},
}

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Nav() -> Element {
    rsx! {
        nav {
            Link { to: Route::Home {}, "Home" }
            Link { to: Route::About {}, "About" }
            a { href: "https://github.com/nylla8444/dioxus-vercel", "GitHub" }
        }
        main { Outlet::<Route> {} }
    }
}

#[component]
fn Home() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        h1 { "dioxus-vercel demo" }
        p {
            "A small Dioxus app, built and deployed to Vercel by "
            a { href: "https://crates.io/crates/dioxus-vercel", "dioxus-vercel" }
            " from GitHub Actions."
        }
        p { "If the button counts, the WebAssembly loaded and is running." }
        button { onclick: move |_| count += 1, "Clicked {count} times" }
    }
}

#[component]
fn About() -> Element {
    rsx! {
        h1 { "About" }
        p {
            "This page is a client-side route. Reloading it shows that Vercel serves "
            "index.html for it and the app's router takes over."
        }
    }
}
