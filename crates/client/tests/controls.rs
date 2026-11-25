//! The controls lent: one plugin holds them at a time, the body stands
//! while it does, and every way they come back without the holder asking.

use client::{Client, Event, Host, Input, Key, Look, Pad, Plugin, Recipe, Turn};
use glam::{DQuat, DVec3};
use protocol::server_message;
use serde_json::{Value, json};

/// A plugin that takes the controls when told to, and says each turn what
/// they press while it holds them.
struct Player(&'static str);

impl Plugin for Player {
    fn name(&self) -> &'static str {
        self.0
    }

    fn version(&self) -> u32 {
        1
    }

    fn command(&mut self, kind: &str, _body: Value, host: &mut Host<'_>) {
        let look = Look {
            position: DVec3::new(0.0, 0.0, 1.0),
            rotation: DQuat::IDENTITY,
            shows: None,
        };
        match kind {
            "play" if host.hold_controls(Some(look)) => {}
            "play" => host.reject("no controls to hold".into()),
            _ => host.reject(format!("`{kind}` is no command of {}", self.0)),
        }
    }

    fn turn(&mut self, turn: &Turn<'_>, host: &mut Host<'_>) {
        if let Some(pad) = turn.controls {
            host.emit("pressed", &json!({ "pad": pad.bits() }));
        }
    }
}

fn server(message: server_message::Message) -> Vec<u8> {
    protocol::encode(&protocol::ServerMessage {
        message: Some(message),
    })
}

fn on(names: &[&str]) -> Vec<u8> {
    server(server_message::Message::Plugins(protocol::Plugins {
        plugins: names
            .iter()
            .map(|name| protocol::Plugin {
                name: (*name).into(),
                version: 1,
            })
            .collect(),
    }))
}

/// A client with two players on, its body settled on the ground.
fn client() -> Client {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.plug(Box::new(Player("one")));
    client.plug(Box::new(Player("two")));
    client.link_opened();
    client.receive(&on(&["one", "two"]));
    client.settled_frame();
    client.drain_events();
    client
}

fn run(client: &mut Client, input: &mut Input, frames: usize) {
    for _ in 0..frames {
        client.update(1.0 / 60.0, input);
    }
}

/// Who the controls were last said lent to, in the events drained.
fn holder(client: &mut Client) -> Option<Option<String>> {
    client
        .drain_events()
        .into_iter()
        .rev()
        .find_map(|event| match event {
            Event::ControlsChanged { plugin } => Some(plugin),
            _ => None,
        })
}

/// Lends the controls to `plugin`, and checks the seam heard it.
fn play(client: &mut Client, plugin: &str) {
    client.command_json(&format!(r#"{{"type":"{plugin}.play"}}"#));
    run(client, &mut Input::default(), 1);
    assert_eq!(holder(client), Some(Some(plugin.to_owned())));
}

/// What the holder was last said pressed, in the events drained.
fn pressed(client: &mut Client) -> Option<u8> {
    client
        .drain_events()
        .into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Plugin(body) if body["type"] == "one.pressed" => {
                body["pad"].as_u64().map(|pad| pad as u8)
            }
            _ => None,
        })
}

#[test]
fn away_from_the_world_the_holder_keeps_them_and_is_pressed_nothing() {
    let mut client = client();
    play(&mut client, "one");
    let mut input = Input::default();
    input.key(Key::Left, true);
    run(&mut client, &mut input, 1);
    assert_eq!(pressed(&mut client), Some(Pad::LEFT.bits()));
    // The window left: every key let go, and the person away.
    input.release_all();
    client.away(true);
    input.key(Key::Left, true);
    run(&mut client, &mut input, 1);
    assert!(client.controls_lent());
    assert_eq!(pressed(&mut client), None, "nothing said pressed");
    // Back: pressed again at once, nothing asked.
    client.away(false);
    run(&mut client, &mut input, 1);
    assert_eq!(pressed(&mut client), Some(Pad::LEFT.bits()));
}

#[test]
fn a_passage_takes_them_back_and_none_holds_them_through_it() {
    let mut client = client();
    play(&mut client, "one");
    client.command_json(r#"{"type":"travel","place":"0-1"}"#);
    run(&mut client, &mut Input::default(), 1);
    assert_eq!(holder(&mut client), Some(None));
    client.command_json(r#"{"type":"one.play"}"#);
    let refused = client
        .drain_events()
        .iter()
        .any(|event| matches!(event, Event::Rejected { .. }));
    assert!(refused, "a body in a passage is the passage's");
}

#[test]
fn the_body_stands_while_they_are_lent_and_walks_once_back() {
    let mut client = client();
    let mut input = Input::default();
    let stood = client.here();
    play(&mut client, "one");
    input.key(Key::Forward, true);
    run(&mut client, &mut input, 60);
    assert_eq!(client.here(), stood, "lent, the body takes no movement");
    let pressed = client.drain_events().into_iter().any(|event| {
        matches!(event, Event::Plugin(value)
            if value["type"] == "one.pressed" && value["pad"] == Pad::UP.bits())
    });
    assert!(pressed, "the key the body walks by presses up instead");
    client.command_json(r#"{"type":"leave"}"#);
    run(&mut client, &mut input, 60);
    assert_ne!(client.here(), stood, "back, the body walks");
}

#[test]
fn the_world_switching_the_holder_off_takes_them_back() {
    let mut client = client();
    play(&mut client, "one");
    client.receive(&on(&["two"]));
    run(&mut client, &mut Input::default(), 1);
    assert_eq!(holder(&mut client), Some(None));
    assert!(!client.controls_lent());
}

#[test]
fn a_second_holder_takes_them_from_the_first() {
    let mut client = client();
    play(&mut client, "one");
    play(&mut client, "two");
    let mut input = Input::default();
    input.key(Key::Right, true);
    run(&mut client, &mut input, 1);
    let pressed: Vec<_> = client
        .drain_events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Plugin(value) => Some(value["type"].as_str()?.to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(pressed, ["two.pressed"], "only the holder reads them");
}

#[test]
fn press_and_leave_are_refused_alike_while_nobody_holds_them() {
    let mut client = client();
    for command in [
        r#"{"type":"press","controls":["start"]}"#,
        r#"{"type":"leave"}"#,
    ] {
        client.command_json(command);
        let refused = client
            .drain_events()
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. }));
        assert!(refused, "{command}");
    }
}
