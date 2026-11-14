#![allow(dead_code)] // Each integration test uses a different subset of fixture helpers.
use ime_core::{Engine, Event, KeyEvent};
pub use ime_test_support::assets;
pub fn engine() -> Engine {
    Engine::new(assets(include_str!("../../../../fixtures/lexicon.tsv")))
}
pub fn key(engine: &mut Engine, name: &str) -> ime_core::Output {
    engine.handle(Event::Key(KeyEvent {
        key: name.into(),
        ..Default::default()
    }))
}
pub fn type_keys(engine: &mut Engine, input: &str) {
    for c in input.chars() {
        let name = c.to_string();
        key(engine, if c == ' ' { "space" } else { &name });
    }
}
