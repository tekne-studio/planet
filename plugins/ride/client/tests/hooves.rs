//! A horse ridden is heard by its pace: the beats of the gait it travels
//! at, by the gait's own clock, among the steps, and nothing while it
//! stands.

use client::{Client, Group, Input, Key, Level, Recipe, Strike, voices};
use glam::DVec3;

fn heard(client: &mut Client, input: &mut Input, seconds: f64) -> Vec<Strike> {
    let mut struck = Vec::new();
    for _ in 0..(seconds * 60.0) as u32 {
        client.update(1.0 / 60.0, input);
        struck.extend(client.drain_strikes());
    }
    struck.retain(|strike| strike.voice.name.starts_with("hoof on"));
    struck
}

/// How loud a strike's voice is: its first layer's gain.
fn loudness(strike: &Strike) -> f32 {
    strike.voice.layers[0].gain
}

/// Silent standing, soft at the walk, full at the gallop, where it is, at
/// a rider's reach; silent again once it stops.
#[test]
fn a_horse_is_heard_by_its_pace_and_only_while_it_travels() {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(ride_client::plugin());
    client.rehearse(Level::Anonymous);
    let mut input = Input::default();
    heard(&mut client, &mut input, 0.1);
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    let grid = client.generator().sphere().blocks();
    client.teleport(grid.surface_point(horse.at.to_array()));
    heard(&mut client, &mut input, 0.1);
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{},"hold":true}}"#,
        horse.number
    ));
    heard(&mut client, &mut input, 2.0);
    assert_eq!(client.host("ride").way(), Some("horse"));

    let stood = heard(&mut client, &mut input, 2.0);
    assert!(stood.is_empty(), "{} hooves standing", stood.len());

    input.key(Key::Forward, true);
    heard(&mut client, &mut input, 3.0);
    let walked = heard(&mut client, &mut input, 3.0);
    assert!(walked.len() >= 6, "{} hooves walking", walked.len());
    let (ridden_m, _) = voices::HOOF_REACH_M;
    assert!(walked.iter().all(|strike| strike.reach_m == ridden_m));
    assert!(walked.iter().all(|strike| strike.group == Group::Steps));
    let at = client.host("ride").body().position;
    assert!(
        walked.last().unwrap().position.distance(at) < 3.0,
        "a hoof sounds where the horse is"
    );

    input.key(Key::Sprint, true);
    heard(&mut client, &mut input, 4.0);
    let galloped = heard(&mut client, &mut input, 3.0);
    assert!(galloped.len() >= 6, "{} hooves galloping", galloped.len());
    assert!(loudness(&galloped[0]) > 2.0 * loudness(&walked[0]));

    // Back from the walk halts it.
    input.key(Key::Sprint, false);
    input.key(Key::Forward, false);
    heard(&mut client, &mut input, 3.0);
    input.key(Key::Back, true);
    heard(&mut client, &mut input, 1.0 / 60.0);
    input.key(Key::Back, false);
    heard(&mut client, &mut input, 3.0);
    let stopped = heard(&mut client, &mut input, 2.0);
    assert!(stopped.is_empty(), "{} hooves stopped", stopped.len());
}

/// A pegasus set down from a glide at a gallop thuds once, as it touches
/// down and never while it glides, at the horse and among the steps, and
/// gallops on at its gait's next beat. Set down straight, at a halt, it
/// touches the ground unheard.
#[test]
fn a_pegasus_set_down_at_a_gallop_thuds_once_and_gallops_on() {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(ride_client::plugin());
    client.rehearse(Level::Anonymous);
    let mut input = Input::default();
    heard(&mut client, &mut input, 0.1);
    aloft_on_a_pegasus(&mut client, &mut input);

    let glided = glide(&mut client, &mut input);
    assert!(glided.is_empty(), "{} struck gliding", glided.len());
    input.key(Key::Sprint, false);
    tap(&mut client, &mut input, Key::ToggleMode);
    let (thuds, touched, first_hoof_s) = set_down(&mut client, &mut input);
    assert_eq!(thuds.len(), 1, "one set-down");
    let thud = &thuds[0];
    assert!(thud.position.distance(touched) < 3.0, "at the horse");
    assert_eq!(thud.group, Group::Steps);
    assert_eq!(thud.reach_m, voices::LEAP_REACH_M);
    // A gallop's beats fall a fifth of a second apart or closer.
    let first_hoof_s = first_hoof_s.expect("a hoof after touching down");
    assert!(first_hoof_s <= 0.25, "first hoof {first_hoof_s} s after");
    // Back, a pace at a time, halts it.
    input.key(Key::Forward, false);
    for _ in 0..5 {
        tap(&mut client, &mut input, Key::Back);
        heard(&mut client, &mut input, 0.5);
    }
    heard(&mut client, &mut input, 3.0);
    let stood = strikes(&mut client, &mut input, 2.0);
    assert!(stood.is_empty(), "{} struck standing", stood.len());

    tap(&mut client, &mut input, Key::ToggleMode);
    heard(&mut client, &mut input, 1.0);
    glide(&mut client, &mut input);
    input.key(Key::Sprint, false);
    input.key(Key::Forward, false);
    tap(&mut client, &mut input, Key::ToggleMode);
    let (thuds, _, _) = set_down(&mut client, &mut input);
    assert!(thuds.is_empty(), "{} set down at a halt", thuds.len());
}

/// Each horse near the spawn in turn is mounted until the flight key
/// lifts one: a pegasus, aloft.
fn aloft_on_a_pegasus(client: &mut Client, input: &mut Input) {
    let horses: Vec<_> = (client.host("ride").animals().iter())
        .filter(|near| !near.ridden)
        .map(|near| (near.number, near.at))
        .collect();
    let grid = client.generator().sphere().blocks();
    let flown = horses.into_iter().any(|(number, at)| {
        client.teleport(grid.surface_point(at.to_array()));
        heard(client, input, 0.1);
        client.command_json(&format!(
            r#"{{"type":"act","plugin":"ride","id":{number},"hold":true}}"#
        ));
        heard(client, input, 1.0);
        tap(client, input, Key::ToggleMode);
        heard(client, input, 1.0);
        let aloft = client.host("ride").aloft();
        if !aloft {
            client.command_json(r#"{"type":"ride.leave"}"#);
            heard(client, input, 0.5);
        }
        aloft
    });
    assert!(flown, "a pegasus near the spawn");
}

/// Climbs, then glides forward at the run: what it struck that is not a
/// wingbeat.
fn glide(client: &mut Client, input: &mut Input) -> Vec<Strike> {
    input.key(Key::Up, true);
    heard(client, input, 1.5);
    input.key(Key::Up, false);
    input.key(Key::Forward, true);
    input.key(Key::Sprint, true);
    strikes(client, input, 3.0)
}

/// Frames until it touches down, then a second on the ground: its
/// set-downs, where it touched down, and how long after its first hoof
/// fell.
fn set_down(client: &mut Client, input: &mut Input) -> (Vec<Strike>, DVec3, Option<f64>) {
    let dt = 1.0 / 60.0;
    let mut frames = 0;
    let mut thuds = Vec::new();
    while client.host("ride").aloft() {
        assert!(frames < 600, "still aloft after ten seconds");
        client.update(dt, input);
        thuds.extend(landings(client.drain_strikes()));
        frames += 1;
    }
    thuds.extend(landings(client.drain_strikes()));
    let touched = client.host("ride").body().position;
    let mut first_hoof_s = None;
    for frame in 0..60 {
        client.update(dt, input);
        let struck = client.drain_strikes();
        let hoof = struck
            .iter()
            .any(|strike| strike.voice.name.starts_with("hoof on"));
        if hoof && first_hoof_s.is_none() {
            first_hoof_s = Some(frame as f64 * dt);
        }
        thuds.extend(landings(struck));
    }
    (thuds, touched, first_hoof_s)
}

fn landings(struck: Vec<Strike>) -> impl Iterator<Item = Strike> {
    let name = voices::TOUCHDOWN.name;
    struck
        .into_iter()
        .filter(move |strike| strike.voice.name == name)
}

/// What it struck over `seconds` that is not a wingbeat.
fn strikes(client: &mut Client, input: &mut Input, seconds: f64) -> Vec<Strike> {
    let mut struck = Vec::new();
    for _ in 0..(seconds * 60.0) as u32 {
        client.update(1.0 / 60.0, input);
        struck.extend(client.drain_strikes());
    }
    struck.retain(|strike| strike.voice.name != voices::WINGBEAT.name);
    struck
}

fn tap(client: &mut Client, input: &mut Input, key: Key) {
    input.key(key, true);
    client.update(1.0 / 60.0, input);
    input.key(key, false);
}
