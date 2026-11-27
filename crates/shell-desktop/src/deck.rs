//! Pictures of a pegasus left on a deck, with a fixed clock and seed: how
//! an animal left on what is built is looked at. `bun run shot --deck
//! out/deck` lays a deck of cells on pillars, its top 3.5 m over the
//! flattest ground near the nearest pegasus, flies the pegasus over it,
//! lands it there and steps down from it, the body staying where it
//! stepped down. The body's own eye, its boom drawn out to [`BOOM_M`],
//! follows the pegasus, as a person beside it would; each picture is that
//! eye (`watch`) and one [`NEAR`] the pegasus, inside what that eye sees
//! (`near`), since a frame draws only the ground its own eye sees. They are
//! drawn as it is left and [`AT_S`] after, and every [`EVERY`] frames of
//! the [`OFF_S`] from when it comes off the deck's edge walking home. Each
//! frame is `<view>-<ms>.png`, `ms` from the step down, and `deck.json`
//! lists every frame drawn with how high the pegasus stood over the deck's
//! top and over the ground under it.

use std::fmt::Write as _;
use std::path::Path;

use client::{Client, Input, Near, Seat};
use glam::{DQuat, DVec3};
use render::write_png;
use topology::{BLOCK_M, SurfacePoint};
use voxel::{Gesture, Span};

use crate::animals::{Land, flattest};
use crate::shot::{drawn, drawn_into};
use crate::takeoff::{picture, tick};

/// The deck, in cells of its plot: from `FROM` to `TO` each way, its slab
/// `OVER` cells over the highest ground under it, on a pillar at the
/// corners of every bay of `BAY` cells.
const FROM: i32 = 16;
const TO: i32 = 47;
const OVER: i32 = 7;
const BAY: usize = 8;
/// About how high the deck's top stands over the ground, metres.
const OVER_M: f64 = OVER as f64 * BLOCK_M;
/// How high over the deck's top the pegasus is brought before it lands,
/// metres.
const LANDS_FROM_M: f64 = 6.0;
/// When it is drawn after the step down, seconds: as it is left, waiting
/// and walking home.
const AT_S: [f64; 3] = [0.0, 5.0, 60.0];
/// How long it is watched after the step down, at most, seconds.
const WATCHED_S: f64 = 120.0;
/// Coming off the deck, a picture every this many frames of 60 a second,
/// for this long, seconds.
const EVERY: u32 = 6;
const OFF_S: f64 = 2.0;
/// This far under the deck's top it has come off it, metres.
const OFF_M: f64 = 0.05;
/// The body's eye: how long its boom is, metres, and how far it looks down,
/// degrees.
const BOOM_M: f64 = 24.0;
const DOWN_DEG: f64 = 22.0;
/// Near: how far from the pegasus the eye stands, how high over its feet,
/// how far round from the body's eye, degrees, and how high it looks at.
const NEAR: [f64; 4] = [7.0, 2.5, 40.0, 1.2];

/// Writes the frames into `dir`; `boom_m` is how long the body's boom is
/// as the shot begins.
pub fn run(client: &mut Client, size: [u32; 2], boom_m: f64, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut input = Input::default();
    // The boom drawn out and the eye looking down, before anything moves.
    input.zoom_by((boom_m / BOOM_M) as f32);
    input.look = looking(client, None);
    tick(client, &mut input, 0.0);
    (input.zoom, input.look) = (0.0, [0.0; 2]);
    // A pegasus, taking off under its rider.
    crate::shot::ride(client, true)?;
    let number = (client.host("ride").animals().iter())
        .find(|near| near.ridden)
        .map(|near| near.number)
        .ok_or("--deck: no animal ridden")?;
    let generator = client.generator().clone();
    let (grid, radius_m) = (generator.sphere().blocks(), generator.sphere().radius_m());
    let here = client.host("ride").body().position;
    let flat = flattest(&Land(&generator), here).point;
    // Up from where it took off, brought over the flat ground, the deck
    // laid under it, and down onto it.
    tick(client, &mut input, 1.5);
    let mut pose = client.here();
    pose.column = grid.column_of(grid.surface_point(flat.to_array()));
    pose.h = Some(((flat.length() - radius_m + OVER_M + LANDS_FROM_M) / BLOCK_M).round() as i16);
    client.go_to(&pose.text(grid))?;
    tick(client, &mut input, 1.0);
    let (middle, top_m) = decked(client, flat)?;
    let mut pose = client.here();
    pose.column = grid.column_of(middle);
    pose.h = Some(((top_m + LANDS_FROM_M) / BLOCK_M).round() as i16);
    client.go_to(&pose.text(grid))?;
    tick(client, &mut input, 0.0);
    client.command_json(r#"{"type":"set_mode","mode":"ride.horse"}"#);
    for _ in 0..900 {
        tick(client, &mut input, 0.0);
        if !client.host("ride").aloft() {
            break;
        }
    }
    if client.host("ride").aloft() {
        return Err("--deck: the pegasus did not land".into());
    }
    client.command_json(r#"{"type":"ride.leave"}"#);
    for event in client.drain_events() {
        if let client::Event::Rejected { message } = event {
            log::warn!("--deck: {message}");
        }
    }
    let (mut headless, _) = drawn(client, size)?;
    let mut listed = String::from("[\n");
    let mut off = None;
    let last = (AT_S[AT_S.len() - 1] * 60.0).round() as u32;
    let mut pegasus = drawn_at(client, number)?;
    for frame in 0..=(WATCHED_S * 60.0) as u32 {
        if frame > 0 {
            input.look = looking(client, Some(pegasus.at));
            tick(client, &mut input, 0.0);
            input.look = [0.0; 2];
            pegasus = drawn_at(client, number)?;
        }
        let over_m = pegasus.at.length() - radius_m - top_m;
        let point = grid.surface_point(pegasus.at.to_array());
        let ground_m = seat::ground(&generator, Seat::Sector(point.sector), point) * BLOCK_M;
        let over_ground_m = pegasus.at.length() - radius_m - ground_m;
        if off.is_none() && over_m < -OFF_M {
            off = Some(frame);
        }
        let coming_off = off.map(|off| frame - off);
        let at = AT_S.iter().any(|s| (s * 60.0).round() as u32 == frame);
        let falling = coming_off.is_some_and(|n| n <= (OFF_S * 60.0) as u32 && n % EVERY == 0);
        if at || falling {
            let ms = frame * 1000 / 60;
            let watch = drawn_into(client, &mut headless);
            let up = pegasus.at.normalize();
            let ahead = watch.camera.rotation * DVec3::NEG_Z;
            let ahead = (ahead - up * ahead.dot(up)).normalize_or(up.any_orthonormal_vector());
            let [away, high, round, look] = NEAR;
            let from = DQuat::from_axis_angle(up, round.to_radians()) * -ahead;
            let (near, seen) = (pegasus.at + from * away + up * high, pegasus.at + up * look);
            let pictures = [
                ("watch", headless.render(&watch)),
                ("near", picture(client, &mut headless, near, seen, up)),
            ];
            for (view, picture) in pictures {
                let out = dir.join(format!("{view}-{ms:06}.png"));
                write_png(&out, size[0], size[1], &picture)?;
            }
            log::info!(
                "{ms} ms: {over_m:.2} m over the deck's top, {over_ground_m:.2} m over the ground"
            );
            let _ = writeln!(
                listed,
                r#"  {{"ms":{ms},"over_m":{over_m:.3},"over_ground_m":{over_ground_m:.3}}},"#
            );
        }
        if frame >= last && coming_off.is_some_and(|n| n > (OFF_S * 60.0) as u32) {
            break;
        }
    }
    if off.is_none() {
        log::warn!("--deck: the pegasus never came off the deck");
    }
    let listed = listed.trim_end_matches(",\n").to_owned() + "\n]\n";
    let out = dir.join("deck.json");
    std::fs::write(&out, listed).map_err(|e| format!("{}: {e}", out.display()))?;
    log::info!("pegasus {number} left on a deck -> {}", dir.display());
    Ok(())
}

/// The pointer's motion that turns the body's eye [`DOWN_DEG`] down and,
/// given `at`, toward it from the body.
fn looking(client: &Client, at: Option<DVec3>) -> [f32; 2] {
    let here = client.here();
    let pitch = here.pitch_deg.unwrap_or(0.0) + DOWN_DEG;
    let up = DVec3::from(client.up());
    let body = up * client.generator().sphere().radius_m();
    let turn = at
        .and_then(|at| {
            let toward = at - body;
            let toward = toward - up * toward.dot(up);
            let bearing = topology::bearing_deg(up.to_array(), toward.to_array())?;
            Some((bearing - here.bearing_deg?).rem_euclid(360.0))
        })
        .map_or(0.0, |turn| if turn > 180.0 { turn - 360.0 } else { turn });
    [turn, pitch].map(|deg| (deg.to_radians() / client::LOOK_RAD_PER_PX) as f32)
}

/// The animal a number names, as drawn this frame.
fn drawn_at(client: &mut Client, number: u64) -> Result<Near, String> {
    let animals = client.host("ride").animals().to_vec();
    animals
        .into_iter()
        .find(|near| near.number == number)
        .ok_or_else(|| "--deck: the pegasus is not drawn".into())
}

/// Opens the volume over `at` and lays the deck on its plot: its slab
/// [`OVER`] cells over the highest ground under it, on pillars from under
/// the ground at their feet. The deck's middle, and how high its top is
/// over the datum, metres.
fn decked(client: &mut Client, at: DVec3) -> Result<(SurfacePoint, f64), String> {
    let generator = client.generator().clone();
    let point = generator.sphere().blocks().surface_point(at.to_array());
    let seat = Seat::Sector(point.sector);
    let [x, y] = seat::plot_of(point).map(|n| n << seat::PLOT_BITS);
    let ground = |dx: i32, dy: i32| {
        let column = SurfacePoint::new(point.sector, f64::from(x + dx), f64::from(y + dy));
        seat::ground(&generator, seat, column)
    };
    let highest = (FROM..=TO + 1)
        .flat_map(|dy| (FROM..=TO + 1).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| ground(dx, dy))
        .fold(f64::MIN, f64::max);
    let slab = highest.ceil() as i32 + OVER - 1;
    let posts: Vec<i32> = (FROM..TO).step_by(BAY).chain([TO]).collect();
    let mut gestures = vec![Gesture::Create {
        span: Span::between([x + FROM, y + FROM, slab], [x + TO, y + TO, slab]),
        paint: 1,
    }];
    for &dx in &posts {
        for &dy in &posts {
            let foot = ground(dx, dy).floor() as i32 - 1;
            gestures.push(Gesture::Create {
                span: Span::between([x + dx, y + dy, foot], [x + dx, y + dy, slab - 1]),
                paint: 4,
            });
        }
    }
    let mut host = client.host("ride");
    host.open(seat, point)
        .map_err(|e| format!("--deck: {e:?}"))?;
    host.apply(seat, &gestures)
        .map_err(|e| format!("--deck: {e:?}"))?;
    let middle = f64::from(FROM + TO + 1) / 2.0;
    let middle = SurfacePoint::new(point.sector, f64::from(x) + middle, f64::from(y) + middle);
    Ok((middle, f64::from(slab + 1) * BLOCK_M))
}
