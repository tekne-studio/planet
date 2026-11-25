//! The action prompt: plugins offer what is near the body, the core picks
//! one, and an act on it reaches the plugin that offered it, by the
//! key, by `act` at the seam, or by name; a tap at once, a hold once its
//! ring closes.

use std::cell::Cell;
use std::rc::Rc;

use client::{
    Act, Client, Event, Hold, Host, Input, Key, Level, Offer, On, Plugin, Prompt, Recipe, Turn,
};
use serde_json::{Value, json};

/// A plugin that offers one thing each turn, where `on` puts it, with a
/// tap, a hold or both, and says over the seam which of its offers was
/// acted on and how. While `away` is set it offers the thing past reach:
/// it was left behind.
struct Thing {
    name: &'static str,
    on: fn(&Host<'_>) -> On,
    tap: Option<&'static str>,
    hold: Option<Hold>,
    away: Rc<Cell<bool>>,
}

impl Thing {
    fn tapped(name: &'static str, on: fn(&Host<'_>) -> On) -> Thing {
        Thing {
            name,
            on,
            tap: Some("use"),
            hold: None,
            away: Rc::default(),
        }
    }

    /// Held for half a second past the tap.
    fn held(name: &'static str, tap: Option<&'static str>) -> Thing {
        Thing {
            tap,
            hold: Some(HOLD),
            ..Thing::tapped(name, ahead)
        }
    }
}

const HOLD: Hold = Hold {
    verb: "take",
    ring_s: 0.5,
};

impl Plugin for Thing {
    fn name(&self) -> &'static str {
        self.name
    }

    fn version(&self) -> u32 {
        1
    }

    fn command(&mut self, kind: &str, _body: Value, host: &mut Host<'_>) {
        host.reject(format!("`{kind}` is no command of {}", self.name));
    }

    fn turn(&mut self, _turn: &Turn<'_>, host: &mut Host<'_>) {
        let on = match self.away.get() {
            true => past_reach(host),
            false => (self.on)(host),
        };
        host.offer(vec![Offer {
            id: 7,
            on,
            thing: "thing",
            tap: self.tap,
            hold: self.hold,
        }]);
    }

    fn act(&mut self, id: u64, act: Act, host: &mut Host<'_>) {
        host.emit("acted", &json!({ "id": id, "act": act }));
    }
}

/// A point a metre and a half ahead of the body, at its chest.
fn ahead(host: &Host<'_>) -> On {
    let body = host.body();
    On::Point(body.position + body.facing * 1.5 + body.up)
}

/// The same, behind it.
fn behind(host: &Host<'_>) -> On {
    let body = host.body();
    On::Point(body.position - body.facing * 1.5 + body.up)
}

/// Ahead of it, past the prompt's reach of 3 m.
fn past_reach(host: &Host<'_>) -> On {
    let body = host.body();
    On::Point(body.position + body.facing * 5.0)
}

fn on_body(_: &Host<'_>) -> On {
    On::Body
}

/// A client standing on dry land with these plugins on.
fn with(things: Vec<Thing>) -> Client {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    for thing in things {
        client.plug(Box::new(thing));
    }
    client.go_to("4-77AEYRG").expect("a place to stand");
    client.rehearse(Level::Anonymous);
    client
}

const DT: f64 = 1.0 / 60.0;

fn frame(client: &mut Client, input: &mut Input) -> Vec<Event> {
    client.update(DT, input);
    client.drain_events()
}

/// The frames of `seconds`, their events together.
fn run(client: &mut Client, input: &mut Input, seconds: f64) -> Vec<Event> {
    let frames = (seconds / DT).round() as usize;
    (0..frames).flat_map(|_| frame(client, input)).collect()
}

/// What the prompt events said, in order.
fn prompts(events: &[Event]) -> Vec<Option<(&'static str, &'static str)>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Prompt { prompt } => Some(prompt.map(|p: Prompt| (p.plugin, p.thing))),
            _ => None,
        })
        .collect()
}

/// Which plugins said they were acted on.
fn acted(events: &[Event]) -> Vec<String> {
    acts(events).into_iter().map(|(name, _)| name).collect()
}

/// Which plugins said they were acted on, and how.
fn acts(events: &[Event]) -> Vec<(String, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Plugin(value) => {
                let kind = value["type"].as_str()?;
                let act = value["act"].as_str()?;
                Some((kind.strip_suffix(".acted")?.to_owned(), act.to_owned()))
            }
            _ => None,
        })
        .collect()
}

/// How the front end was told an offer was acted on, in order.
fn answered(events: &[Event]) -> Vec<(&'static str, Act)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Acted { plugin, act, .. } => Some((*plugin, *act)),
            _ => None,
        })
        .collect()
}

/// What the ring was said to have closed to, in order.
fn rings(events: &[Event]) -> Vec<Option<f32>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Ring { closed } => Some(*closed),
            _ => None,
        })
        .collect()
}

fn rejected(events: &[Event]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::Rejected { .. }))
}

/// The world keeps these plugins alone on.
fn keep_on(client: &mut Client, names: &[&str]) {
    client.link_opened();
    client.receive(&protocol::encode(&protocol::ServerMessage {
        message: Some(protocol::server_message::Message::Plugins(
            protocol::Plugins {
                plugins: names
                    .iter()
                    .map(|name| protocol::Plugin {
                        name: (*name).into(),
                        version: 1,
                    })
                    .collect(),
            },
        )),
    }));
}

#[test]
fn nothing_offered_is_no_prompt_and_nothing_to_act_on() {
    let mut client = with(Vec::new());
    let mut input = Input::default();
    assert!(prompts(&frame(&mut client, &mut input)).is_empty());
    input.key(Key::Act, true);
    let events = frame(&mut client, &mut input);
    assert!(
        prompts(&events).is_empty() && !rejected(&events),
        "{events:?}"
    );
    client.command_json(r#"{"type":"act"}"#);
    assert!(rejected(&client.drain_events()));
}

#[test]
fn the_prompt_takes_what_is_near_before_what_the_body_is_on() {
    let mut client = with(vec![
        Thing::tapped("front", ahead),
        Thing::tapped("seat", on_body),
    ]);
    let mut input = Input::default();
    let events = frame(&mut client, &mut input);
    assert_eq!(prompts(&events), vec![Some(("front", "thing"))]);
    // Said on change alone.
    assert!(prompts(&frame(&mut client, &mut input)).is_empty());
}

#[test]
fn a_point_behind_the_body_is_prompted_and_an_agent_reads_it_behind() {
    let mut client = with(vec![Thing::tapped("back", behind)]);
    let mut input = Input::default();
    assert_eq!(
        prompts(&frame(&mut client, &mut input)),
        vec![Some(("back", "thing"))]
    );

    client.command_json(r#"{"type":"offers"}"#);
    let listed = client.drain_events();
    let [Event::Offers { offers }] = listed.as_slice() else {
        panic!("{listed:?}");
    };
    let offers = serde_json::to_value(offers).unwrap();
    assert_eq!(offers[0]["plugin"], "back");
    assert_eq!(offers[0]["on"], "point");
    assert_eq!(offers[0]["in_front"], false);

    // By name, it is acted on, and by no other id.
    client.command_json(r#"{"type":"act","plugin":"back","id":7}"#);
    assert_eq!(acted(&client.drain_events()), vec!["back"]);
    client.command_json(r#"{"type":"act","plugin":"back","id":8}"#);
    assert!(rejected(&client.drain_events()));
}

#[test]
fn an_act_reaches_the_plugin_whose_offer_the_prompt_shows() {
    let mut client = with(vec![
        Thing::tapped("front", ahead),
        Thing::tapped("seat", on_body),
    ]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    input.key(Key::Act, true);
    assert_eq!(acted(&frame(&mut client, &mut input)), vec!["front"]);
    client.command_json(r#"{"type":"act"}"#);
    assert_eq!(acted(&client.drain_events()), vec!["front"]);
    // What the body is on, by name, though something is in front.
    client.command_json(r#"{"type":"act","plugin":"seat","id":7}"#);
    assert_eq!(acted(&client.drain_events()), vec!["seat"]);
}

#[test]
fn a_plugin_switched_off_takes_its_offers_at_once() {
    let mut client = with(vec![
        Thing::tapped("front", ahead),
        Thing::tapped("seat", on_body),
    ]);
    let mut input = Input::default();
    frame(&mut client, &mut input);

    // The world keeps `seat` alone on: the prompt moves to it before the
    // next frame.
    keep_on(&mut client, &["seat"]);
    assert_eq!(
        prompts(&client.drain_events()),
        vec![Some(("seat", "thing"))]
    );
    client.command_json(r#"{"type":"act","plugin":"front","id":7}"#);
    assert!(rejected(&client.drain_events()));
}

#[test]
fn an_offer_with_no_tap_and_no_hold_is_refused_and_left_out() {
    let mut client = with(Vec::new());
    let host = &mut client.host("tamer");
    let offer = |tap, hold| Offer {
        id: 1,
        on: On::Body,
        thing: "pet",
        tap,
        hold,
    };
    let instant = Some(Hold {
        verb: "tame",
        ring_s: 0.0,
    });
    assert!(!host.offer(vec![offer(None, None), offer(Some("pat"), None)]));
    assert!(!host.offer(vec![offer(Some("pat"), instant)]));
    assert!(host.offer(vec![offer(Some("pat"), None), offer(None, Some(HOLD))]));
    let mut input = Input::default();
    let prompt = frame(&mut client, &mut input);
    assert_eq!(prompts(&prompt), vec![Some(("tamer", "pet"))]);
}

#[test]
fn a_thing_with_a_tap_alone_is_acted_on_as_the_key_goes_down() {
    let mut client = with(vec![Thing::tapped("lamp", ahead)]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    input.key(Key::Act, true);
    let events = frame(&mut client, &mut input);
    assert_eq!(acts(&events), vec![("lamp".into(), "tap".into())]);
    // The front end is told, to sweep the ring once.
    assert_eq!(answered(&events), vec![("lamp", Act::Tap)]);
    // Held on, it does nothing more, and no ring is drawn.
    let events = run(&mut client, &mut input, 1.0);
    assert!(
        acts(&events).is_empty() && rings(&events).is_empty(),
        "{events:?}"
    );
}

#[test]
fn a_thing_with_a_hold_alone_is_acted_on_once_its_ring_closes() {
    let mut client = with(vec![Thing::held("chest", None)]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    // A short press is nothing, and shows no ring.
    input.key(Key::Act, true);
    let events = frame(&mut client, &mut input);
    input.key(Key::Act, false);
    let events = [events, run(&mut client, &mut input, 0.5)].concat();
    assert!(
        acts(&events).is_empty() && rings(&events).is_empty(),
        "{events:?}"
    );

    // Held: no ring for the length of a tap, then it closes, then the act.
    input.key(Key::Act, true);
    let tap = run(&mut client, &mut input, 0.15);
    assert!(rings(&tap).is_empty(), "{tap:?}");
    let events = run(&mut client, &mut input, 0.6);
    let rings = rings(&events);
    assert!(
        rings.windows(2).all(|pair| match pair {
            [Some(a), Some(b)] => a < b,
            [Some(_), None] => true,
            _ => false,
        }),
        "{rings:?}"
    );
    assert_eq!(rings[rings.len() - 2..], [Some(1.0), None]);
    assert_eq!(acts(&events), vec![("chest".into(), "hold".into())]);
    assert_eq!(answered(&events), vec![("chest", Act::Hold)]);
}

#[test]
fn a_thing_with_both_taps_on_a_short_press_and_holds_on_a_long_one() {
    let mut client = with(vec![Thing::held("door", Some("open"))]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    input.key(Key::Act, true);
    let pressed = run(&mut client, &mut input, 0.1);
    assert!(
        acts(&pressed).is_empty(),
        "the tap waits for the key to come up"
    );
    input.key(Key::Act, false);
    let events = frame(&mut client, &mut input);
    assert_eq!(acts(&events), vec![("door".into(), "tap".into())]);
    assert!(
        rings(&[pressed, events].concat()).is_empty(),
        "a tap never flashes the ring"
    );

    input.key(Key::Act, true);
    let events = run(&mut client, &mut input, 0.8);
    assert_eq!(acts(&events), vec![("door".into(), "hold".into())]);
}

#[test]
fn a_hold_let_go_before_its_ring_closes_does_nothing() {
    let mut client = with(vec![Thing::held("door", Some("open"))]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    input.key(Key::Act, true);
    let events = run(&mut client, &mut input, 0.4);
    assert!(rings(&events).last().is_some_and(|ring| ring.is_some()));
    input.key(Key::Act, false);
    let events = run(&mut client, &mut input, 1.0);
    assert!(acts(&events).is_empty(), "{events:?}");
    assert_eq!(rings(&events), vec![None]);
}

#[test]
fn a_hold_breaks_when_its_thing_leaves_reach() {
    let door = Thing::held("door", None);
    let away = door.away.clone();
    let mut client = with(vec![door]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    input.key(Key::Act, true);
    run(&mut client, &mut input, 0.4);
    away.set(true);
    let events = run(&mut client, &mut input, 1.0);
    assert!(acts(&events).is_empty(), "{events:?}");
    assert_eq!(rings(&events), vec![None]);
    // Back within reach, the key still held is no new press.
    away.set(false);
    let events = run(&mut client, &mut input, 1.0);
    assert!(acts(&events).is_empty(), "{events:?}");
}

#[test]
fn a_hold_breaks_when_its_plugin_is_switched_off() {
    let mut client = with(vec![
        Thing::held("door", None),
        Thing::tapped("seat", on_body),
    ]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    input.key(Key::Act, true);
    run(&mut client, &mut input, 0.4);
    keep_on(&mut client, &["seat"]);
    let events = [client.drain_events(), run(&mut client, &mut input, 1.0)].concat();
    assert!(acts(&events).is_empty(), "{events:?}");
    assert_eq!(rings(&events), vec![None]);
}

#[test]
fn an_agent_holds_as_the_key_does() {
    let mut client = with(vec![Thing::held("chest", None)]);
    let mut input = Input::default();
    frame(&mut client, &mut input);
    // A hold alone is no tap.
    client.command_json(r#"{"type":"act"}"#);
    assert!(rejected(&client.drain_events()));
    client.command_json(r#"{"type":"act","hold":true}"#);
    let events = run(&mut client, &mut input, 0.8);
    assert!(rings(&events).iter().any(|ring| ring.is_some()));
    assert_eq!(acts(&events), vec![("chest".into(), "hold".into())]);
    // By name, the same.
    client.command_json(r#"{"type":"act","plugin":"chest","id":7,"hold":true}"#);
    let events = run(&mut client, &mut input, 0.8);
    assert_eq!(acts(&events), vec![("chest".into(), "hold".into())]);
}
