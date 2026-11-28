//! Tests of the rules made in the editor.

use std::rc::Rc;

use muddie_script::{Action, Engine};

use super::*;

mod keeping;
mod running;

/// The rules running as a session runs them: switched as saved.
fn run(rules: &Rules) -> Engine {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.set_switches(rules.switched_off());
    engine.exec("rules", &rules.to_luau());
    assert_eq!(engine.take_actions(), [], "the generated script failed");
    engine
}

fn send(text: &str) -> Action {
    Action::Send(text.into())
}

/// A trigger on `pattern`, with what `with` gives it.
fn trigger(pattern: &str, with: impl FnOnce(&mut Trigger)) -> Trigger {
    let mut rule = Trigger::default();
    rule.when.pattern = pattern.into();
    with(&mut rule);
    rule
}

fn alias(pattern: &str, with: impl FnOnce(&mut Alias)) -> Alias {
    let mut rule = Alias::default();
    rule.when.pattern = pattern.into();
    with(&mut rule);
    rule
}

fn key(key: &str, with: impl FnOnce(&mut Macro)) -> Macro {
    let mut rule = Macro::default();
    rule.when.key = key.into();
    with(&mut rule);
    rule
}

fn timer(tenths: u32, with: impl FnOnce(&mut Timer)) -> Timer {
    let mut rule = Timer::default();
    rule.when.tenths = tenths;
    with(&mut rule);
    rule
}
mod playing;
