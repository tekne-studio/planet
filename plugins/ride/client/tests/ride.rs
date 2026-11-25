//! Riding, as a front end, an agent and a shell's keys ask for it: the
//! plugin plugged into a client that stands in for a world.

use client::{Client, Event, Input, Key, Level, Mode, Recipe};

/// A client standing on dry land, with riding plugged in and on.
fn alone() -> Client {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(ride_client::plugin());
    client.go_to("4-77AEYRG").expect("a place to stand");
    client.rehearse(Level::Anonymous);
    client.update(1.0 / 60.0, &mut Input::default());
    client.drain_events();
    client
}

fn modes(client: &mut Client) -> Vec<Mode> {
    client
        .drain_events()
        .into_iter()
        .filter_map(|event| match event {
            Event::ModeChanged { mode } => Some(mode),
            _ => None,
        })
        .collect()
}

fn run(client: &mut Client, input: &mut Input, frames: u32) -> client::Frame {
    let mut frame = client.update(1.0 / 60.0, input);
    for _ in 1..frames {
        frame = client.update(1.0 / 60.0, input);
    }
    frame
}

#[test]
fn an_agent_mounts_rides_and_leaves_a_board() {
    let mut client = alone();
    let standing = run(&mut client, &mut Input::default(), 1).boxes.len();
    client.command_json(r#"{"type":"ride.mount"}"#);
    assert_eq!(modes(&mut client), vec![Mode::Way("ride.board".into())]);

    // Ridden ahead, it gathers speed and the plank is drawn under it, its
    // flame behind.
    let mut input = Input::default();
    input.key(Key::Forward, true);
    let riding = run(&mut client, &mut input, 120);
    let on = client.host("ride").in_ways();
    assert_eq!(on.len(), 1);
    assert_eq!((on[0].session, on[0].way), (None, ride_client::BOARD));
    assert!(on[0].speed_mps > 5.0, "{}", on[0].speed_mps);
    assert_eq!(riding.boxes.len(), standing + 8);

    client.command_json(r#"{"type":"ride.leave"}"#);
    assert_eq!(modes(&mut client), vec![Mode::Walk]);
    assert!(client.host("ride").in_ways().is_empty());
    // The board shrinks away where it was left, its flame out, then is gone.
    let stepped_off = run(&mut client, &mut Input::default(), 1);
    assert_eq!(stepped_off.boxes.len(), standing + 6);
    let walking = run(&mut client, &mut Input::default(), 30);
    assert_eq!(walking.boxes.len(), standing);
}

#[test]
fn the_key_mounts_and_leaves_and_the_flight_key_leaves_first() {
    let mut client = alone();
    let mut input = Input::default();
    let press = |client: &mut Client, input: &mut Input, code: &str| {
        input.code(code, client::Chord::default(), true);
        run(client, input, 1);
        input.code(code, client::Chord::default(), false);
        run(client, input, 1);
    };
    press(&mut client, &mut input, "KeyH");
    assert_eq!(modes(&mut client), vec![Mode::Way("ride.board".into())]);
    press(&mut client, &mut input, "KeyH");
    assert_eq!(modes(&mut client), vec![Mode::Walk]);

    // The core's flight key, on a board, steps off it first.
    press(&mut client, &mut input, "KeyH");
    input.key(Key::ToggleMode, true);
    run(&mut client, &mut input, 1);
    input.key(Key::ToggleMode, false);
    assert_eq!(
        modes(&mut client),
        vec![Mode::Way("ride.board".into()), Mode::Walk]
    );
}

#[test]
fn a_board_is_a_mode_the_core_takes_and_leaves_with_its_plugin() {
    let mut client = alone();
    client.command_json(r#"{"type":"set_mode","mode":"ride.board"}"#);
    assert_eq!(modes(&mut client), vec![Mode::Way("ride.board".into())]);
    client.command_json(r#"{"type":"set_mode","mode":"ride.skis"}"#);
    let refused = client.drain_events();
    assert!(
        matches!(refused.as_slice(), [Event::Rejected { .. }]),
        "{refused:?}"
    );

    // A world that turns riding off takes the body off the board.
    client.link_opened();
    client.receive(&protocol::encode(&protocol::ServerMessage {
        message: Some(protocol::server_message::Message::Plugins(
            protocol::Plugins {
                plugins: Vec::new(),
            },
        )),
    }));
    assert_eq!(modes(&mut client), vec![Mode::Walk]);
}

/// What the prompt said, as the thing and the tap, in order.
fn prompted(client: &mut Client) -> Vec<Option<(&'static str, &'static str)>> {
    client
        .drain_events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Prompt { prompt } => Some(prompt.map(|p| (p.thing, p.tap.unwrap_or_default()))),
            _ => None,
        })
        .collect()
}

#[test]
fn riding_the_prompt_offers_to_leave_and_on_foot_nothing() {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(ride_client::plugin());
    client.go_to("4-77AEYRG").expect("a place to stand");
    client.rehearse(Level::Anonymous);
    let mut input = Input::default();
    // On foot E offers no board: mounting is H's.
    input.key(Key::Act, true);
    run(&mut client, &mut input, 2);
    input.key(Key::Act, false);
    assert_eq!(prompted(&mut client), vec![]);

    input.code("KeyH", client::Chord::default(), true);
    run(&mut client, &mut input, 1);
    input.code("KeyH", client::Chord::default(), false);
    assert_eq!(prompted(&mut client), vec![Some(("board", "leave"))]);

    // E leaves, and the prompt goes with the board.
    input.key(Key::Act, true);
    run(&mut client, &mut input, 1);
    let events = client.drain_events();
    assert!(
        events.contains(&Event::ModeChanged { mode: Mode::Walk }),
        "{events:?}"
    );
    assert!(
        events.contains(&Event::Prompt { prompt: None }),
        "{events:?}"
    );

    // An agent acts on the same prompt, or names it.
    client.command_json(r#"{"type":"ride.mount"}"#);
    run(&mut client, &mut input, 1);
    client.command_json(r#"{"type":"act"}"#);
    assert_eq!(
        modes(&mut client),
        vec![Mode::Way("ride.board".into()), Mode::Walk]
    );
    client.command_json(r#"{"type":"ride.mount"}"#);
    run(&mut client, &mut input, 1);
    client.command_json(r#"{"type":"act","plugin":"ride","id":0}"#);
    assert_eq!(
        modes(&mut client),
        vec![Mode::Way("ride.board".into()), Mode::Walk]
    );
}

/// A horse is mounted through its offer, held, as a person holds the key
/// before it; an agent names the offer by the horse's number. Neither
/// `ride.mount` nor a mode names a horse, so none is ridden bare. Leaving,
/// the body steps down beside it.
#[test]
fn a_horse_is_mounted_through_its_offer_held_and_left_beside_it() {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(ride_client::plugin());
    client.rehearse(Level::Anonymous);
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    client.drain_events();
    client.command_json(r#"{"type":"ride.mount","way":"horse"}"#);
    let refused = client.drain_events();
    assert!(
        refused
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. })),
        "{refused:?}"
    );
    client.command_json(r#"{"type":"set_mode","mode":"ride.horse"}"#);
    let refused = client.drain_events();
    assert!(
        refused
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. })),
        "{refused:?}"
    );
    assert!(client.host("ride").in_ways().is_empty());

    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    let grid = client.generator().sphere().blocks();
    client.teleport(grid.surface_point(horse.at.to_array()));
    run(&mut client, &mut input, 1);
    client.drain_events();
    let number = horse.number;
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{number},"hold":true}}"#
    ));
    run(&mut client, &mut input, 60);
    assert_eq!(modes(&mut client), vec![Mode::Way("ride.horse".into())]);
    let on = client.host("ride").in_ways();
    assert_eq!((on[0].way, on[0].carrier), (ride_client::HORSE, number));

    let saddled = client.here().column;
    client.command_json(r#"{"type":"ride.leave"}"#);
    run(&mut client, &mut input, 1);
    assert_eq!(modes(&mut client), vec![Mode::Walk]);
    assert!(client.host("ride").in_ways().is_empty());
    assert_ne!(client.here().column, saddled, "stepped down beside it");
}

/// A body flying low over a horse is offered it as one on foot is, and
/// comes down into the saddle.
#[test]
fn a_horse_is_mounted_from_flight() {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(ride_client::plugin());
    client.rehearse(Level::Anonymous);
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    let grid = client.generator().sphere().blocks();
    client.teleport(grid.surface_point(horse.at.to_array()));
    run(&mut client, &mut input, 1);
    client.command_json(r#"{"type":"set_mode","mode":"fly"}"#);
    run(&mut client, &mut input, 1);
    input.key(Key::Up, true);
    run(&mut client, &mut input, 1);
    input.key(Key::Up, false);
    run(&mut client, &mut input, 1);
    assert!(!client.host("ride").grounded(), "off the ground");
    client.drain_events();
    let number = horse.number;
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{number},"hold":true}}"#
    ));
    run(&mut client, &mut input, 60);
    assert_eq!(modes(&mut client), vec![Mode::Way("ride.horse".into())]);
    assert_eq!(client.host("ride").in_ways()[0].carrier, number);
}

/// A frame from a world that says this envelope of life.
fn life_says(kind: &str, payload: Vec<u8>) -> Vec<u8> {
    use protocol::{ServerMessage, server_message::Message};
    protocol::encode(&ServerMessage {
        message: Some(Message::Envelope(protocol::Envelope {
            plugin: protocol::life::OWNER.into(),
            kind: kind.into(),
            payload,
            id: 0,
        })),
    })
}

/// What holding the key on the body's card does, as an agent reads it.
fn held_verb(client: &mut Client) -> Option<String> {
    client.command_json(r#"{"type":"offers"}"#);
    let events = client.drain_events();
    let offers = events.iter().find_map(|event| match event {
        Event::Offers { offers } => Some(serde_json::to_value(offers).unwrap()),
        _ => None,
    })?;
    let card = offers
        .as_array()?
        .iter()
        .find(|offer| offer["on"] == "body")?;
    card["hold"]["verb"].as_str().map(str::to_owned)
}

/// A visitor's client a world with life and riding on has welcomed.
fn welcomed() -> Client {
    use protocol::{Message as _, life as wire, server_message::Message};
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(ride_client::plugin());
    client.link_opened();
    let recipe = client.recipe();
    client.receive(&protocol::encode(&protocol::ServerMessage {
        message: Some(Message::Welcome(protocol::Welcome {
            session: 1,
            recipe: Some(protocol::Recipe {
                seed: format!("{:016x}", recipe.seed),
                generator_version: recipe.generator_version,
                params_json: serde_json::to_string(&recipe.params).unwrap(),
            }),
            level: protocol::Level::Anonymous.into(),
            plugins: vec![
                protocol::Plugin {
                    name: wire::OWNER.into(),
                    version: gaia::VERSION,
                },
                protocol::Plugin {
                    name: ride_world::NAME.into(),
                    version: ride_world::VERSION,
                },
            ],
            visitor: true,
            ..Default::default()
        })),
    }));
    // Nothing is built near: the world says so, and the herds come.
    let seen = protocol::cells::Seen {
        complete: true,
        ..Default::default()
    };
    client.receive(&protocol::encode(&protocol::ServerMessage {
        message: Some(Message::Envelope(protocol::Envelope {
            plugin: protocol::cells::OWNER.into(),
            kind: protocol::cells::SEEN.into(),
            payload: seen.encode_to_vec(),
            id: 0,
        })),
    }));
    client
}

/// A pet this client keeps, the animal of `number`, standing at `at`.
fn pet_told(client: &mut Client, number: u64, name: &str, at: glam::DVec3) {
    use protocol::{Message as _, life as wire};
    let grid = client.generator().sphere().blocks();
    let point = grid.surface_point(at.to_array());
    let pet = wire::Pet {
        number,
        name: name.into(),
        spot: Some(wire::Spot {
            sector: point.sector.index() as u32,
            u: point.u,
            v: point.v,
            heading: 0.0,
            at_ms: 0,
        }),
        mine: true,
        session: 1,
        ..Default::default()
    };
    let told = wire::Told { pet: Some(pet) }.encode_to_vec();
    client.receive(&life_says(wire::PET, told));
}

#[test]
fn a_pet_answers_a_whistle_within_its_reach_and_no_farther() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    let up = horse.at.normalize();
    let along = up.cross(glam::DVec3::Y).normalize_or(glam::DVec3::X);
    let radius_m = client.generator().sphere().radius_m();
    // The body `away_m` along the ground from the pet, whistling: what the
    // whistle says, and whether a call went to the world.
    let mut whistle = |away_m: f64| {
        pet_told(&mut client, horse.number, "Bay", horse.at);
        let there = glam::DQuat::from_axis_angle(up.cross(along), away_m / radius_m) * horse.at;
        let grid = client.generator().sphere().blocks();
        client.teleport(grid.surface_point(there.to_array()));
        run(&mut client, &mut input, 2);
        client.drain_events();
        client.command_json(r#"{"type":"sound","voice":"whistle"}"#);
        run(&mut client, &mut input, 1);
        client
            .drain_events()
            .into_iter()
            .find_map(|event| match event {
                Event::Whistled {
                    away_m, answered, ..
                } => Some((away_m, answered)),
                _ => None,
            })
            .expect("the whistle says whether it answers")
    };
    let reach = gaia::fauna::ANSWERS_M;
    let (away_m, answered) = whistle(reach - 30.0);
    assert!(answered, "within reach, {away_m} m off, it comes");
    let (away_m, answered) = whistle(reach + 30.0);
    assert!(!answered, "past it, {away_m} m off, it does not");
}

/// How many calls of the pet went to the world since the last drain.
fn calls_asked(client: &mut Client) -> usize {
    use protocol::{Message as _, life as wire};
    let asked = client.drain_outbound().into_iter().filter(|out| {
        let client::Outbound::Frame(frame) = out else {
            return false;
        };
        let message = protocol::ClientMessage::decode(&frame[..]).unwrap().message;
        matches!(message,
            Some(protocol::client_message::Message::Envelope(envelope))
                if envelope.kind == wire::CALL)
    });
    asked.count()
}

/// A client whose pet, the horse nearest the spawn, stands `away_m` along
/// the ground from the body, just whistled for, and the calls that went:
/// the client, its input, the pet's number and where the body whistled.
fn whistled_from(away_m: f64) -> (Client, Input, u64, glam::DVec3) {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    pet_told(&mut client, horse.number, "Bay", horse.at);
    let up = horse.at.normalize();
    let along = up.cross(glam::DVec3::Y).normalize_or(glam::DVec3::X);
    let radius_m = client.generator().sphere().radius_m();
    let there = glam::DQuat::from_axis_angle(up.cross(along), away_m / radius_m) * horse.at;
    let grid = client.generator().sphere().blocks();
    client.teleport(grid.surface_point(there.to_array()));
    run(&mut client, &mut input, 2);
    client.drain_outbound();
    client.command_json(r#"{"type":"sound","voice":"whistle"}"#);
    run(&mut client, &mut input, 1);
    assert_eq!(calls_asked(&mut client), 1, "the whistle calls it");
    let body = client.host("ride").body().position;
    (client, input, horse.number, body)
}

/// `seconds` of frames, the world heard from every second, as a world
/// says the stances near: a world silent for long is a link lost.
fn run_heard(client: &mut Client, input: &mut Input, seconds: u32) {
    for _ in 0..seconds {
        client.receive(&protocol::encode(&protocol::ServerMessage {
            message: None,
        }));
        run(client, input, 60);
    }
}

/// `seconds` of frames, the world heard from every second, and the
/// largest change of the speed the pet of `number` is drawn at from one
/// frame to the next, metres a second.
fn speed_steps(client: &mut Client, input: &mut Input, number: u64, seconds: u32) -> f64 {
    let dt = 1.0 / 60.0;
    let drawn = |client: &mut Client| {
        let ride = client.host("ride");
        let pet = ride.animals().iter().find(|near| near.number == number);
        pet.expect("the pet is drawn").at
    };
    let (mut was, mut speed, mut most) = (drawn(client), None::<f64>, 0.0_f64);
    for frame in 0..seconds * 60 {
        if frame % 60 == 0 {
            client.receive(&protocol::encode(&protocol::ServerMessage {
                message: None,
            }));
        }
        client.update(dt, input);
        let at = drawn(client);
        // Along the ground: what the ground's rise adds is no speed.
        let up = at.normalize();
        let moved = at - was;
        let now = (moved - up * moved.dot(up)).length() / dt;
        if let Some(speed) = speed {
            most = most.max((now - speed).abs());
        }
        (was, speed) = (at, Some(now));
    }
    most
}

/// Where the pet of `number` is drawn, metres from the body.
fn pet_from_body_m(client: &mut Client, number: u64) -> f64 {
    let ride = client.host("ride");
    let pet = ride.animals().iter().find(|near| near.number == number);
    (pet.expect("the pet is drawn").at - ride.body().position).length()
}

/// A keeper standing where it whistled, swaying a step to each side, calls
/// its pet once: it runs in and stands beside the body, called no more.
#[test]
fn a_pet_whistled_for_by_a_keeper_standing_is_called_once() {
    let (mut client, mut input, number, whistled) = whistled_from(40.0);
    let mut calls = 0;
    for second in 0..10 {
        let side = if second % 2 == 0 {
            Key::Right
        } else {
            Key::Left
        };
        input.key(side, true);
        run(&mut client, &mut input, 5);
        input.key(side, false);
        run_heard(&mut client, &mut input, 1);
        let swayed_m = (client.host("ride").body().position - whistled).length();
        assert!(swayed_m < 1.0, "a sway, not a step: {swayed_m} m");
        calls += calls_asked(&mut client);
    }
    assert_eq!(calls, 0, "a body swaying in place calls nothing more");
    let beside_m = pet_from_body_m(&mut client, number);
    assert!(beside_m < 4.0, "it stands beside the body: {beside_m} m");
}

/// A keeper walking on after its whistle is followed: the pet is called
/// again as the body goes, its speed never stepping, stands beside it
/// where it stops, and once there is called no more.
#[test]
fn a_pet_whistled_for_follows_its_keeper_walking_on() {
    let (mut client, mut input, number, whistled) = whistled_from(40.0);
    input.key(Key::Forward, true);
    let walking = speed_steps(&mut client, &mut input, number, 4);
    input.key(Key::Forward, false);
    let stopping = speed_steps(&mut client, &mut input, number, 3);
    // Called again on its way, it goes on at its speed: no frame steps it
    // as a run set off from a standstill does (3 m/s a frame and more).
    for (steps, while_) in [(walking, "walking"), (stopping, "stopping")] {
        assert!(
            steps < 0.5,
            "its speed steps {steps} m/s in a frame, {while_}"
        );
    }
    let walked_m = (client.host("ride").body().position - whistled).length();
    assert!(walked_m > 8.0, "the body walked on: {walked_m} m");
    let mut calls = calls_asked(&mut client);
    run_heard(&mut client, &mut input, 10);
    calls += calls_asked(&mut client);
    assert!(calls >= 2, "called again as the body went: {calls}");
    let beside_m = pet_from_body_m(&mut client, number);
    assert!(
        beside_m < 4.0,
        "it stands beside where the body stopped: {beside_m} m"
    );
    run_heard(&mut client, &mut input, 3);
    assert_eq!(
        calls_asked(&mut client),
        0,
        "beside the body, it is called no more"
    );
}

/// A keeper gone past earshot, as through a portal or on a board, is
/// followed no more: the pet runs on to where it was called and waits
/// there.
#[test]
fn a_pet_whistled_for_waits_where_it_was_called_once_its_keeper_is_past_earshot() {
    let (mut client, mut input, number, whistled) = whistled_from(40.0);
    let grid = client.generator().sphere().blocks();
    let radius_m = client.generator().sphere().radius_m();
    let up = whistled.normalize();
    let axis = up.cross(glam::DVec3::Y).normalize_or(glam::DVec3::X);
    let far_m = gaia::fauna::HEARS_M + 100.0;
    let gone = glam::DQuat::from_axis_angle(axis, far_m / radius_m) * whistled;
    client.teleport(grid.surface_point(gone.to_array()));
    run_heard(&mut client, &mut input, 10);
    assert_eq!(
        calls_asked(&mut client),
        0,
        "past earshot, it is called no more"
    );
    client.teleport(grid.surface_point(whistled.to_array()));
    run(&mut client, &mut input, 1);
    let waits_m = pet_from_body_m(&mut client, number);
    assert!(waits_m < 4.0, "it waits where it was called: {waits_m} m");
}

#[test]
fn ones_own_pet_is_released_from_the_saddle_standing_and_never_aloft() {
    use protocol::{Message as _, life as wire};
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);

    // The world says the horse nearest the spawn is this client's pet.
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    let grid = client.generator().sphere().blocks();
    let point = grid.surface_point(horse.at.to_array());
    pet_told(&mut client, horse.number, "Bay", horse.at);
    client.teleport(point);
    run(&mut client, &mut input, 2);
    client.drain_events();
    let number = horse.number;
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{number},"hold":true}}"#
    ));
    run(&mut client, &mut input, 30);
    assert_eq!(modes(&mut client), vec![Mode::Way("ride.horse".into())]);
    assert_eq!(held_verb(&mut client).as_deref(), Some("release"));

    // Aloft in a jump from a trot, the card holds nothing and the hold
    // is refused.
    for _ in 0..2 {
        input.key(Key::Forward, true);
        run(&mut client, &mut input, 1);
        input.key(Key::Forward, false);
        run(&mut client, &mut input, 1);
    }
    run(&mut client, &mut input, 90);
    // Jump tapped: it gathers, and leaves the ground let go.
    input.key(Key::Up, true);
    run(&mut client, &mut input, 1);
    input.key(Key::Up, false);
    let lifted = (0..120).any(|_| {
        run(&mut client, &mut input, 1);
        !client.host("ride").grounded()
    });
    assert!(lifted, "the jump lifts it");
    assert_eq!(held_verb(&mut client), None);
    client.drain_outbound();
    client.command_json(r#"{"type":"act","plugin":"ride","id":0,"hold":true}"#);
    assert!(
        client
            .drain_events()
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. }))
    );

    // Down again and halted, it is let go once the ring closes.
    run(&mut client, &mut input, 120);
    for _ in 0..2 {
        input.key(Key::Back, true);
        run(&mut client, &mut input, 1);
        input.key(Key::Back, false);
        run(&mut client, &mut input, 1);
    }
    run(&mut client, &mut input, 60);
    client.drain_outbound();
    assert_eq!(held_verb(&mut client).as_deref(), Some("release"));
    client.command_json(r#"{"type":"act","plugin":"ride","id":0,"hold":true}"#);
    run(&mut client, &mut input, 120);
    let asked = client.drain_outbound().into_iter().any(|out| {
        let client::Outbound::Frame(frame) = out else {
            return false;
        };
        let message = protocol::ClientMessage::decode(&frame[..]).unwrap().message;
        matches!(message,
            Some(protocol::client_message::Message::Envelope(envelope))
                if envelope.kind == wire::RELEASE)
    });
    assert!(asked, "release is asked of the world");
}

/// The body's card, as an agent reads it: what a tap does, what a hold.
fn card(client: &mut Client) -> (Option<String>, Option<String>) {
    client.command_json(r#"{"type":"offers"}"#);
    let offers = client
        .drain_events()
        .into_iter()
        .find_map(|event| match event {
            Event::Offers { offers } => Some(serde_json::to_value(offers).unwrap()),
            _ => None,
        })
        .unwrap_or_default();
    let card = offers
        .as_array()
        .and_then(|offers| offers.iter().find(|offer| offer["on"] == "body"))
        .cloned()
        .unwrap_or_default();
    let word = |value: &serde_json::Value| value.as_str().map(str::to_owned);
    (word(&card["tap"]), word(&card["hold"]["verb"]))
}

/// The body mounts the horse of `number` where it stands, through its offer.
fn mount(client: &mut Client, input: &mut Input, number: u64) -> bool {
    let Some(horse) = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| near.number == number)
    else {
        return false;
    };
    let point = client
        .generator()
        .sphere()
        .blocks()
        .surface_point(horse.at.to_array());
    client.teleport(point);
    run(client, input, 2);
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{number},"hold":true}}"#
    ));
    run(client, input, 30);
    client.drain_events();
    client.host("ride").way() == Some("horse")
}

fn fly_key(client: &mut Client, input: &mut Input) {
    input.key(Key::ToggleMode, true);
    run(client, input, 1);
    input.key(Key::ToggleMode, false);
    run(client, input, 1);
}

/// Each horse near the spawn in turn is told this client's pet and
/// mounted, until the flight key lifts one: a pet pegasus, aloft. One
/// without wings keeps its rider, who says it does not fly and leaves it
/// by its card. On the ground each card holds Release.
fn aloft_on_a_pet_pegasus(client: &mut Client, input: &mut Input) {
    let horses: Vec<(u64, glam::DVec3)> = client
        .host("ride")
        .animals()
        .iter()
        .filter(|near| !near.ridden)
        .map(|near| (near.number, near.at))
        .collect();
    let flown = horses.into_iter().any(|(number, at)| {
        pet_told(client, number, "Snow", at);
        run(client, input, 2);
        if !mount(client, input, number) {
            return false;
        }
        assert_eq!(card(client), (Some("leave".into()), Some("release".into())));
        client.drain_events();
        fly_key(client, input);
        run(client, input, 60);
        if client.host("ride").aloft() {
            return true;
        }
        let said = client.drain_events();
        assert!(
            said.iter()
                .any(|event| matches!(event, Event::Rejected { .. })),
            "a horse without wings says so: {said:?}"
        );
        assert_eq!(
            client.host("ride").way(),
            Some("horse"),
            "and keeps its rider"
        );
        client.command_json(r#"{"type":"ride.leave"}"#);
        run(client, input, 2);
        false
    });
    assert!(flown, "the flight key lifts a pet pegasus");
}

#[test]
fn a_pet_pegasus_flies_as_a_wild_one_and_aloft_its_card_lands_it() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    aloft_on_a_pet_pegasus(&mut client, &mut input);

    // Aloft, its card lands it, and nothing lets it go.
    assert_eq!(card(&mut client), (Some("land".into()), None));
    client.command_json(r#"{"type":"act","plugin":"ride","id":0}"#);
    run(&mut client, &mut input, 300);
    assert!(!client.host("ride").aloft());
    assert_eq!(client.host("ride").way(), Some("horse"));
    assert_eq!(
        card(&mut client),
        (Some("leave".into()), Some("release".into()))
    );
}

/// What the last stats said of the whistle: whether it calls someone.
fn calls(client: &mut Client, input: &mut Input) -> bool {
    run(client, input, 40);
    client
        .drain_events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Stats { calls, .. } => Some(calls),
            _ => None,
        })
        .next_back()
        .expect("the stats, twice a second")
}

/// The whistle calls someone while the body has a pet and is off it.
#[test]
fn the_stats_say_the_whistle_calls_only_a_pet_one_is_off() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    assert!(!calls(&mut client, &mut input), "no pet");
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    pet_told(&mut client, horse.number, "Bay", horse.at);
    assert!(calls(&mut client, &mut input), "a pet, on foot");
    assert!(mount(&mut client, &mut input, horse.number));
    assert!(!calls(&mut client, &mut input), "on its back");
}

/// `ride.mount`, the board's button, is the key's: aloft on a horse it is
/// refused, and the horse keeps its rider.
#[test]
fn the_board_is_refused_aloft_by_command_as_by_its_key() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    aloft_on_a_pet_pegasus(&mut client, &mut input);
    let flying = |client: &mut Client, input: &mut Input| {
        run(client, input, 40);
        client
            .drain_events()
            .into_iter()
            .rev()
            .find_map(|event| match event {
                Event::Stats { flying, .. } => Some(flying),
                _ => None,
            })
    };
    assert_eq!(
        flying(&mut client, &mut input),
        Some(true),
        "the stats say it flies"
    );
    client.command_json(r#"{"type":"ride.mount"}"#);
    run(&mut client, &mut input, 2);
    let why = client
        .drain_events()
        .into_iter()
        .find_map(|event| match event {
            Event::Rejected { message } => Some(message),
            _ => None,
        });
    assert!(
        why.is_some_and(|why| why.contains("lands")),
        "aloft, it says why"
    );
    assert_eq!(client.host("ride").way(), Some("horse"));
}

/// Whether a release went to the world since the last drain.
fn release_asked(client: &mut Client) -> bool {
    use protocol::{Message as _, life as wire};
    client.drain_outbound().into_iter().any(|out| {
        let client::Outbound::Frame(frame) = out else {
            return false;
        };
        let message = protocol::ClientMessage::decode(&frame[..]).unwrap().message;
        matches!(message,
            Some(protocol::client_message::Message::Envelope(envelope))
                if envelope.kind == wire::RELEASE)
    })
}

/// `ride.release`, what the marker's Let go sends, lets the pet go from
/// anywhere but from its back aloft: there it is refused and nothing goes
/// to the world; off it and flying, the same command lets it go.
#[test]
fn letting_go_by_command_is_refused_only_aloft_on_the_pet() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    aloft_on_a_pet_pegasus(&mut client, &mut input);
    client.drain_outbound();
    client.drain_events();
    client.command_json(r#"{"type":"ride.release"}"#);
    let refused = client
        .drain_events()
        .iter()
        .any(|event| matches!(event, Event::Rejected { .. }));
    assert!(refused, "aloft on it");
    assert!(!release_asked(&mut client), "nothing goes to the world");

    client.command_json(r#"{"type":"act","plugin":"ride","id":0}"#);
    run(&mut client, &mut input, 300);
    client.command_json(r#"{"type":"ride.leave"}"#);
    run(&mut client, &mut input, 30);
    client.command_json(r#"{"type":"set_mode","mode":"fly"}"#);
    input.key(Key::Up, true);
    run(&mut client, &mut input, 60);
    input.key(Key::Up, false);
    assert!(!client.host("ride").grounded(), "flying on foot");
    client.drain_outbound();
    client.command_json(r#"{"type":"ride.release"}"#);
    assert!(release_asked(&mut client), "aloft off it, it is let go");
}

/// A wild horse is tamed standing: in a jump the card holds nothing, and
/// the hold asked anyway is refused.
#[test]
fn a_wild_horse_is_tamed_only_standing() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let number = client
        .host("ride")
        .animals()
        .iter()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn")
        .number;
    assert!(mount(&mut client, &mut input, number));
    assert_eq!(held_verb(&mut client).as_deref(), Some("tame"));
    for _ in 0..2 {
        input.key(Key::Forward, true);
        run(&mut client, &mut input, 1);
        input.key(Key::Forward, false);
        run(&mut client, &mut input, 1);
    }
    run(&mut client, &mut input, 90);
    // Jump tapped: it gathers, and leaves the ground let go.
    input.key(Key::Up, true);
    run(&mut client, &mut input, 1);
    input.key(Key::Up, false);
    let lifted = (0..120).any(|_| {
        run(&mut client, &mut input, 1);
        !client.host("ride").grounded()
    });
    assert!(lifted, "the jump lifts it");
    assert_eq!(held_verb(&mut client), None);
    client.command_json(r#"{"type":"act","plugin":"ride","id":0,"hold":true}"#);
    assert!(
        client
            .drain_events()
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. }))
    );
}

/// The gaits of the stances this client sent since the last look.
fn gaits(client: &mut Client) -> Vec<protocol::Gait> {
    use protocol::Message as _;
    client
        .drain_outbound()
        .into_iter()
        .filter_map(|out| match out {
            client::Outbound::Frame(frame) => {
                protocol::ClientMessage::decode(&frame[..]).ok()?.message
            }
            _ => None,
        })
        .filter_map(|message| match message {
            protocol::client_message::Message::Stance(stance) => Some(stance.gait()),
            _ => None,
        })
        .collect()
}

/// Peers draw a mount's wings from the stance's gait, so it says what the
/// body's own client draws: aloft it flies whatever clip plays, and on the
/// ground a clip of flight does not make it fly.
#[test]
fn a_mount_aloft_says_it_flies_whatever_clip_plays() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    aloft_on_a_pet_pegasus(&mut client, &mut input);
    gaits(&mut client);
    client.command_json(r#"{"type":"play","clip":"fall","for_s":2}"#);
    run(&mut client, &mut input, 60);
    let aloft = gaits(&mut client);
    assert!(!aloft.is_empty());
    assert!(
        aloft.iter().all(|gait| *gait == protocol::Gait::Fly),
        "{aloft:?}"
    );

    client.command_json(r#"{"type":"act","plugin":"ride","id":0}"#);
    run(&mut client, &mut input, 300);
    assert!(!client.host("ride").aloft());
    gaits(&mut client);
    client.command_json(r#"{"type":"play","clip":"fly","for_s":2}"#);
    run(&mut client, &mut input, 60);
    let grounded = gaits(&mut client);
    assert!(!grounded.is_empty());
    assert!(!grounded.contains(&protocol::Gait::Fly), "{grounded:?}");
}

#[test]
fn an_agent_reads_the_animals_near_and_whose_each_is() {
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    pet_told(&mut client, horse.number, "Bay", horse.at);
    let grid = client.generator().sphere().blocks();
    client.teleport(grid.surface_point(horse.at.to_array()));
    run(&mut client, &mut input, 2);
    client.drain_events();

    let read = |client: &mut Client| {
        client.command_json(r#"{"type":"animals"}"#);
        client
            .drain_events()
            .into_iter()
            .find_map(|event| match event {
                Event::Animals { animals } => Some(serde_json::to_value(animals).unwrap()),
                _ => None,
            })
            .expect("an answer")
    };
    let animals = read(&mut client);
    let animals = animals.as_array().unwrap();
    assert!(animals.len() > 1, "a herd: {animals:?}");
    let away: Vec<f64> = animals
        .iter()
        .map(|a| a["away_m"].as_f64().unwrap())
        .collect();
    assert!(
        away.windows(2).all(|pair| pair[0] <= pair[1]),
        "nearest first"
    );
    let bay = &animals[0];
    assert_eq!(bay["number"], horse.number);
    assert_eq!(bay["kind"], "horse");
    assert_eq!(
        bay["pet"],
        serde_json::json!({ "name": "Bay", "mine": true })
    );
    assert!(bay["bearing_deg"].is_number() && bay["ridden"] == false);
    assert!(
        animals[1..].iter().all(|animal| animal["pet"].is_null()),
        "the rest are wild"
    );

    // Its place is the column it stands on, one `go_to` takes.
    let place = bay["place"].as_str().unwrap().to_owned();
    let under = grid.column_of(grid.surface_point(horse.at.to_array()));
    client.go_to(&place).expect("a place");
    assert_eq!(client.here().column, under);
}

/// A pegasus flown forward stays in the view: at sprint and not, looking
/// down, level and up, low, and climbing with sprint held until it flies
/// at its top speed and against its ceiling. Each frame most of what is
/// drawn near the camera is before it, the horse and its rider, and the
/// camera is within its boom of the body; drawn on the ground under its
/// flight, they were gone, and the eye sank after the saddle.
#[test]
fn a_pegasus_flown_forward_stays_in_the_view() {
    let cases = [
        (false, 0.0, 300),
        (true, 0.0, 300),
        (true, 150.0, 300),
        (true, -150.0, 2400),
        (true, -400.0, 1800),
    ];
    for (sprint, look_px, frames) in cases {
        let mut client = welcomed();
        let mut input = Input::default();
        run(&mut client, &mut input, 1);
        aloft_on_a_pet_pegasus(&mut client, &mut input);
        input.look = [0.0, look_px];
        run(&mut client, &mut input, 1);
        input.key(Key::Forward, true);
        input.key(Key::Sprint, sprint);
        let mut top_mps = 0.0f64;
        for frame in 0..frames {
            // The world this client was welcomed by says it is there.
            if frame % 60 == 0 {
                client.receive(&protocol::encode(&protocol::ServerMessage {
                    message: None,
                }));
            }
            // A frame now and then twice as long, as a browser drops one.
            let dt = if frame % 7 == 0 {
                1.0 / 30.0
            } else {
                1.0 / 60.0
            };
            let drawn = client.update(dt, &mut input);
            let camera = drawn.camera;
            let look = camera.rotation * glam::DVec3::NEG_Z;
            let before = drawn
                .boxes
                .iter()
                .map(|part| part.transform.translation - camera.position)
                .filter(|to| to.length() < 15.0 && to.normalize().dot(look) > 0.75)
                .count();
            assert!(
                before >= 20,
                "sprint {sprint}, look {look_px}, frame {frame}: {before} boxes before the camera"
            );
            let apart_m = (camera.position - client.host("ride").body().position).length();
            assert!(
                apart_m < 10.0,
                "sprint {sprint}, look {look_px}, frame {frame}: the camera {apart_m} m off"
            );
            top_mps = top_mps.max(client.host("ride").in_ways()[0].speed_mps);
        }
        assert!(client.host("ride").aloft(), "it flew all along");
        if frames > 2000 {
            assert!(top_mps > 140.0, "it reached its top: {top_mps} m/s");
        }
    }
}

/// What the plugins point out as the seam last said it, and where each is.
fn marked(events: &[Event]) -> (Option<serde_json::Value>, Option<serde_json::Value>) {
    let mut said = None;
    let mut at = None;
    for event in events {
        match event {
            Event::MarkersChanged { markers } => {
                said = Some(serde_json::to_value(markers).unwrap())
            }
            Event::MarkersAt { at: now } => at = Some(serde_json::to_value(now).unwrap()),
            _ => {}
        }
    }
    (said, at)
}

/// One's own pet, kilometres away, is pointed out on the screen's edge with
/// how far it is, read the same by an agent; its choices are the whistle
/// and letting it go, sent as they are. Under the body, it is not.
#[test]
fn ones_own_pet_far_away_is_pointed_out_called_and_let_go() {
    use protocol::{Message as _, life as wire};
    let mut client = welcomed();
    let mut input = Input::default();
    run(&mut client, &mut input, 1);
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    pet_told(&mut client, horse.number, "Bay", horse.at);
    let grid = client.generator().sphere().blocks();
    let radius_m = client.generator().sphere().radius_m();
    let up = horse.at.normalize();
    let across = up.cross(glam::DVec3::Z).normalize();
    let far_at = glam::DQuat::from_axis_angle(across, 2000.0 / radius_m) * horse.at;
    client.teleport(grid.surface_point(far_at.to_array()));
    run(&mut client, &mut input, 3);
    let (said, at) = marked(&client.drain_events());

    let said = said.expect("the pet is pointed out");
    let pet = &said[0];
    assert_eq!(
        (pet["giver"].as_str(), pet["kind"].as_str()),
        (Some("ride"), Some("pet"))
    );
    assert_eq!(pet["on"], serde_json::json!({ "animal": horse.number }));
    assert_eq!(
        (pet["label"].as_str(), pet["glyph"].as_str()),
        (Some("Bay"), Some("paw-print"))
    );
    let choices = pet["choices"].as_array().unwrap();
    assert_eq!(
        choices[0]["command"],
        serde_json::json!({ "type": "sound", "voice": "whistle" })
    );
    assert!(choices[0].get("confirm").is_none(), "a call is one press");
    assert_eq!(
        choices[1]["command"],
        serde_json::json!({ "type": "ride.release" })
    );
    assert_eq!(choices[1]["confirm"], "let_go.confirm");

    let at = at.expect("where it is for the eye");
    let distance_m = at[0]["distance_m"].as_f64().unwrap();
    assert!((1900.0..2100.0).contains(&distance_m), "{distance_m}");
    assert_eq!(at[0]["tagged"], false, "too far for its nametag");

    // Still, nothing more is said; turned about, it moves, and out of view
    // it rides the edge.
    run(&mut client, &mut input, 2);
    assert_eq!(marked(&client.drain_events()), (None, None));
    let mut edges = Vec::new();
    for _ in 0..12 {
        input.look = [300.0, 0.0];
        run(&mut client, &mut input, 1);
        if let (_, Some(turned)) = marked(&client.drain_events()) {
            assert_ne!(turned, at, "turned with the eye");
            edges.extend(turned[0].get("edge_deg").map(|_| turned[0].clone()));
        }
    }
    input.look = [0.0, 0.0];
    let edge = edges.first().expect("out of view on some turn");
    let (x, y) = (edge["x"].as_f64().unwrap(), edge["y"].as_f64().unwrap());
    assert!(
        x == 0.0 || x == 1.0 || y == 0.0 || y == 1.0,
        "off the screen it rides the edge: {edge}"
    );

    // An agent reads it with where it is.
    client.command_json(r#"{"type":"markers"}"#);
    let read = client
        .drain_events()
        .into_iter()
        .find_map(|event| match event {
            Event::Markers { markers } => Some(serde_json::to_value(markers).unwrap()),
            _ => None,
        })
        .expect("an answer");
    assert_eq!(read[0]["label"], "Bay");
    assert!(read[0]["place"].is_string() && read[0]["bearing_deg"].is_number());

    // Its choices, sent as the card sends them: the whistle is struck and
    // says the pet is past how far it answers, and letting go is asked of
    // the world from afar, the body on its feet.
    client.command_json(&choices[0]["command"].to_string());
    let whistled = client.drain_events().into_iter().any(|event| {
        matches!(
            event,
            Event::Whistled {
                answered: false,
                ..
            }
        )
    });
    assert!(whistled, "the same whistle as Q, 2 km off: too far");
    client.drain_outbound();
    client.command_json(&choices[1]["command"].to_string());
    let asked = client.drain_outbound().into_iter().any(|out| {
        let client::Outbound::Frame(frame) = out else {
            return false;
        };
        let message = protocol::ClientMessage::decode(&frame[..]).unwrap().message;
        matches!(message,
            Some(protocol::client_message::Message::Envelope(envelope))
                if envelope.kind == wire::RELEASE)
    });
    assert!(asked, "release is asked of the world from afar");

    // Ridden, the pet is under the body and nothing is pointed out.
    client.teleport(grid.surface_point(horse.at.to_array()));
    run(&mut client, &mut input, 2);
    assert!(mount(&mut client, &mut input, horse.number), "mounted");
    run(&mut client, &mut input, 2);
    client.command_json(r#"{"type":"markers"}"#);
    let read = client
        .drain_events()
        .into_iter()
        .find_map(|event| match event {
            Event::Markers { markers } => Some(markers.len()),
            _ => None,
        });
    assert_eq!(read, Some(0));
}
