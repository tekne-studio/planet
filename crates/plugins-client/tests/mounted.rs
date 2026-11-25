//! A rider moved far at once arrives mounted, whatever moves it: the pass
//! through a portal, a landmark's line of the chart. The horse goes with
//! its rider, at its pace, and is drawn once wherever it is.

use client::{Client, Event, Input, Key, Level, Recipe};

fn client() -> Client {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    for plugin in plugins_client::all() {
        client.plug(plugin);
    }
    client.rehearse(Level::Admin);
    client.set_view([1600, 900]);
    client
}

fn run(client: &mut Client, input: &mut Input, seconds: f64) -> Vec<Event> {
    let mut said = Vec::new();
    for _ in 0..(seconds * 60.0).max(1.0) as u32 {
        client.update(1.0 / 60.0, input);
        said.extend(client.drain_events());
    }
    said
}

/// A landmark placed in front of the body, hung `over` blocks above where
/// it was placed: its id.
fn landmark(client: &mut Client, input: &mut Input, name: &str, over: f64) -> u64 {
    let before: Vec<u64> = client.parts().all().map(|(id, ..)| id.0).collect();
    client.command_json(&format!(
        r#"{{"type":"items.take","item":"landmark","name":"{name}","ahead":true}}"#
    ));
    run(client, input, 0.1);
    let (id, _, part) = client
        .parts()
        .all()
        .find(|(id, ..)| !before.contains(&id.0))
        .expect("a landmark placed");
    let (id, mut at) = (id.0, part.pose.at);
    at[2] += over;
    client.command_json(&format!(r#"{{"type":"items.pick","id":{id}}}"#));
    client.command_json(&format!(
        r#"{{"type":"items.set","at":[{},{},{}]}}"#,
        at[0], at[1], at[2]
    ));
    client.command_json(r#"{"type":"items.pick","id":null}"#);
    client.command_json(r#"{"type":"items.take","item":null}"#);
    run(client, input, 0.1);
    id
}

/// The place a landmark's line of the chart travels to.
fn charted(client: &mut Client, input: &mut Input, id: u64) -> String {
    client.command_json(r#"{"type":"items.chart"}"#);
    let said = run(client, input, 0.1);
    let landmarks = said
        .into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Plugin(event) if event["type"] == "items.landmarks" => Some(event),
            _ => None,
        })
        .expect("the chart answered");
    landmarks["landmarks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"].as_u64() == Some(id))
        .and_then(|row| row["place"].as_str())
        .expect("the landmark charted")
        .to_owned()
}

/// The animals drawn near the body, by number, and the one ridden.
fn herd(client: &mut Client) -> (Vec<u64>, Vec<u64>) {
    let ride = client.host("ride");
    let animals = ride.animals();
    let mut all: Vec<u64> = animals.iter().map(|near| near.number).collect();
    all.sort_unstable();
    let ridden = animals
        .iter()
        .filter(|near| near.ridden)
        .map(|near| near.number)
        .collect();
    (all, ridden)
}

/// Until a passage is over: the picture drawn in and let out.
fn passed(client: &mut Client, input: &mut Input) {
    let mut frames = 0;
    while client.update(1.0 / 60.0, input).warp.is_some() {
        client.drain_events();
        frames += 1;
        assert!(frames < 60 * 20, "the passage ends");
    }
    run(client, input, 0.5);
}

#[test]
fn a_rider_arrives_mounted_through_a_portal_and_a_landmark() {
    let mut client = client();
    let mut input = Input::default();
    run(&mut client, &mut input, 0.1);
    let grid = client.generator().sphere().blocks();

    // A landmark far off, on open ground.
    client.go_to("5-77AEYRG").unwrap();
    run(&mut client, &mut input, 1.0);
    let far = landmark(&mut client, &mut input, "Far", 0.0);

    // Home by a herd: a landmark hung three blocks up, as one on a floor
    // is, and a horse there mounted.
    let horse = {
        client.go_to("0-JNAQ39R").unwrap();
        run(&mut client, &mut input, 1.0);
        client
            .host("ride")
            .animals()
            .iter()
            .copied()
            .find(|near| !near.ridden)
            .expect("a horse near the spawn")
    };
    client.teleport(grid.surface_point(horse.at.to_array()));
    run(&mut client, &mut input, 0.5);
    let home = landmark(&mut client, &mut input, "Home", 3.0);
    let (herd_before, _) = herd(&mut client);
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{},"hold":true}}"#,
        horse.number
    ));
    run(&mut client, &mut input, 2.0);
    assert_eq!(client.host("ride").way(), Some("horse"));

    // A portal ahead to the far landmark, walked through.
    client.command_json(&format!(
        r#"{{"type":"items.take","item":"portal","to":{far},"ahead":true}}"#
    ));
    run(&mut client, &mut input, 0.1);
    client.command_json(r#"{"type":"items.take","item":null}"#);
    input.key(Key::Forward, true);
    run(&mut client, &mut input, 1.0 / 60.0);
    input.key(Key::Forward, false);
    let mut frames = 0;
    while client.update(1.0 / 60.0, &mut input).warp.is_none() {
        client.drain_events();
        frames += 1;
        assert!(frames < 60 * 10, "the horse walks into the portal");
    }
    passed(&mut client, &mut input);
    assert_eq!(
        client.host("ride").way(),
        Some("horse"),
        "mounted through the portal"
    );
    let there = charted(&mut client, &mut input, far);
    let here = client.here().place(grid);
    assert_eq!(
        here[..8],
        there[..8],
        "at the far landmark: {here}, {there}"
    );
    let (_, ridden) = herd(&mut client);
    assert_eq!(ridden, vec![horse.number], "the same horse, once");
    let (all, _) = herd(&mut client);
    assert_eq!(
        all.iter().filter(|&&number| number == horse.number).count(),
        1
    );

    // Home again by the landmark's line of the chart, walking on.
    input.key(Key::Forward, true);
    run(&mut client, &mut input, 1.0 / 60.0);
    input.key(Key::Forward, false);
    run(&mut client, &mut input, 1.0);
    let place = charted(&mut client, &mut input, home);
    assert!(
        place.contains('@'),
        "a landmark hung over the ground keeps its height: {place}"
    );
    client.command(client::Command::Travel { place });
    passed(&mut client, &mut input);
    assert_eq!(
        client.host("ride").way(),
        Some("horse"),
        "mounted by the chart"
    );
    let (herd_after, ridden) = herd(&mut client);
    assert_eq!(ridden, vec![horse.number]);
    // Each animal drawn once, none lost: the herd grazes on meanwhile, so
    // one may have come into view.
    let mut once = herd_after.clone();
    once.dedup();
    assert_eq!(once, herd_after, "an animal drawn twice");
    assert!(
        herd_before.iter().all(|number| herd_after.contains(number)),
        "the herd counts the same animals: {herd_before:?} then {herd_after:?}"
    );
}
