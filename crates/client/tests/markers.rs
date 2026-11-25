//! Markers on a point of the world: found where they were given, over it
//! while it is in view and on the screen's edge while it is behind, told
//! only as they move, read by an agent with its place, and hidden by kind;
//! the home and favourites, given by the parts.

use client::{
    Client, Event, Host, Input, Level, Marked, Marker, Name, Part, PartPose, Plugin, Recipe, Shape,
    Turn,
};
use glam::{DQuat, DVec3};
use serde_json::{Value, json};

/// A plugin that marks a point `ahead_m` straight ahead of the eye at its
/// first turn, and keeps marking it there.
struct Pin {
    ahead_m: f64,
    at: Option<DVec3>,
}

impl Plugin for Pin {
    fn name(&self) -> &'static str {
        "pin"
    }

    fn version(&self) -> u32 {
        1
    }

    fn command(&mut self, kind: &str, _body: Value, host: &mut Host<'_>) {
        host.reject(format!("`{kind}` is no command of pin"));
    }

    fn turn(&mut self, turn: &Turn<'_>, host: &mut Host<'_>) {
        let eye = turn.eye;
        let at = *self
            .at
            .get_or_insert(eye.position + eye.rotation * DVec3::new(0.0, 0.0, -self.ahead_m));
        host.markers(vec![Marker {
            id: 1,
            on: Marked::Point(at),
            kind: "pin",
            thing: "pin",
            label: "Square".into(),
            glyph: "map-pin",
            note: None,
            choices: Vec::new(),
        }]);
    }
}

fn with(ahead_m: f64) -> Client {
    rehearsed(ahead_m, Level::Anonymous)
}

fn rehearsed(ahead_m: f64, level: Level) -> Client {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(Box::new(Pin { ahead_m, at: None }));
    client.go_to("4-77AEYRG").expect("a place to stand");
    client.rehearse(level);
    client
}

const DT: f64 = 1.0 / 60.0;

fn run(client: &mut Client, input: &mut Input, frames: usize) -> Vec<Event> {
    (0..frames)
        .flat_map(|_| {
            client.update(DT, input);
            client.drain_events()
        })
        .collect()
}

/// Where a marker lands: its point, its edge angle and how far.
type Landed = ([f32; 2], Option<f32>, f32);

/// Where the last `markers_at` put each marker.
fn last_at(events: &[Event]) -> Option<Vec<Landed>> {
    events.iter().rev().find_map(|event| match event {
        Event::MarkersAt { at } => Some(
            at.iter()
                .map(|at| ([at.x, at.y], at.edge_deg, at.distance_m))
                .collect(),
        ),
        _ => None,
    })
}

fn said(events: &[Event]) -> Option<Value> {
    events.iter().rev().find_map(|event| match event {
        Event::MarkersChanged { markers } => Some(serde_json::to_value(markers).unwrap()),
        _ => None,
    })
}

#[test]
fn a_point_ahead_lands_in_the_middle_and_behind_rides_the_edge() {
    let mut client = with(40.0);
    let mut input = Input::default();
    let events = run(&mut client, &mut input, 3);
    let markers = said(&events).expect("the pin is said");
    assert_eq!(markers[0]["giver"], "pin");
    assert_eq!(markers[0]["kind"], "pin");
    assert_eq!(markers[0]["on"]["point"].as_array().map(Vec::len), Some(3));

    let at = last_at(&events).expect("where it is");
    let ([x, y], edge, distance_m) = at[0];
    assert_eq!(edge, None, "in view");
    assert!((x - 0.5).abs() < 1e-3 && (y - 0.5).abs() < 1e-3, "{x} {y}");
    // Along the ground from the body, shorter than the eye's ray down to it.
    assert!((10.0..40.0).contains(&distance_m), "{distance_m}");

    assert_eq!(last_at(&run(&mut client, &mut input, 3)), None, "still");

    // The eye turns half round: the point is behind it, on the edge.
    input.look = [1800.0, 0.0];
    let events = run(&mut client, &mut input, 1);
    input.look = [0.0, 0.0];
    let events = [events, run(&mut client, &mut input, 3)].concat();
    let ([x, y], edge, _) = last_at(&events).expect("it moved")[0];
    assert!(edge.is_some(), "on the edge");
    assert!(x <= 0.0 || x >= 1.0 || y <= 0.0 || y >= 1.0, "{x} {y}");
}

#[test]
fn an_agent_reads_the_point_with_its_place_and_hides_its_kind() {
    let mut client = with(2000.0);
    let mut input = Input::default();
    run(&mut client, &mut input, 3);

    client.command_json(r#"{"type":"markers"}"#);
    let events = run(&mut client, &mut input, 1);
    let pointed = events
        .iter()
        .find_map(|event| match event {
            Event::Markers { markers } => Some(serde_json::to_value(markers).unwrap()),
            _ => None,
        })
        .expect("the reading");
    assert_eq!(pointed[0]["label"], "Square");
    assert!(
        pointed[0]["place"]
            .as_str()
            .is_some_and(|place| !place.is_empty())
    );
    let away_m = pointed[0]["away_m"].as_f64().unwrap();
    assert!((1000.0..2000.0).contains(&away_m), "{away_m}");

    client.command_json(r#"{"type":"hide_markers","kinds":["pin"]}"#);
    let events = run(&mut client, &mut input, 2);
    assert_eq!(said(&events), Some(json!([])), "hidden, unsaid");
    assert_eq!(
        last_at(&events),
        Some(Vec::new()),
        "and gone from the screen"
    );

    client.command_json(r#"{"type":"hide_markers","kinds":[]}"#);
    let events = run(&mut client, &mut input, 2);
    assert_eq!(
        said(&events).map(|said| said[0]["id"].clone()),
        Some(json!(1))
    );
}

/// A landmark called `name`, `ahead` blocks north of the feet.
fn star(client: &mut Client, name: &str, ahead: f64) -> (client::PartId, client::Seat, PartPose) {
    let feet = client.host("pin").feet();
    let seat = client::Seat::Sector(feet.point.sector);
    let pose = PartPose::upright(
        [
            feet.point.u,
            feet.point.v + ahead,
            feet.height_m / topology::BLOCK_M,
        ],
        [2.0, 2.0, 0.16],
    );
    let part = Part {
        shape: Shape::Landmark(Name::new(name).unwrap()),
        pose,
        paint: 0,
        solid: false,
        shine: 0.0,
        reach_m: 0.0,
        name: None,
        muted: false,
        volume: 0.0,
    };
    let id = client.host("pin").place_part(seat, part).unwrap();
    (id, seat, pose)
}

/// The home and the favourites are the parts' own markers, one a
/// landmark, the home's first and a favourite that is the home the
/// home's, each with its name and glyph, travelling there when pressed;
/// given again as the choices change, and hidden by kind apart.
#[test]
fn the_home_and_the_favourites_are_marked_and_travelled_to() {
    // A builder, to place the stars.
    let mut client = rehearsed(40.0, Level::Builder);
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let (home, seat, pose) = star(&mut client, "Praça", 4000.0);
    let (square, ..) = star(&mut client, "Sé", 9000.0);
    let markers_of = |events: &[Event]| {
        said(events).map(|said| {
            said.as_array()
                .unwrap()
                .iter()
                .filter(|marker| marker["giver"] == "parts")
                .cloned()
                .collect::<Vec<_>>()
        })
    };
    let events = run(&mut client, &mut input, 2);
    assert_eq!(markers_of(&events), None, "nothing chosen, nothing said");

    client.command_json(&format!(
        r#"{{"type":"set_choices","home":{},"favourites":[{},{}]}}"#,
        home.0, home.0, square.0
    ));
    let events = run(&mut client, &mut input, 2);
    let parts = markers_of(&events).expect("the choices are marked");
    let shown: Vec<(u64, &str, &str, &str)> = parts
        .iter()
        .map(|marker| {
            (
                marker["id"].as_u64().unwrap(),
                marker["kind"].as_str().unwrap(),
                marker["label"].as_str().unwrap(),
                marker["glyph"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            (home.0, "home", "Praça", "house"),
            (square.0, "favourite", "Sé", "star"),
        ]
    );
    let place = client.host("pin").place_at(seat, &pose);
    assert_eq!(
        parts[0]["choices"][0]["command"],
        json!({ "type": "travel", "place": place })
    );
    assert_eq!(parts[0]["choices"][0]["verb"], "travel");
    let at = last_at(&events).expect("where they are");
    let far = at
        .iter()
        .map(|(_, _, distance_m)| *distance_m)
        .fold(0.0, f32::max);
    assert!((4000.0..5000.0).contains(&far), "the Sé, 4.5 km: {far}");

    // Still, nothing is given again.
    assert_eq!(markers_of(&run(&mut client, &mut input, 2)), None);

    // Favourites hidden, the home stays.
    client.command_json(r#"{"type":"hide_markers","kinds":["favourite"]}"#);
    let events = run(&mut client, &mut input, 1);
    let kinds: Vec<Value> = markers_of(&events)
        .unwrap()
        .iter()
        .map(|marker| marker["kind"].clone())
        .collect();
    assert_eq!(kinds, [json!("home")]);

    // No home any more: its marker goes.
    client.command_json(&format!(
        r#"{{"type":"set_choices","home":null,"favourites":[{}]}}"#,
        square.0
    ));
    client.command_json(r#"{"type":"hide_markers","kinds":[]}"#);
    let events = run(&mut client, &mut input, 1);
    let ids: Vec<u64> = markers_of(&events)
        .unwrap()
        .iter()
        .map(|marker| marker["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, [square.0]);

    // Pressed, it travels there, as a line of the chart does.
    let travel = parts[1]["choices"][0]["command"].to_string();
    client.command_json(&travel);
    let events = run(&mut client, &mut input, 1);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. })),
        "{events:?}"
    );
}

/// One arriving at a star faces the way its arrow shows: its point laid
/// flat on the ground, turned any way and tilted too.
#[test]
fn one_arriving_at_a_star_faces_the_way_its_arrow_shows() {
    let mut client = rehearsed(40.0, Level::Builder);
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let (id, seat, upright) = star(&mut client, "Praça", 6.0);
    for (turn_deg, tilt_deg) in [
        (0.0, 0.0),
        (90.0, 0.0),
        (-135.0, 0.0),
        (60.0, 40.0),
        (200.0, -70.0),
    ] {
        let rotation = DQuat::from_rotation_z(f64::to_radians(turn_deg))
            * DQuat::from_rotation_x(f64::to_radians(tilt_deg));
        let pose = PartPose {
            rotation,
            ..upright
        };
        let mut part = *client.parts().get(id).unwrap().1;
        part.pose = pose;
        let mut host = client.host("pin");
        host.set_part(id, seat, part).unwrap();
        let facing = host.facing(id).expect("it faces somewhere");
        assert!(
            facing.along.dot(facing.up).abs() < 1e-9,
            "flat on the ground"
        );
        let point = host.part_transform(id).unwrap().y_axis;
        let flat = (point - facing.up * facing.up.dot(point)).normalize();
        assert!(flat.dot(facing.along) > 1.0 - 1e-9, "its point, laid flat");
        let place = host.place_at(seat, &pose);
        client.go_to(&place).unwrap();
        run(&mut client, &mut input, 1);
        let arrived = client.here().bearing_deg.unwrap();
        let shown = facing.bearing_deg().unwrap();
        // A place carries whole degrees.
        let off = (arrived - shown + 540.0).rem_euclid(360.0) - 180.0;
        assert!(
            off.abs() <= 0.5 + 1e-6,
            "turned {turn_deg}, tilted {tilt_deg}: arrived {arrived}, shown {shown}"
        );
    }
}
