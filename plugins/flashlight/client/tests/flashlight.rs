//! The flashlight, as a front end, an agent and a shell's key ask for it:
//! the plugin plugged into a client that stands in for a world.

use client::{Client, Event, Frame, Input, Level, Recipe};
use protocol::Message;

/// A client standing on dry land, with the flashlight plugged in and on.
fn alone() -> Client {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    client.plug(flashlight_client::plugin());
    client.go_to("4-77AEYRG").expect("a place to stand");
    client.rehearse(Level::Anonymous);
    client.update(1.0 / 60.0, &mut Input::default());
    client.drain_events();
    client
}

fn run(client: &mut Client, input: &mut Input, frames: u32) -> Frame {
    let mut frame = client.update(1.0 / 60.0, input);
    for _ in 1..frames {
        frame = client.update(1.0 / 60.0, input);
    }
    frame
}

/// What the flashlight told, `on` each time.
fn told(client: &mut Client) -> Vec<bool> {
    client
        .drain_events()
        .into_iter()
        .filter_map(|event| match event {
            Event::Plugin(event) if event["type"] == "flashlight.changed" => event["on"].as_bool(),
            _ => None,
        })
        .collect()
}

/// The beams lighting a frame.
fn beams(frame: &Frame) -> Vec<scene::Beam> {
    frame.lamps.iter().filter_map(|lamp| lamp.beam).collect()
}

#[test]
fn an_agent_lights_the_flashlight_in_the_hand_and_puts_it_away() {
    let mut client = alone();
    let mut input = Input::default();
    let dark = run(&mut client, &mut input, 1);
    assert!(beams(&dark).is_empty());

    client.command_json(r#"{"type":"flashlight.on"}"#);
    assert_eq!(told(&mut client), vec![true]);
    assert_eq!(
        client.host("flashlight").grasping(),
        Some(flashlight_client::THING)
    );
    // Said again, it is already so: nothing is told.
    client.command_json(r#"{"type":"flashlight.on"}"#);
    assert_eq!(told(&mut client), Vec::<bool>::new());

    // The arm comes up, and the flashlight shows in it, its beam thrown ahead.
    let lit = run(&mut client, &mut input, 30);
    let thrown = beams(&lit);
    assert_eq!(thrown.len(), 1);
    let body = client.host("flashlight").body();
    let ahead = thrown[0].direction.as_dvec3();
    let up = body.position.normalize();
    assert!(
        ahead.dot(up) < -0.05,
        "it falls a little: {}",
        ahead.dot(up)
    );
    assert!(ahead.dot(up) > -0.5, "and goes ahead: {}", ahead.dot(up));
    assert!(lit.boxes.len() >= dark.boxes.len() + 3);

    client.command_json(r#"{"type":"flashlight.off"}"#);
    assert_eq!(told(&mut client), vec![false]);
    let put_away = run(&mut client, &mut input, 30);
    assert!(beams(&put_away).is_empty());
    assert_eq!(put_away.boxes.len(), dark.boxes.len());
}

#[test]
fn the_key_switches_it_on_and_off() {
    let mut client = alone();
    let mut input = Input::default();
    let press = |client: &mut Client, input: &mut Input| {
        input.code("KeyL", client::Chord::default(), true);
        run(client, input, 1);
        input.code("KeyL", client::Chord::default(), false);
        run(client, input, 1);
    };
    press(&mut client, &mut input);
    assert_eq!(told(&mut client), vec![true]);
    press(&mut client, &mut input);
    assert_eq!(told(&mut client), vec![false]);
}

#[test]
fn an_unknown_command_is_refused() {
    let mut client = alone();
    client.command_json(r#"{"type":"flashlight.blink"}"#);
    let refused = client
        .drain_events()
        .into_iter()
        .any(|event| matches!(event, Event::Rejected { .. }));
    assert!(refused);
}

fn server(message: protocol::server_message::Message) -> Vec<u8> {
    protocol::encode(&protocol::ServerMessage {
        message: Some(message),
    })
}

/// The stances this client sent since it was last asked.
fn stances(client: &mut Client) -> Vec<protocol::Stance> {
    client
        .drain_outbound()
        .into_iter()
        .filter_map(|out| match out {
            client::Outbound::Frame(frame) => {
                match protocol::ClientMessage::decode(&frame[..]).ok()?.message? {
                    protocol::client_message::Message::Stance(stance) => Some(stance),
                    _ => None,
                }
            }
            client::Outbound::Close => None,
        })
        .collect()
}

/// A client online in a world where the flashlight is on, with peers by
/// their sessions.
fn online(peers: &[u32]) -> Client {
    let mut client = alone();
    client.link_opened();
    let recipe = client.recipe();
    client.receive(&server(protocol::server_message::Message::Welcome(
        protocol::Welcome {
            session: 7,
            recipe: Some(protocol::Recipe {
                seed: worldgen::format_seed(recipe.seed),
                generator_version: recipe.generator_version,
                params_json: serde_json::to_string(&recipe.params).unwrap(),
            }),
            peers: peers
                .iter()
                .map(|&session| protocol::Peer {
                    session,
                    name: format!("peer {session}"),
                    ..Default::default()
                })
                .collect(),
            plugins: vec![protocol::Plugin {
                name: flashlight_client::NAME.into(),
                version: flashlight_client::VERSION,
            }],
            ..Default::default()
        },
    )));
    run(&mut client, &mut Input::default(), 30);
    stances(&mut client);
    client.drain_events();
    client
}

/// A peer's stance, as the world relays it.
fn moved(session: u32, stance: protocol::Stance) -> Vec<u8> {
    server(protocol::server_message::Message::Stances(
        protocol::Stances {
            moved: vec![protocol::Moved {
                session,
                stance: Some(stance),
            }],
        },
    ))
}

/// This body's stance with its flashlight lit, as a peer standing here
/// would say it.
fn lit_stance(client: &mut Client) -> protocol::Stance {
    client.command_json(r#"{"type":"flashlight.on"}"#);
    run(client, &mut Input::default(), 30);
    stances(client).pop().expect("a stance sent")
}

/// A peer grasps its flashlight as this body does: the stance says what its
/// hand grasps, and the peer's flashlight and beam are drawn where it
/// stands.
#[test]
fn the_stance_says_the_flashlight_and_a_peer_s_is_drawn() {
    let mut client = online(&[3]);
    let mut input = Input::default();
    let said = lit_stance(&mut client);
    assert_eq!(said.hand, "flashlight.flashlight");

    // Another body where this one stands, its flashlight in hand.
    client.receive(&moved(3, said.clone()));
    client.receive(&moved(3, said.clone()));
    let both = run(&mut client, &mut input, 30);
    assert_eq!(beams(&both).len(), 2);

    // This body puts its flashlight away; the peer's stays lit.
    client.command_json(r#"{"type":"flashlight.off"}"#);
    let one = run(&mut client, &mut input, 30);
    assert_eq!(beams(&one).len(), 1);
    let said = stances(&mut client).pop().expect("a stance sent");
    assert_eq!(said.hand, "");

    // And the peer's goes as its stance says its hand is empty.
    client.receive(&moved(
        3,
        protocol::Stance {
            hand: String::new(),
            ..said
        },
    ));
    let none = run(&mut client, &mut input, 30);
    assert!(beams(&none).is_empty());
}

/// A peer naming a thing no plugin that is on gave draws nothing in its
/// hand: one this plugin never gave, or one of a plugin not plugged in.
#[test]
fn a_peer_naming_a_thing_unknown_here_draws_none() {
    let mut client = online(&[3, 4]);
    let said = lit_stance(&mut client);
    client.command_json(r#"{"type":"flashlight.off"}"#);
    for (session, hand) in [(3, "flashlight.lantern"), (4, "ride.flashlight")] {
        client.receive(&moved(
            session,
            protocol::Stance {
                hand: hand.into(),
                ..said.clone()
            },
        ));
    }
    let frame = run(&mut client, &mut Input::default(), 30);
    assert!(beams(&frame).is_empty());
}

/// However many hands grasp a flashlight, four light a view, this body's
/// among them.
#[test]
fn four_beams_light_a_view_this_body_s_among_them() {
    let peers = [3, 4, 5, 6, 8];
    let mut client = online(&peers);
    let said = lit_stance(&mut client);
    for (n, &session) in peers.iter().enumerate() {
        client.receive(&moved(
            session,
            protocol::Stance {
                u: said.u + 3.0 + n as f32,
                ..said.clone()
            },
        ));
    }
    let frame = run(&mut client, &mut Input::default(), 30);
    let thrown: Vec<_> = frame
        .lamps
        .iter()
        .filter(|lamp| lamp.beam.is_some())
        .collect();
    assert_eq!(thrown.len(), 4);
    let body = client.host("flashlight").body().position;
    assert!(
        thrown.iter().any(|lamp| lamp.position.distance(body) < 1.5),
        "this body's beam lights the view"
    );
}

/// The lens shines of itself, whatever light the body is drawn in.
#[test]
fn the_lens_is_lit() {
    let mut client = alone();
    let mut input = Input::default();
    let shining = |frame: &Frame| frame.boxes.iter().filter(|part| part.shine >= 1.0).count();
    let dark = run(&mut client, &mut input, 1);
    client.command_json(r#"{"type":"flashlight.on"}"#);
    let lit = run(&mut client, &mut input, 30);
    assert_eq!(shining(&lit), shining(&dark) + 1);
}

/// The hand keeps the flashlight through a new world, as the body keeps
/// its way, and nothing is told.
#[test]
fn the_hand_keeps_it_through_a_new_world() {
    let mut client = alone();
    client.command_json(r#"{"type":"flashlight.on"}"#);
    told(&mut client);
    client.command(client::Command::SetRecipe {
        recipe: Recipe::new(2),
    });
    let frame = run(&mut client, &mut Input::default(), 30);
    assert_eq!(told(&mut client), Vec::<bool>::new());
    assert_eq!(
        client.host("flashlight").grasping(),
        Some(flashlight_client::THING)
    );
    assert_eq!(beams(&frame).len(), 1);
}

/// The world switching the plugin off takes the flashlight back: it is
/// told, the stance's hand empties and the beam goes.
#[test]
fn switched_off_it_lets_go() {
    let mut client = online(&[]);
    lit_stance(&mut client);
    told(&mut client);
    client.receive(&server(protocol::server_message::Message::Plugins(
        protocol::Plugins { plugins: vec![] },
    )));
    assert_eq!(told(&mut client), vec![false]);
    let frame = run(&mut client, &mut Input::default(), 30);
    assert!(beams(&frame).is_empty());
    let said = stances(&mut client).pop().expect("a stance sent");
    assert_eq!(said.hand, "");
}

/// Asked, it says whether the flashlight is in the hand: what the button
/// reads as it mounts.
#[test]
fn asked_it_says_its_state() {
    let mut client = alone();
    let state = |client: &mut Client| {
        client.command_json(r#"{"type":"flashlight.state"}"#);
        client
            .drain_events()
            .into_iter()
            .find_map(|event| match event {
                Event::Plugin(event) if event["type"] == "flashlight.state" => {
                    event["on"].as_bool()
                }
                _ => None,
            })
    };
    assert_eq!(state(&mut client), Some(false));
    client.command_json(r#"{"type":"flashlight.on"}"#);
    assert_eq!(state(&mut client), Some(true));
}
