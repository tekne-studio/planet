//! The world's day: the sun is the world's clock over the length its admin
//! chose, so every client that hears one world draws one sun, and the admin
//! changes the length by one command a person and an agent send alike.

use client::{Client, Command, DayRefused, Event, Input, Outbound, Recipe, day};
use protocol::{Message, client_message, server_message};

fn server(message: server_message::Message) -> Vec<u8> {
    protocol::encode(&protocol::ServerMessage {
        message: Some(message),
    })
}

fn recipe_of(client: &Client) -> protocol::Recipe {
    let recipe = client.recipe();
    protocol::Recipe {
        seed: worldgen::format_seed(recipe.seed),
        generator_version: recipe.generator_version,
        params_json: serde_json::to_string(&recipe.params).unwrap(),
    }
}

/// A client of seed 1, welcomed into a world at `clock_ms` whose day is
/// `day_minutes`, with `level`.
fn welcomed(level: protocol::Level, clock_ms: u64, day_minutes: u32) -> Client {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    let welcome = server(server_message::Message::Welcome(protocol::Welcome {
        session: 1,
        recipe: Some(recipe_of(&client)),
        level: level.into(),
        clock_ms,
        day_minutes,
        visitor: level == protocol::Level::Anonymous,
        ..Default::default()
    }));
    client.receive(&welcome);
    client.drain_outbound();
    client
}

fn day_said(minutes: u32) -> Vec<u8> {
    server(server_message::Message::Day(protocol::Day {
        minutes,
        ..Default::default()
    }))
}

/// Where the sun is now, as the frame draws it.
fn sun(client: &mut Client) -> [f32; 3] {
    let frame = client.update(0.0, &mut Input::default());
    client.drain_events();
    frame.sun_direction.to_array()
}

fn days(events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Day { minutes, .. } => Some(*minutes),
            _ => None,
        })
        .collect()
}

/// The days a client asked the world for, among what it sent.
fn asked(client: &mut Client) -> Vec<u32> {
    client
        .drain_outbound()
        .into_iter()
        .filter_map(|out| match out {
            Outbound::Frame(frame) => protocol::ClientMessage::decode(&frame[..]).unwrap().message,
            Outbound::Close => None,
        })
        .filter_map(|message| match message {
            client_message::Message::Day(day) => Some(day.minutes),
            _ => None,
        })
        .collect()
}

fn rejected(events: &[Event]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::Rejected { .. }))
}

#[test]
fn offline_the_day_is_twenty_four_minutes() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    let events = client.drain_events();
    assert!(
        events.contains(&Event::Day {
            minutes: day::DEFAULT_MINUTES,
            lengths: vec![24, 48, 120, 240],
            refused: None,
        }),
        "the day is said first, with the lengths to choose among"
    );
    assert_eq!(day::DEFAULT_MINUTES, 24);
}

/// The sun is where the clock over each length puts it, and a world that
/// never chose is the client's own day.
#[test]
fn the_sun_is_the_clock_over_the_day() {
    const CLOCK_MS: u64 = 1_764_000_123_456;
    for minutes in [0, 24, 48, 120, 240] {
        let mut client = welcomed(protocol::Level::Anonymous, CLOCK_MS, minutes);
        let length = if minutes == 0 {
            day::DEFAULT_MINUTES
        } else {
            minutes
        };
        let want = day::sun_toward(day::sun_angle(CLOCK_MS as f64 / 1000.0, length)).as_vec3();
        let drawn = glam::Vec3::from_array(sun(&mut client));
        assert!(
            drawn.distance(want) < 1e-5,
            "{minutes} min: {drawn} for {want}"
        );
    }
}

/// Two clients of one world draw one sun; the admin's change moves it in
/// both at once, and a client that arrives after draws it where they do.
#[test]
fn a_change_of_day_moves_the_sun_in_every_eye_at_once() {
    // Not a whole number of either day: a clock that is would draw the sun
    // where it was.
    const CLOCK_MS: u64 = 1_764_000_777_000;
    let mut admin = welcomed(protocol::Level::Admin, CLOCK_MS, 48);
    let mut visitor = welcomed(protocol::Level::Anonymous, CLOCK_MS, 48);
    let before = sun(&mut admin);
    assert_eq!(before, sun(&mut visitor), "one world, one sun");

    admin.command(Command::SetDay { minutes: 240 });
    assert_eq!(asked(&mut admin), [240], "the admin asks the world");
    assert_eq!(
        sun(&mut admin),
        before,
        "and the sun waits for the world's word"
    );

    // The world says it to everyone here, the asker too.
    for client in [&mut admin, &mut visitor] {
        client.receive(&day_said(240));
        assert_eq!(days(&client.drain_events()), [240]);
    }
    let after = sun(&mut admin);
    assert_ne!(after, before, "the sun jumps");
    assert_eq!(after, sun(&mut visitor), "in every eye at once");

    let mut late = welcomed(protocol::Level::Anonymous, CLOCK_MS, 240);
    assert_eq!(
        sun(&mut late),
        after,
        "and whoever arrives after sees it there"
    );
}

#[test]
fn the_day_is_the_world_admin_s_to_choose() {
    let mut offline = Client::new(Recipe::new(1)).unwrap();
    offline.drain_events();
    offline.command(Command::SetDay { minutes: 120 });
    assert!(
        rejected(&offline.drain_events()),
        "offline there is no world to ask"
    );

    for level in [
        protocol::Level::Anonymous,
        protocol::Level::SignedIn,
        protocol::Level::Builder,
    ] {
        let mut client = welcomed(level, 0, 0);
        client.drain_events();
        client.command(Command::SetDay { minutes: 120 });
        assert!(rejected(&client.drain_events()), "{level:?} may not");
        assert!(asked(&mut client).is_empty(), "{level:?} asks nothing");
    }

    let mut admin = welcomed(protocol::Level::Admin, 0, 0);
    admin.drain_events();
    for minutes in [0, 20, 30, 1440] {
        admin.command(Command::SetDay { minutes });
        assert!(
            rejected(&admin.drain_events()),
            "{minutes} is not a listed length"
        );
    }
    assert!(asked(&mut admin).is_empty(), "and nothing goes out for one");
}

/// The day in force is answered at once and asks the world nothing; an ask
/// the world did not take comes back to the asker alone, the day still in
/// force and why.
#[test]
fn an_ask_that_does_not_land_is_answered() {
    let mut admin = welcomed(protocol::Level::Admin, 0, 48);
    admin.drain_events();
    admin.command(Command::SetDay { minutes: 48 });
    assert_eq!(
        days(&admin.drain_events()),
        [48],
        "the day in force is answered at once"
    );
    assert!(asked(&mut admin).is_empty(), "with nothing asked");

    admin.command(Command::SetDay { minutes: 120 });
    assert_eq!(asked(&mut admin), [120]);
    for (refusal, refused) in [
        (protocol::DayRefusal::Rate, DayRefused::Rate),
        (protocol::DayRefusal::Fault, DayRefused::Fault),
    ] {
        admin.receive(&server(server_message::Message::Day(protocol::Day {
            minutes: 48,
            refusal: refusal.into(),
        })));
        assert!(admin.drain_events().contains(&Event::Day {
            minutes: 48,
            lengths: vec![24, 48, 120, 240],
            refused: Some(refused),
        }));
    }
    assert_eq!(
        serde_json::to_string(&DayRefused::Rate).unwrap(),
        r#""rate""#
    );
}

/// A command reads as JSON the way a front end and an agent send it.
#[test]
fn set_day_is_a_command_of_the_seam() {
    let command: Command = serde_json::from_str(r#"{"type":"set_day","minutes":120}"#).unwrap();
    assert_eq!(command, Command::SetDay { minutes: 120 });
}

/// The place line's hour is the sun's at the time zone the body stands in, on the
/// planet; the moon keeps no clock.
#[test]
fn the_hour_is_read_where_the_place_is() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.set_clock(0.0);
    let mut input = Input::default();
    let mut hour = None;
    for _ in 0..60 {
        client.update(1.0 / 60.0, &mut input);
        for event in client.drain_events() {
            if let Event::Stats { hour_min, .. } = event {
                hour = Some(hour_min);
            }
        }
    }
    let want = day::hour_min(client.up(), day::sun_angle(1.0, day::DEFAULT_MINUTES));
    let hour = hour.expect("stats twice a second").expect("on the planet");
    assert!(hour.abs_diff(want) <= 1, "{hour} for {want}");
}
