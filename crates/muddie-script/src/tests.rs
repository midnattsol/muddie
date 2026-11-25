//! Tests of the script engine.

use super::*;

mod data;
mod gauges;
mod line;
mod loading;
mod map;
mod rules;
mod sandbox;
mod things;

fn engine(code: &str) -> Engine {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec("test", code);
    assert_eq!(engine.take_actions(), [], "setup script failed");
    engine
}

fn send(text: &str) -> Action {
    Action::Send(text.into())
}

fn error_text(actions: &[Action]) -> &str {
    match actions {
        [Action::Error(message)] => message,
        other => panic!("expected one error, got {other:?}"),
    }
}
