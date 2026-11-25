//! Pictures of a ridden horse's jump, with a fixed clock and seed: how a
//! jump is looked at. `bun run shot --leap out/leap` mounts the nearest
//! horse and, on the flattest ground near and on a slope of about 17
//! degrees facing up it, at a walk and at a gallop, holds the jump key for
//! a tap, for half the hold that reaches the highest jump and for all of
//! it, then lets go. On a floor laid over the flattest ground it trots and
//! gallops at a wall two cells high and jumps it, at a tap and at a full
//! hold, timed to top over the wall, or as near as its nose lets it. Every fourth frame from the press is drawn from
//! its side, the sun behind the eye; the eye goes along with it over the
//! ground and keeps the height it pressed at, so the arc is seen. Each
//! frame is `<run>/side-<ms>.png`, and `leap.json` lists every frame of 60
//! a second with how far along and how high over where it pressed the
//! body was, and how fast it went.

use std::fmt::Write as _;
use std::path::Path;

use client::{Client, GRAVITY_MPS2, Input, Key, Least, Near, Seat};
use gaia::fauna::horse::HORSE;
use gaia::fauna::{Carry, Ground};
use glam::DVec3;
use render::write_png;
use topology::{BLOCK_M, SurfacePoint};
use voxel::{Gesture, Span};

use crate::animals::{Land, flattest};
use crate::shot::drawn;
use crate::takeoff::{picture, sloped, tick};

/// How a horse carries a rider: its paces and its jump.
fn carry() -> Carry {
    HORSE.carry.expect("a horse carries a rider")
}

/// How long the jump key is held, seconds: a tap, then half and all of the
/// hold that reaches a horse's highest jump.
fn holds() -> [(&'static str, f64); 3] {
    let hold_s = carry().jump.hold_s;
    [("tap", 0.0), ("half", hold_s / 2.0), ("full", hold_s)]
}
/// The paces it jumps from, by how many presses of forward reach them, and
/// how long it goes at one before the press, seconds.
const PACES: [(&str, u32, f64); 2] = [("walk", 1, 1.5), ("gallop", 3, 3.0)];
/// The paces it jumps the wall from, by how many presses reach them and
/// which of its paces they are.
const AT_WALL: [(&str, u32, usize); 2] = [("trot", 2, 1), ("gallop", 3, 2)];
/// How long it is looked at after the key is let go, seconds.
const AFTER_S: f64 = 2.0;
/// A picture every this many frames of 60 a second.
const EVERY: u32 = 4;
/// From its side, how far away the eye stands, how high over the ground
/// where it pressed, and how high it looks at.
const SIDE: [f64; 3] = [5.5, 2.2, 1.1];
/// How long it stands before it sets off, seconds.
const SETTLE_S: f64 = 1.0;
/// The wall: how many cells high, how many cells from the west of its plot,
/// and from how many it sets off.
const WALL_CELLS: i32 = 2;
const WALL_AT: i32 = 50;
const START_AT: f64 = 2.5;

/// Writes the frames into `dir`.
pub fn run(client: &mut Client, size: [u32; 2], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut input = Input::default();
    crate::shot::ride(client, false)?;
    let generator = client.generator().clone();
    let land = Land(&generator);
    let here = client.host("ride").body().position;
    let flat = flattest(&land, here);
    let (slope, uphill) = sloped(&land, here);
    let grid = generator.sphere().blocks();
    let (mut headless, base) = drawn(client, size)?;
    let sun = base.sun_direction.as_dvec3();
    let mut listed = String::from("[\n");
    // How far it goes from where it sets off to the press, at each pace:
    // on the slope it sets off as far down it, so it jumps on the slope.
    let mut run_up_m = [0.0; PACES.len()];
    for (ground, footing, way) in [("flat", flat.point, None), ("slope", slope, Some(uphill))] {
        for (i, (pace, presses, going_s)) in PACES.into_iter().enumerate() {
            for (hold, hold_s) in holds() {
                let up = footing.normalize();
                let tangent =
                    |v: DVec3| (v - up * v.dot(up)).normalize_or(up.any_orthonormal_vector());
                // On the flat it faces across the sun, its side lit; on the
                // slope, up it.
                let facing = way.unwrap_or_else(|| up.cross(tangent(sun)));
                let start = match way {
                    Some(uphill) => land.under(footing - uphill * run_up_m[i]).point,
                    None => footing,
                };
                let point = grid.surface_point(start.to_array());
                stand(client, &mut input, point, None, up, facing)?;
                let from = client.host("ride").body().position;
                set_off(client, &mut input, presses, going_s);
                if way.is_none() {
                    run_up_m[i] = (client.host("ride").body().position - from).length();
                }
                let name = format!("{ground}-{pace}-{hold}");
                let jump = Jump {
                    name: &name,
                    hold_s,
                    press_at: None,
                };
                jump.take(
                    client,
                    &mut headless,
                    &mut input,
                    size,
                    sun,
                    dir,
                    &mut listed,
                )?;
                halt(client, &mut input);
            }
        }
    }
    // A wall two cells high across a floor laid over the flattest ground.
    let (start, floor, toward, wall_u) = walled(client, flat.point)?;
    let up = DVec3::from(grid.direction(start));
    for (pace, presses, which) in AT_WALL {
        let least = carry().paces[which]
            .jump
            .map(Least::from)
            .ok_or("--leap: a pace with no jump")?;
        for (hold, hold_s) in [holds()[0], holds()[2]] {
            stand(client, &mut input, start, Some(floor), up, toward)?;
            set_off(client, &mut input, presses, 0.0);
            let name = format!("wall-{pace}-{hold}");
            let jump = Jump {
                name: &name,
                hold_s,
                press_at: Some((wall_u, least)),
            };
            jump.take(
                client,
                &mut headless,
                &mut input,
                size,
                sun,
                dir,
                &mut listed,
            )?;
            halt(client, &mut input);
        }
    }
    let listed = listed.trim_end_matches(",\n").to_owned() + "\n]\n";
    let out = dir.join("leap.json");
    std::fs::write(&out, listed).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

/// One jump looked at: its name, how long the key is held, and, at a wall,
/// the column of the wall's near side it is timed against and the least
/// jump of the pace it jumps from.
struct Jump<'a> {
    name: &'a str,
    hold_s: f64,
    press_at: Option<(f64, Least)>,
}

impl Jump<'_> {
    #[allow(clippy::too_many_arguments)]
    fn take(
        &self,
        client: &mut Client,
        headless: &mut render::Headless,
        input: &mut Input,
        size: [u32; 2],
        sun: DVec3,
        dir: &Path,
        listed: &mut String,
    ) -> Result<(), String> {
        let held = (self.hold_s * 60.0).round().max(1.0) as u32;
        // At a wall it goes on at its pace until the press tops the arc
        // over the wall's middle, as a rider times it by eye.
        if let Some((wall_u, least)) = self.press_at {
            let ridden = ridden(client)?;
            for _ in 0..600 {
                let u = client.host("ride").feet().point.u;
                let lead_m = lead_m(self.hold_s, speed_mps(client), least, ridden);
                if u + lead_m / BLOCK_M >= wall_u + 0.5 {
                    break;
                }
                tick(client, input, 0.0);
            }
        }
        client.drain_events();
        let body = client.host("ride").body();
        let (pressed, up) = (body.position, body.up);
        let forward = (body.facing - up * body.facing.dot(up)).normalize();
        let right = forward.cross(up);
        let side = match right.dot(sun) >= 0.0 {
            true => right,
            false => -right,
        };
        let folder = dir.join(self.name);
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let frames = held + (AFTER_S * 60.0) as u32;
        let generator = client.generator().clone();
        let land = Land(&generator);
        input.key(Key::Up, true);
        for frame in 0..=frames {
            if frame == held {
                input.key(Key::Up, false);
            }
            let at = client.host("ride").body().position;
            let along_m = (at - pressed).dot(forward);
            let over_m = (at - pressed).dot(up);
            let ground_m = (land.under(at).point - pressed).dot(up);
            let ms = frame * 1000 / 60;
            if frame % EVERY == 0 {
                // Along with it over the ground, at the height it pressed at.
                let along = at - up * (at - pressed).dot(up);
                let [away, eye, look] = SIDE;
                let target = along + up * look;
                let eye = along + side * away + up * eye;
                let drawn = picture(client, headless, eye, target, up);
                let out = folder.join(format!("side-{ms:04}.png"));
                write_png(&out, size[0], size[1], &drawn)?;
            }
            let _ = writeln!(
                listed,
                r#"  {{"run":"{}","hold_s":{:.3},"ms":{ms},"along_m":{along_m:.3},"over_m":{over_m:.3},"ground_m":{ground_m:.3},"speed_mps":{:.3},"held":{}}},"#,
                self.name,
                self.hold_s,
                speed_mps(client),
                frame < held
            );
            tick(client, input, 0.0);
        }
        input.key(Key::Up, false);
        for event in client.drain_events() {
            if let client::Event::Rejected { message } = event {
                log::warn!("{}: {message}", self.name);
            }
        }
        log::info!("{} -> {}", self.name, folder.display());
        Ok(())
    }
}

/// How far ahead of where the key is pressed, going at `speed_mps`, the
/// arc of a jump held `hold_s` from a pace whose least is `least` tops,
/// metres, for the horse `ridden`: carried at its pace until it is let go
/// and gathered for that least, then half its arc on. A short arc tops
/// over a wall's middle no nearer than its nose a cell short of the wall
/// as it leaves the ground, wider than the half of its body a wall stops
/// and a frame's going, so it jumps and meets the wall in the air.
fn lead_m(hold_s: f64, speed_mps: f64, least: Least, ridden: Near) -> f64 {
    let jump = client::Jump::from(carry().jump).scaled(ridden.speed_share);
    let arc = jump.arc(jump.height_m(least, hold_s), speed_mps, GRAVITY_MPS2);
    let [nose_m, _] = HORSE.reach_m().map(|m| m * ridden.size);
    let shortest_m = nose_m + BLOCK_M + BLOCK_M / 2.0;
    speed_mps * hold_s.max(least.gather_s) + (arc.length_m() / 2.0).max(shortest_m)
}

/// The horse the body rides.
fn ridden(client: &mut Client) -> Result<Near, String> {
    client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| near.ridden)
        .ok_or_else(|| "--leap: no horse ridden".to_string())
}

/// How fast the body goes along the ground, metres a second.
fn speed_mps(client: &mut Client) -> f64 {
    client
        .host("ride")
        .in_ways()
        .iter()
        .find(|way| way.session.is_none())
        .map_or(0.0, |way| way.speed_mps)
}

/// Stands the body at `point` facing `facing`, still: on the ground, or
/// on a floor whose top is `floor` cells over the datum.
fn stand(
    client: &mut Client,
    input: &mut Input,
    point: SurfacePoint,
    floor: Option<i32>,
    up: DVec3,
    facing: DVec3,
) -> Result<(), String> {
    let grid = client.generator().sphere().blocks();
    client.teleport(point);
    let mut pose = client.here();
    pose.h = floor.and_then(|floor| i16::try_from(floor).ok());
    pose.bearing_deg = topology::bearing_deg(up.to_array(), facing.to_array());
    pose.pitch_deg = Some(0.0);
    client.go_to(&pose.text(grid))?;
    tick(client, input, SETTLE_S);
    Ok(())
}

/// Forward pressed `presses` times, a pace each, then gone at for
/// `going_s` seconds.
fn set_off(client: &mut Client, input: &mut Input, presses: u32, going_s: f64) {
    for _ in 0..presses {
        input.key(Key::Forward, true);
        tick(client, input, 0.0);
        input.key(Key::Forward, false);
        tick(client, input, 0.0);
    }
    tick(client, input, going_s);
}

/// Back pressed until it halts, then stood a moment.
fn halt(client: &mut Client, input: &mut Input) {
    for _ in 0..5 {
        input.key(Key::Back, true);
        tick(client, input, 0.0);
        input.key(Key::Back, false);
        tick(client, input, 0.0);
    }
    tick(client, input, SETTLE_S);
}

/// Opens the volume over `at`, lays a floor one cell thick across its plot
/// over the highest ground under it and a wall [`WALL_CELLS`] high across
/// it, [`WALL_AT`] cells from its west side. Returns where to set off, at
/// the floor's west, how high its top is in cells, which way the wall is,
/// and the column of the wall's near side.
fn walled(client: &mut Client, at: DVec3) -> Result<(SurfacePoint, i32, DVec3, f64), String> {
    let generator = client.generator().clone();
    let grid = generator.sphere().blocks();
    let point = grid.surface_point(at.to_array());
    let seat = Seat::Sector(point.sector);
    let [x, y] = seat::plot_of(point).map(|n| n << seat::PLOT_BITS);
    let side = 1 << seat::PLOT_BITS;
    let highest = (0..=side)
        .flat_map(|dy| (0..=side).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| {
            let column = SurfacePoint::new(point.sector, f64::from(x + dx), f64::from(y + dy));
            seat::ground(&generator, seat, column)
        })
        .fold(f64::MIN, f64::max);
    let top = highest.ceil() as i32;
    let far = side - 1;
    let floor = Span::between([x, y, top - 1], [x + far, y + far, top - 1]);
    let wall = Span::between(
        [x + WALL_AT, y, top],
        [x + WALL_AT, y + far, top + WALL_CELLS - 1],
    );
    let mut host = client.host("ride");
    host.open(seat, point)
        .map_err(|e| format!("--leap: {e:?}"))?;
    host.apply(
        seat,
        &[
            Gesture::Create {
                span: floor,
                paint: 0,
            },
            Gesture::Create {
                span: wall,
                paint: 4,
            },
        ],
    )
    .map_err(|e| format!("--leap: {e:?}"))?;
    let v = f64::from(y) + f64::from(side) / 2.0;
    let start = SurfacePoint::new(point.sector, f64::from(x) + START_AT, v);
    let east = SurfacePoint::new(point.sector, f64::from(x) + START_AT + 1.0, v);
    let toward = DVec3::from(grid.direction(east)) - DVec3::from(grid.direction(start));
    let up = DVec3::from(grid.direction(start));
    let toward = (toward - up * toward.dot(up)).normalize();
    Ok((start, top, toward, f64::from(x + WALL_AT)))
}
