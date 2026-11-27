//! Two people see each other: the client's side of the link, driven by hand
//! made server frames, the way a shell drives it with a socket.

use client::{Client, Command, Event, Input, Level, Outbound, PeerInfo, Recipe, SessionStatus};
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

fn stance(u: f32) -> protocol::Stance {
    protocol::Stance {
        sector: 0,
        u,
        v: 30_000.0,
        height_m: 5.0,
        facing_z: -1.0,
        gait: protocol::Gait::Walk.into(),
        speed_mps: 1.5,
        ..Default::default()
    }
}

fn peer(session: u32, name: &str) -> protocol::Peer {
    protocol::Peer {
        session,
        name: name.into(),
        visitor: name.is_empty(),
        avatar: String::new(),
        stance: Some(stance(30_000.0)),
    }
}

/// A welcome to a visitor: the least a world says a session may do.
fn welcome(client: &Client, session: u32, peers: Vec<protocol::Peer>) -> Vec<u8> {
    welcome_as(client, session, peers, protocol::Level::Anonymous)
}

fn welcome_as(
    client: &Client,
    session: u32,
    peers: Vec<protocol::Peer>,
    level: protocol::Level,
) -> Vec<u8> {
    welcome_at(client, session, peers, level, 0)
}

fn welcome_at(
    client: &Client,
    session: u32,
    peers: Vec<protocol::Peer>,
    level: protocol::Level,
    clock_ms: u64,
) -> Vec<u8> {
    server(server_message::Message::Welcome(protocol::Welcome {
        session,
        recipe: Some(recipe_of(client)),
        peers,
        level: level.into(),
        plugins: vec![],
        clock_ms,
        avatar: String::new(),
        visitor: level == protocol::Level::Anonymous,
        day_minutes: 0,
    }))
}

fn sent(client: &mut Client) -> Vec<client_message::Message> {
    client
        .drain_outbound()
        .into_iter()
        .filter_map(|out| match out {
            Outbound::Frame(frame) => protocol::ClientMessage::decode(&frame[..]).unwrap().message,
            Outbound::Close => None,
        })
        .collect()
}

fn run(client: &mut Client, seconds: f64) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..(seconds * 60.0) as u32 {
        client.update(1.0 / 60.0, &mut Input::default());
        events.extend(client.drain_events());
    }
    events
}

#[test]
fn hello_goes_out_and_welcome_brings_the_peers() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.drain_events();
    client.link_opened();
    let hello = sent(&mut client);
    assert!(
        matches!(&hello[..], [client_message::Message::Hello(h)] if h.protocol == protocol::PROTOCOL),
        "{hello:?}"
    );

    client.receive(&welcome(&client, 7, vec![peer(3, "Ada"), peer(5, "")]));
    let events = client.drain_events();
    assert!(events.contains(&Event::Session {
        status: SessionStatus::Online,
        session: Some(7),
        level: Some(Level::Anonymous),
    }));
    assert!(events.contains(&Event::Peers {
        peers: vec![
            PeerInfo {
                session: 3,
                name: "Ada".into(),
                visitor: false
            },
            PeerInfo {
                session: 5,
                name: String::new(),
                visitor: true
            },
        ]
    }));

    // Three bodies, the player's and two peers, all boxes: no file has landed.
    let frame = client.settled_frame();
    assert_eq!(frame.boxes.len(), 3 * 6, "{}", frame.boxes.len());
}

#[test]
fn a_peer_moves_between_the_stances_it_sent() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    client.receive(&welcome(&client, 1, vec![peer(2, "Ada")]));
    run(&mut client, 0.5);
    let before = client.settled_frame().boxes[6].transform.translation;

    client.receive(&server(server_message::Message::Stances(
        protocol::Stances {
            moved: vec![protocol::Moved {
                session: 2,
                stance: Some(stance(30_010.0)),
            }],
        },
    )));
    run(&mut client, 0.5);
    let after = client.settled_frame().boxes[6].transform.translation;
    let moved = (after - before).length();
    // Ten blocks of half a metre, give or take the tangent warp.
    assert!((4.0..6.5).contains(&moved), "{moved}");

    // The player's own stance echoed back moves nobody.
    client.receive(&server(server_message::Message::Stances(
        protocol::Stances {
            moved: vec![protocol::Moved {
                session: 1,
                stance: Some(stance(0.0)),
            }],
        },
    )));
    run(&mut client, 0.5);
    assert_eq!(client.settled_frame().boxes.len(), 12);
}

#[test]
fn leaving_and_losing_the_link_take_the_bodies_away() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    client.receive(&welcome(&client, 1, vec![peer(2, "Ada"), peer(3, "Bo")]));
    client.receive(&server(server_message::Message::Left(protocol::Left {
        session: 2,
    })));
    let events = client.drain_events();
    assert!(matches!(
        events.last(),
        Some(Event::Peers { peers }) if peers.len() == 1 && peers[0].session == 3
    ));
    assert_eq!(client.settled_frame().boxes.len(), 12);

    client.link_closed();
    let events = client.drain_events();
    // The world's word on the level stands through a dropped link.
    assert!(events.contains(&Event::Session {
        status: SessionStatus::Offline,
        session: None,
        level: Some(Level::Anonymous),
    }));
    assert!(events.contains(&Event::Peers { peers: vec![] }));
    assert_eq!(client.settled_frame().boxes.len(), 6);
}

/// A world of another protocol leaves this build outdated for good: every
/// drop after says so, so a page reloads for the world's own build instead
/// of linking again. Any other refusal is a drop like any.
#[test]
fn a_world_of_another_protocol_leaves_this_build_outdated() {
    for (refusal, dropped) in [
        (protocol::Refusal::Protocol, SessionStatus::Outdated),
        (protocol::Refusal::World, SessionStatus::Offline),
    ] {
        let mut client = Client::new(Recipe::new(1)).unwrap();
        client.drain_events();
        client.link_opened();
        client.receive(&server(server_message::Message::Refused(
            protocol::Refused {
                refusal: refusal.into(),
            },
        )));
        client.link_closed();
        client.link_opened();
        client.link_closed();
        let said: Vec<SessionStatus> = client
            .drain_events()
            .into_iter()
            .filter_map(|event| match event {
                Event::Session { status, .. } => Some(status),
                _ => None,
            })
            .collect();
        assert_eq!(
            said,
            [
                SessionStatus::Connecting,
                dropped,
                dropped,
                SessionStatus::Connecting,
                dropped
            ],
            "{refusal:?}"
        );
    }
    assert_eq!(
        serde_json::to_string(&SessionStatus::Outdated).unwrap(),
        r#""outdated""#
    );
}

#[test]
fn the_stance_goes_out_when_it_changes_and_as_a_heartbeat() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    client.receive(&welcome(&client, 1, vec![]));
    sent(&mut client);

    // Standing still: one stance, then the heartbeat every two seconds.
    run(&mut client, 3.0);
    let stances = sent(&mut client)
        .into_iter()
        .filter(|m| matches!(m, client_message::Message::Stance(_)))
        .count();
    assert_eq!(stances, 2, "one at once, one two seconds later");

    // Walking: a stance about fifteen times a second.
    let mut input = Input::default();
    input.key(client::Key::Forward, true);
    for _ in 0..60 {
        client.update(1.0 / 60.0, &mut input);
    }
    // Beside them go the looks of the cells and the parts, once a second.
    let stances = sent(&mut client)
        .into_iter()
        .filter(|m| matches!(m, client_message::Message::Stance(_)))
        .count();
    assert!((10..=16).contains(&stances), "{stances}");
}

/// The gaits of the stances sent since the last look.
fn gaits(client: &mut Client) -> Vec<protocol::Gait> {
    sent(client)
        .into_iter()
        .filter_map(|m| match m {
            client_message::Message::Stance(stance) => Some(stance.gait()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_clip_asked_for_goes_out_as_the_gait_while_it_plays() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    client.receive(&welcome(&client, 1, vec![]));
    run(&mut client, 1.0);
    sent(&mut client);

    // Asked at the seam, as an agent asks: it falls for a while, then
    // stands again.
    client.command_json(r#"{"type":"play","clip":"fall","for_s":0.5}"#);
    run(&mut client, 0.3);
    assert_eq!(gaits(&mut client), vec![protocol::Gait::Fall]);
    run(&mut client, 0.4);
    assert_eq!(gaits(&mut client), vec![protocol::Gait::Idle]);

    // One that moving ends, ended by a step.
    client.command_json(r#"{"type":"play","clip":"fly","for_s":5,"until_moved":true}"#);
    run(&mut client, 0.2);
    assert_eq!(gaits(&mut client), vec![protocol::Gait::Fly]);
    let mut input = Input::default();
    input.key(client::Key::Forward, true);
    client.update(1.0 / 60.0, &mut input);
    assert_ne!(gaits(&mut client).last(), Some(&protocol::Gait::Fly));

    // A length that is no length of time is refused, and nothing plays.
    client.command_json(r#"{"type":"play","clip":"jump","for_s":-1}"#);
    assert!(
        client
            .drain_events()
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. }))
    );
}

#[test]
fn another_recipe_is_another_world() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    let mut recipe = recipe_of(&client);
    recipe.seed = worldgen::format_seed(2);
    client.receive(&server(server_message::Message::Welcome(
        protocol::Welcome {
            session: 1,
            recipe: Some(recipe),
            peers: vec![],
            level: protocol::Level::Admin.into(),
            plugins: vec![],
            clock_ms: 0,
            avatar: String::new(),
            visitor: false,
            day_minutes: 0,
        },
    )));
    assert!(client.drain_outbound().contains(&Outbound::Close));
    // A world this client never stood in says nothing of its level either.
    assert!(client.drain_events().contains(&Event::Session {
        status: SessionStatus::Offline,
        session: None,
        level: None,
    }));
}

#[test]
fn silence_ends_the_link() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    client.receive(&welcome(&client, 1, vec![]));
    run(&mut client, 11.0);
    assert!(client.drain_outbound().contains(&Outbound::Close));
}

#[test]
fn heads_are_anchored_on_the_screen_while_online() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    let events = run(&mut client, 0.2);
    assert!(
        !events.iter().any(|e| matches!(e, Event::Anchors { .. })),
        "nobody is anchored offline"
    );

    client.link_opened();
    client.receive(&welcome(&client, 1, vec![peer(2, "Ada")]));
    let events = run(&mut client, 0.2);
    let Some(Event::Anchors { anchors }) = events
        .iter()
        .rev()
        .find(|e| matches!(e, Event::Anchors { .. }))
    else {
        panic!("the own head is anchored every frame: {events:?}");
    };
    let own = anchors
        .iter()
        .find(|a| a.session == 1)
        .expect("the own head, in view of the own camera");
    // The camera looks at the eyes and the head sits just above: near the
    // middle of the screen, a few metres off.
    assert!(
        (0.3..0.7).contains(&own.x) && (0.2..0.7).contains(&own.y),
        "{own:?}"
    );
    assert!(own.distance_m > 0.5 && own.distance_m < 20.0, "{own:?}");

    client.link_closed();
    let events = run(&mut client, 0.1);
    assert!(
        events.contains(&Event::Anchors { anchors: vec![] }),
        "the last anchors are taken down: {events:?}"
    );
}

#[test]
fn a_name_rides_in_hello_and_a_peer_renamed_is_listed_anew() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.command(Command::SetName { name: "Zed".into() });
    client.link_opened();
    let hello = sent(&mut client);
    assert!(
        matches!(&hello[..], [client_message::Message::Hello(h)] if h.name == "Zed"),
        "{hello:?}"
    );
    client.receive(&welcome(&client, 1, vec![peer(2, "Ada")]));
    sent(&mut client);

    client.command(Command::SetName { name: "Zoe".into() });
    let out = sent(&mut client);
    assert!(
        matches!(&out[..], [client_message::Message::Rename(r)] if r.name == "Zoe"),
        "{out:?}"
    );

    client.drain_events();
    client.receive(&server(server_message::Message::Renamed(
        protocol::Renamed {
            session: 2,
            name: "Grace".into(),
        },
    )));
    assert!(matches!(
        client.drain_events().last(),
        Some(Event::Peers { peers }) if peers.len() == 1 && peers[0].name == "Grace"
    ));
}

/// The sky is the world's, not the client's: two clients welcomed by one
/// server at one moment see the sun and the moon in the same place, however
/// long each ran alone, and whenever each came to this recipe.
#[test]
fn one_welcome_sets_one_sky() {
    let mut a = Client::new(Recipe::new(7)).unwrap();
    let mut b = Client::new(Recipe::new(3)).unwrap();
    let mut input = Input::default();
    // A ran a while; B ran a while and only then turned to A's world.
    for _ in 0..600 {
        a.update(1.0 / 60.0, &mut input);
    }
    for _ in 0..1500 {
        b.update(1.0 / 60.0, &mut input);
    }
    b.command(Command::SetRecipe {
        recipe: a.recipe().clone(),
    });
    let apart = a.settled_frame().sun_direction - b.settled_frame().sun_direction;
    assert!(apart.length() > 0.01, "alone, their suns disagree: {apart}");

    let now_ms = 1_760_000_000_000;
    for (client, session) in [(&mut a, 1), (&mut b, 2)] {
        client.link_opened();
        let frame = welcome_at(client, session, vec![], protocol::Level::Anonymous, now_ms);
        client.receive(&frame);
    }
    let (fa, fb) = (a.settled_frame(), b.settled_frame());
    assert!(
        (fa.sun_direction - fb.sun_direction).length() < 1e-5,
        "the sun: {} and {}",
        fa.sun_direction,
        fb.sun_direction
    );
    let (sa, sb) = (fa.sun_steps, fb.sun_steps);
    assert!(
        (sa.from - sb.from).length() < 1e-5 && (sa.to - sb.to).length() < 1e-5,
        "the sun their shadows are drawn from: {sa:?} and {sb:?}"
    );
    assert!(
        (fa.moon.position - fb.moon.position).length() < 1e-3,
        "the moon: {} and {}",
        fa.moon.position,
        fb.moon.position
    );
    assert_eq!(fa.clock_s, now_ms as f64 / 1000.0);
}

/// A plugin that gives one way of moving, and holds the hips turned a
/// quarter round in it.
struct Boards;

impl client::Plugin for Boards {
    fn name(&self) -> &'static str {
        "test"
    }

    fn version(&self) -> u32 {
        1
    }

    /// `test.wave` plays a clip as a plugin asks for one.
    fn command(&mut self, kind: &str, _body: serde_json::Value, host: &mut client::Host<'_>) {
        if kind == "wave" {
            host.play(client::Play {
                clip: client::Gait::Swim,
                for_s: Some(5.0),
                until_moved: false,
            });
        }
    }

    fn ways(&self) -> Vec<client::Way> {
        vec![client::Way {
            name: "board",
            profile: client::Profile::Hover(client::Hover {
                height_m: 0.5,
                top_mps: 10.0,
                sprint_mps: 20.0,
                accel_mps2: 8.0,
                turn_per_s: 2.0,
                lean_rad: 0.5,
                coast_per_s: 0.5,
                slope: 0.5,
                over_sea: true,
                bob_m: 0.0,
                bob_hz: 0.0,
                span_m: 1.0,
                footprint_m: 2.0,
                lift_mps: 0.0,
                sink_mps: 5.0,
                ceiling_m: 0.0,
                hop_m: 0.0,
                jump_m: 0.0,
            }),
            steering: client::Steering::Steered,
            held: client::Held {
                bones: vec![(
                    "hips",
                    glam::Quat::from_rotation_y(core::f32::consts::FRAC_PI_2),
                )],
                hips: glam::Vec3::Y,
            },
        }]
    }
}

fn ways(client: &mut Client) -> Vec<String> {
    sent(client)
        .into_iter()
        .filter_map(|m| match m {
            client_message::Message::Stance(stance) => Some(stance.way),
            _ => None,
        })
        .collect()
}

/// A client online with [`Boards`] plugged and on, and `peers` here.
fn with_boards(peers: Vec<protocol::Peer>) -> Client {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.plug(Box::new(Boards));
    client.link_opened();
    let mut welcome = welcome(&client, 1, peers);
    if let Some(server_message::Message::Welcome(mut said)) =
        protocol::ServerMessage::decode(&welcome[..])
            .unwrap()
            .message
    {
        said.plugins = vec![protocol::Plugin {
            name: "test".into(),
            version: 1,
        }];
        welcome = server(server_message::Message::Welcome(said));
    }
    client.receive(&welcome);
    run(&mut client, 0.5);
    sent(&mut client);
    client
}

#[test]
fn a_way_goes_out_in_the_stance_and_a_peer_in_one_holds_its_pose_for_the_plugin() {
    let mut client = with_boards(vec![peer(2, "Ada")]);

    // The body's own way rides its stance, and on foot again none does.
    client.command_json(r#"{"type":"set_mode","mode":"test.board"}"#);
    run(&mut client, 0.2);
    assert_eq!(
        ways(&mut client).last().map(String::as_str),
        Some("test.board")
    );
    client.command_json(r#"{"type":"set_mode","mode":"walk"}"#);
    run(&mut client, 0.2);
    assert_eq!(ways(&mut client).last().map(String::as_str), Some(""));

    // A peer standing, then in the way: its figure turns the hips as the
    // way holds them, and the plugin that gave the way is handed it.
    let torso = |client: &mut Client| client.settled_frame().boxes[6].transform.matrix3.x_axis;
    let standing = torso(&mut client).normalize();
    assert!(client.host("test").in_ways().is_empty());
    let riding = protocol::Stance {
        gait: protocol::Gait::Idle.into(),
        speed_mps: 0.0,
        way: "test.board".into(),
        ..stance(30_000.0)
    };
    client.receive(&server(server_message::Message::Stances(
        protocol::Stances {
            moved: vec![protocol::Moved {
                session: 2,
                stance: Some(riding),
            }],
        },
    )));
    run(&mut client, 0.5);
    let held = torso(&mut client).normalize();
    assert!(standing.dot(held).abs() < 0.1, "{standing} {held}");
    let in_ways = client.host("test").in_ways();
    assert_eq!(in_ways.len(), 1);
    assert_eq!((in_ways[0].session, in_ways[0].way), (Some(2), "board"));
}

#[test]
fn a_stance_that_is_not_finite_is_not_heard() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    client.receive(&welcome(&client, 1, vec![peer(2, "Ada")]));
    run(&mut client, 0.5);
    let before = client.settled_frame().boxes[6].transform;

    // A hostile client says a place and a speed that are no numbers: the
    // peer stays where it was heard last, drawn whole.
    for broken in [
        protocol::Stance {
            speed_mps: f32::NAN,
            ..stance(30_010.0)
        },
        protocol::Stance {
            facing_x: f32::INFINITY,
            ..stance(30_010.0)
        },
        stance(f32::NAN),
    ] {
        client.receive(&server(server_message::Message::Stances(
            protocol::Stances {
                moved: vec![protocol::Moved {
                    session: 2,
                    stance: Some(broken),
                }],
            },
        )));
    }
    run(&mut client, 0.5);
    let after = client.settled_frame().boxes[6].transform;
    assert!(after.is_finite(), "{after:?}");
    assert!((after.translation - before.translation).length() < 1e-6);
}

#[test]
fn a_clip_asked_for_ends_with_how_the_body_moved_a_passage_the_world_or_who_asked() {
    let mut client = with_boards(vec![]);
    let swimming = |client: &mut Client| gaits(client).last() == Some(&protocol::Gait::Swim);
    let ask = r#"{"type":"play","clip":"swim","for_s":10}"#;

    // Longer than ten seconds is a state of the body, not a clip: refused.
    client.command_json(r#"{"type":"play","clip":"swim","for_s":1e9}"#);
    let refused = client.drain_events();
    assert!(
        refused
            .iter()
            .any(|event| matches!(event, Event::Rejected { .. }))
    );

    // Taken to the air, the body plays what it moves by.
    client.command_json(ask);
    run(&mut client, 0.2);
    assert!(swimming(&mut client));
    client.command_json(r#"{"type":"set_mode","mode":"fly"}"#);
    run(&mut client, 0.2);
    assert_eq!(gaits(&mut client).last(), Some(&protocol::Gait::Fly));

    // A passage ends it: out the other side, it does not swim again.
    client.command_json(r#"{"type":"set_mode","mode":"walk"}"#);
    run(&mut client, 1.0);
    client.command_json(ask);
    run(&mut client, 0.2);
    assert!(swimming(&mut client));
    client.command_json(r#"{"type":"travel","place":"0-1"}"#);
    run(&mut client, 4.0);
    let through = gaits(&mut client);
    assert!(through.contains(&protocol::Gait::Fall), "{through:?}");
    assert_eq!(through.last(), Some(&protocol::Gait::Idle), "{through:?}");

    // Asked by a plugin, it ends as the world switches the plugin off.
    client.command_json(r#"{"type":"test.wave"}"#);
    run(&mut client, 0.2);
    assert!(swimming(&mut client));
    client.receive(&server(server_message::Message::Plugins(
        protocol::Plugins { plugins: vec![] },
    )));
    run(&mut client, 0.2);
    assert_eq!(gaits(&mut client).last(), Some(&protocol::Gait::Idle));

    // Another world ends it: the body there stands, its arms along it.
    let arms_down = |client: &mut Client| {
        let boxes = client.settled_frame().boxes;
        let along = |part: usize| boxes[part].transform.matrix3.y_axis.normalize();
        along(0).dot(along(4)) > 0.999
    };
    client.command_json(ask);
    run(&mut client, 0.2);
    assert!(!arms_down(&mut client));
    client.command_json(&format!(
        r#"{{"type":"set_recipe","recipe":{}}}"#,
        serde_json::to_string(&Recipe::new(2)).unwrap()
    ));
    run(&mut client, 2.0);
    assert!(arms_down(&mut client));
}

#[test]
fn a_way_whose_name_a_stance_cannot_carry_is_refused_as_it_is_plugged() {
    /// [`Boards`]'s board, under a name one byte too long with the
    /// plugin's, and one exactly as long as a stance carries.
    struct Long;
    impl client::Plugin for Long {
        fn name(&self) -> &'static str {
            "test"
        }
        fn version(&self) -> u32 {
            1
        }
        fn command(&mut self, _: &str, _: serde_json::Value, _: &mut client::Host<'_>) {}
        fn ways(&self) -> Vec<client::Way> {
            let board = Boards.ways().remove(0);
            vec![
                client::Way {
                    name: "a_board_whose_name_runs_over",
                    ..board.clone()
                },
                client::Way {
                    name: "a_board_whose_name_fills_it",
                    ..board
                },
            ]
        }
    }
    let limit = protocol::Limit::WayBytes as usize;
    assert_eq!("test.a_board_whose_name_runs_over".len(), limit + 1);
    assert_eq!("test.a_board_whose_name_fills_it".len(), limit);

    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.drain_events();
    client.plug(Box::new(Long));
    let refused = client.drain_events();
    assert!(
        matches!(&refused[..], [Event::Rejected { message }] if message.contains("runs_over")),
        "{refused:?}"
    );
    client.rehearse(client::Level::Builder);
    client.command_json(r#"{"type":"set_mode","mode":"test.a_board_whose_name_runs_over"}"#);
    let events = run(&mut client, 0.1);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::ModeChanged { .. })),
        "{events:?}"
    );
    client.command_json(r#"{"type":"set_mode","mode":"test.a_board_whose_name_fills_it"}"#);
    let events = run(&mut client, 0.1);
    let mode = client::Mode::Way("test.a_board_whose_name_fills_it".into());
    assert!(events.contains(&Event::ModeChanged { mode }), "{events:?}");
}

#[test]
fn a_whistle_goes_out_with_its_seed_and_a_peers_sounds_at_its_mouth() {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.link_opened();
    client.receive(&welcome(&client, 1, vec![peer(2, "Ada")]));
    run(&mut client, 0.5);
    sent(&mut client);
    client.drain_strikes();

    client.command(Command::Sound {
        voice: "whistle".into(),
    });
    let own = client.drain_strikes();
    let said: Vec<_> = sent(&mut client)
        .into_iter()
        .filter_map(|message| match message {
            client_message::Message::Sound(sound) => Some(sound),
            _ => None,
        })
        .collect();
    assert_eq!(said.len(), 1);
    assert_eq!(
        (said[0].voice.as_str(), said[0].seed),
        ("whistle", own[0].seed)
    );

    // A peer's is struck where its body is, with the seed it took; a voice
    // this client does not hold, a jump from a peer on no horse, or a
    // session it has not seen, is not.
    let sounded = |session: u32, voice: &str, seed: u32| {
        server(server_message::Message::Sounded(protocol::Sounded {
            session,
            voice: voice.into(),
            seed,
        }))
    };
    client.receive(&sounded(2, "whistle", 9));
    client.receive(&sounded(2, "neigh", 1));
    client.receive(&sounded(2, "spring", 1));
    client.receive(&sounded(7, "whistle", 1));
    let heard = client.drain_strikes();
    assert_eq!(heard.len(), 1);
    assert_eq!(
        (heard[0].seed, heard[0].reach_m),
        (9 % client::VARIANTS, 150.0)
    );
    assert_eq!(
        heard[0].group,
        client::Group::Calls,
        "a peer's whistle is a call"
    );
    let ada = heard[0].position;
    assert!(
        ada.distance(own[0].position) > 100.0,
        "at Ada's mouth, far from this body"
    );
}
