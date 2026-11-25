//! The moon has a gravity field of its own: fly in, land, walk, jump high.

use client::{Client, Command, Input, Key, Mode, Recipe};

fn altitude_after(client: &mut Client, input: &mut Input, seconds: f64) -> (f64, f64) {
    let (mut last, mut peak) = (0.0, f64::MIN);
    for _ in 0..(seconds * 60.0) as u32 {
        client.update(1.0 / 60.0, input);
        for event in client.drain_events() {
            if let client::Event::Stats { altitude_m, .. } = event {
                last = altitude_m;
                peak = peak.max(altitude_m);
            }
        }
    }
    (last, peak)
}

fn jump_height(client: &mut Client) -> f64 {
    let mut input = Input::default();
    altitude_after(client, &mut input, 1.0);
    input.key(Key::Up, true);
    altitude_after(client, &mut input, 0.1);
    input.key(Key::Up, false);
    altitude_after(client, &mut input, 8.0).1
}

#[test]
fn flight_off_near_the_moon_means_landing_on_it() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    let on_planet = jump_height(&mut client);

    // A hundred metres under the moon, then flight off: the moon is down.
    client.visit_moon(100.0, 0.0, 6.0);
    client.command(Command::SetMode { mode: Mode::Walk });
    let mut input = Input::default();
    let (landed, _) = altitude_after(&mut client, &mut input, 30.0);
    assert!(landed.abs() < 0.01, "standing on the moon at {landed}");

    // Same legs, a fifth of the gravity.
    let on_moon = jump_height(&mut client);
    assert!(
        on_moon > on_planet * 3.0,
        "jumped {on_moon} against {on_planet} at home"
    );

    // Walking follows the craters, and a minute later you are still on the
    // ground: the moon carries you along its orbit.
    input.key(Key::Forward, true);
    let (walking, _) = altitude_after(&mut client, &mut input, 60.0);
    assert!(
        (-0.01..1.0).contains(&walking),
        "walking on the moon at {walking}"
    );

    // Flight wins over gravity: from the moon, fly away and the planet's frame
    // takes you back, with no gravity pulling the flyer round on the way.
    input.key(Key::Forward, false);
    client.command(Command::SetMode { mode: Mode::Fly });
    input.key(Key::Up, true);
    input.key(Key::Sprint, true);
    let (leaving, _) = altitude_after(&mut client, &mut input, 30.0);
    assert!(leaving > 30_000.0, "flew {leaving} m away from the moon");
}

/// What a frame builds of each body: the planet's patches, then the moon's.
fn built(client: &mut Client) -> (usize, usize) {
    client
        .drain_terrain_changes()
        .iter()
        .fold((0, 0), |(planet, moon), change| {
            match change {
                // A patch names its body in its top bit.
                scene::TerrainChange::Add(id, _) if id.0 >> 63 == 1 => (planet, moon + 1),
                scene::TerrainChange::Add(..) => (planet + 1, moon),
                _ => (planet, moon),
            }
        })
}

/// Frames until each body has built anything, and the most one frame built,
/// over the first `frames` of a client posed by `pose`.
fn first_builds(pose: impl Fn(&mut Client), frames: usize) -> ([Option<usize>; 2], usize) {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    pose(&mut client);
    let mut input = Input::default();
    let (mut first, mut most) = ([None, None], 0);
    for frame in 0..frames {
        client.update(1.0 / 60.0, &mut input);
        let (planet, moon) = built(&mut client);
        most = most.max(planet + moon);
        for (first, count) in first.iter_mut().zip([planet, moon]) {
            if count > 0 {
                first.get_or_insert(frame);
            }
        }
    }
    (first, most)
}

#[test]
fn on_the_planet_the_moon_waits_for_the_ground() {
    let ([planet, moon], most) = first_builds(|client| client.pose(3000.0, -1.2, 6.0), 60);
    assert_eq!(planet, Some(0), "the ground comes first");
    assert!(
        moon.is_some_and(|moon| moon > 0),
        "the moon waits: {moon:?}"
    );
    assert!(
        most <= 6,
        "{most} patches in one frame, both bodies together"
    );
}

#[test]
fn on_the_moon_the_planet_waits_for_the_ground() {
    let ([planet, moon], most) = first_builds(|client| client.visit_moon(100.0, 0.0, 6.0), 120);
    assert_eq!(moon, Some(0), "the ground comes first");
    assert!(
        planet.is_some_and(|planet| planet > 0),
        "the planet waits: {planet:?}"
    );
    assert!(
        most <= 6,
        "{most} patches in one frame, both bodies together"
    );
}

#[test]
fn a_settled_frame_builds_both_bodies_at_once() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.pose(3000.0, -1.2, 6.0);
    client.settled_frame();
    let (planet, moon) = built(&mut client);
    assert!(
        planet > 6 && moon >= 6,
        "{planet} of the planet, {moon} of the moon"
    );
}
