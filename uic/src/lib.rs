#![allow(clippy::module_inception)]
pub use uic_macros::Chainable;

pub mod components;
pub mod desktop;
pub mod utils;

pub fn init(cx: &mut gpui::App) {
    components::context_menu::init(cx);
    components::input::init(cx);
    components::modal::init(cx);
    components::notification::init(cx);
    components::toast::init(cx);
}
