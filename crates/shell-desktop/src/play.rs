//! A cabinet in front of the body, its game running on the native engine as
//! the window runs it: how a cabinet's screen is looked at headless, from
//! afar or played, and its stations with a body at each.

use std::time::Duration;

use client::{Client, Input, PartId, PartPose, Seat, Shape};
use glam::DQuat;
use render::Headless;

use crate::carts::Carts;

/// One frame of the world and of the games, paced as the window paces
/// them: the games run on their own thread by the cartridges' clock, so a
/// frame here waits as long as a frame there lasts.
const FRAME: Duration = Duration::from_micros(16_667);

/// How long the game runs before it is looked at or played, and how long
/// after the tap the eye eases to its screen.
const SETTLE: u32 = 120;
const EASE: u32 = 120;

/// How a cabinet is shot.
pub struct Cabinet {
    /// The cartridge's name in the world, `<sha256>`: read from
    /// `PLANET_FILES` as a world's file is.
    pub cart: client::ContentId,
    /// Metres from the body to the cabinet's front.
    pub ahead_m: f64,
    /// How far the eye is swung round the body, degrees: the cabinet faces
    /// it, but for one played, which faces the body, as one walks up to a
    /// machine to play it.
    pub yaw_deg: f64,
    /// Whether the body plays it: the controls lent, the eye on its screen.
    pub played: bool,
    /// Whether, played, the body leaves it: it stands at its station, and
    /// the eye is home.
    pub left: bool,
    /// Whether a second player stands at player 2's station.
    pub paired: bool,
    /// What the player holds down once playing, by the names a command
    /// gives the controls: a stick leant, a button sunk.
    pub pressed: Vec<String>,
}

/// Places the cabinet facing the eye, runs its game until its screen
/// shows, and plays it when asked; what is drawn lands in `headless`.
pub fn run(client: &mut Client, headless: &mut Headless, cabinet: &Cabinet) -> Result<(), String> {
    let id = place(client, cabinet)?;
    let mut carts = Carts::default();
    let mut input = Input::default();
    let mut frame = |client: &mut Client, headless: &mut Headless| {
        client.update(1.0 / 60.0, &mut input);
        headless
            .renderer
            .apply_pictures(client.drain_picture_changes());
        carts.tick(client, &headless.renderer);
        std::thread::sleep(FRAME);
    };
    for _ in 0..SETTLE {
        frame(client, headless);
    }
    if cabinet.played {
        client.drain_events();
        client.command_json(r#"{"type":"act"}"#);
        frame(client, headless);
        let pressed = format!(r#"{{"type":"press","controls":{:?}}}"#, cabinet.pressed);
        client.command_json(&pressed);
        for _ in 0..EASE {
            frame(client, headless);
        }
        let lent = client
            .drain_events()
            .into_iter()
            .any(|event| matches!(event, client::Event::ControlsChanged { plugin: Some(_) }));
        if !lent {
            return Err("--play: the cabinet in front lent no controls".into());
        }
        if cabinet.left {
            client.command_json(r#"{"type":"leave"}"#);
            for _ in 0..EASE {
                frame(client, headless);
            }
        }
    }
    if cabinet.paired {
        pair(client, id)?;
        for _ in 0..EASE {
            frame(client, headless);
        }
    }
    Ok(())
}

/// A second player at player 2's station, square on to the screen: a peer
/// joined there, as the world says one to every client near.
fn pair(client: &mut Client, cabinet: PartId) -> Result<(), String> {
    let host = client.host("shot");
    let world = host
        .part_transform(cabinet)
        .ok_or("--pair: the cabinet is gone")?;
    let station = client::cabinet::station(1).ok_or("--pair: the console has one pad")?;
    let feet = host.feet_at(world.transform_point3(station.feet));
    let facing = world.transform_vector3(station.facing).normalize();
    let stance = protocol::Stance {
        body: protocol::Body::Planet.into(),
        sector: feet.seat.sector().index() as u32,
        u: feet.point.u as f32,
        v: feet.point.v as f32,
        height_m: feet.height_m as f32,
        facing_x: facing.x as f32,
        facing_y: facing.y as f32,
        facing_z: facing.z as f32,
        ..protocol::Stance::default()
    };
    let joined = protocol::ServerMessage {
        message: Some(protocol::server_message::Message::Joined(
            protocol::Joined {
                peer: Some(protocol::Peer {
                    session: 2,
                    stance: Some(stance),
                    ..protocol::Peer::default()
                }),
            },
        )),
    };
    client.receive(&protocol::encode(&joined));
    // What it wears comes from disk, as the body's own did.
    client.update(1.0 / 60.0, &mut Input::default());
    while crate::assets::serve(client) {}
    Ok(())
}

/// The cabinet stands `ahead_m` before the body, its front turned to the
/// eye: the body's back, or where `--yaw` swings the eye round it.
fn place(client: &mut Client, cabinet: &Cabinet) -> Result<PartId, String> {
    let mut host = client.host("shot");
    let body = host.body();
    let centre = body.position + body.facing * (cabinet.ahead_m + DEPTH_M / 2.0);
    let at = host.feet_at(centre);
    let ground = host.ground(at.seat, at.point);
    let swing = DQuat::from_axis_angle(body.up, -cabinet.yaw_deg.to_radians());
    let ahead = host.feet_at(centre - swing * body.facing);
    // The part's front is its -y: turned about the up so it faces the eye.
    let toward = [ahead.point.u - at.point.u, ahead.point.v - at.point.v];
    let turn = toward[0].atan2(-toward[1]);
    let size = client::cabinet::SIZE_M.map(|m| m / topology::BLOCK_M);
    let part = client::Part {
        shape: Shape::Cabinet(client::Cabinet {
            cart: Some(cabinet.cart),
            skin: None,
        }),
        pose: PartPose {
            rotation: DQuat::from_rotation_z(turn),
            ..PartPose::upright([at.point.u, at.point.v, ground], size)
        },
        paint: 0,
        solid: true,
        shine: 0.0,
        reach_m: 0.0,
        name: None,
        muted: false,
        volume: 0.0,
    };
    host.place_part(Seat::Sector(at.seat.sector()), part)
        .map_err(|refusal| format!("--play: the cabinet was refused: {refusal:?}"))
}

/// The cabinet's depth, metres: its anchor is its middle.
const DEPTH_M: f64 = client::cabinet::SIZE_M[1];
