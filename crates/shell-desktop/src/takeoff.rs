//! Pictures of a pegasus taking off under its rider, with a fixed clock and
//! seed: how a take-off is looked at. `bun run shot --takeoff out/takeoff`
//! mounts the nearest pegasus and, on the flattest ground near and on a
//! slope of about 17 degrees facing up it, from a standing start and from
//! a trot, presses the walk and fly key as a person does, holds forward
//! from then on, and draws every other frame from the press until it flies
//! level: from its side, the sun behind the eye, and from behind its
//! rider. The eye goes along with it over the ground and keeps its height,
//! so the climb is seen. Each frame is `<ground>-<start>/<view>-<ms>.png`,
//! and `takeoff.json` lists them with how high the body was over where it
//! pressed.

use std::fmt::Write as _;
use std::path::Path;

use client::{Client, Input, Key};
use gaia::fauna::Ground;
use glam::DVec3;
use render::write_png;

use crate::animals::{Land, flattest, looking};
use crate::shot::drawn;

/// The frames drawn, counted at 60 a second from the press: every other
/// one through the first second, then every sixth to 1.8 s.
fn frames() -> impl Iterator<Item = u32> {
    (0..=60).step_by(2).chain((66..=108).step_by(6))
}
/// The slope looked for, rise per metre: about 17 degrees.
const SLOPE: f64 = 0.306;
/// How far around the pegasus a slope is looked for, metres, in how many
/// steps each way, and over how long a run its rise is read.
const LOOK_M: f64 = 400.0;
const LOOKS: i32 = 20;
const RUN_M: f64 = 8.0;
/// From its side, how far away the eye stands, how high over the ground
/// where it pressed, and how high it looks at; from behind, the same.
const SIDE: [f64; 3] = [5.5, 1.4, 1.6];
const BEHIND: [f64; 3] = [5.0, 2.4, 1.6];
/// How long it stands, and trots, before the press, seconds.
const SETTLE_S: f64 = 1.0;
const TROT_S: f64 = 2.5;

/// Writes the frames into `dir`.
pub fn run(client: &mut Client, size: [u32; 2], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut input = Input::default();
    // A pegasus, taken off by the walk and fly command, then landed.
    crate::shot::ride(client, true)?;
    client.command_json(r#"{"type":"set_mode","mode":"ride.horse"}"#);
    tick(client, &mut input, 0.0);
    for _ in 0..600 {
        if !client.host("ride").aloft() {
            break;
        }
        tick(client, &mut input, 0.0);
    }
    if client.host("ride").aloft() {
        return Err("--takeoff: the pegasus did not land".into());
    }
    let generator = client.generator().clone();
    let land = Land(&generator);
    let here = client.host("ride").body().position;
    let flat = flattest(&land, here);
    let (slope, uphill) = sloped(&land, here);
    let grid = generator.sphere().blocks();
    let (mut headless, base) = drawn(client, size)?;
    let sun = base.sun_direction.as_dvec3();
    let mut listed = String::from("[\n");
    for (ground, footing, way) in [("flat", flat.point, None), ("slope", slope, Some(uphill))] {
        for start in ["stand", "trot"] {
            let up = footing.normalize();
            let tangent = |v: DVec3| (v - up * v.dot(up)).normalize_or(up.any_orthonormal_vector());
            // On the flat it faces across the sun, its side lit; on the
            // slope, up it.
            let facing = way.unwrap_or_else(|| up.cross(tangent(sun)));
            client.teleport(grid.surface_point(footing.to_array()));
            let mut pose = client.here();
            pose.h = None;
            pose.bearing_deg = topology::bearing_deg(up.to_array(), facing.to_array());
            pose.pitch_deg = Some(0.0);
            client.go_to(&pose.text(grid))?;
            tick(client, &mut input, SETTLE_S);
            if start == "trot" {
                // Forward pressed twice: the walk, then the trot.
                for _ in 0..2 {
                    input.key(Key::Forward, true);
                    tick(client, &mut input, 0.0);
                    input.key(Key::Forward, false);
                    tick(client, &mut input, 0.0);
                }
                tick(client, &mut input, TROT_S);
            }
            client.drain_events();
            let body = client.host("ride").body();
            let pressed = body.position;
            let forward = tangent(body.facing);
            let right = forward.cross(up);
            // The eye on the sun's side.
            let side = match right.dot(sun) >= 0.0 {
                true => right,
                false => -right,
            };
            let folder = dir.join(format!("{ground}-{start}"));
            std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            input.key(Key::ToggleMode, true);
            input.key(Key::Forward, true);
            let mut ticked = 0;
            for frame in frames() {
                while ticked <= frame {
                    tick(client, &mut input, 0.0);
                    input.key(Key::ToggleMode, false);
                    ticked += 1;
                }
                let at = client.host("ride").body().position;
                // Along with it over the ground, at the height it pressed at.
                let along = at - up * (at - pressed).dot(up);
                let over_m = (at - pressed).dot(up);
                let ms = frame * 1000 / 60;
                for (view, [away, eye, look], from) in
                    [("side", SIDE, side), ("behind", BEHIND, -forward)]
                {
                    let target = along + up * look;
                    let eye = along + from * away + up * eye;
                    let picture = picture(client, &mut headless, eye, target, up);
                    let out = folder.join(format!("{view}-{ms:04}.png"));
                    write_png(&out, size[0], size[1], &picture)?;
                }
                let _ = writeln!(
                    listed,
                    r#"  {{"ground":"{ground}","start":"{start}","ms":{ms},"over_m":{over_m:.3}}},"#
                );
            }
            input.key(Key::Forward, false);
            for event in client.drain_events() {
                if let client::Event::Rejected { message } = event {
                    log::warn!("{ground} {start}: {message}");
                }
            }
            // Landed again for the next.
            client.command_json(r#"{"type":"set_mode","mode":"ride.horse"}"#);
            for _ in 0..900 {
                tick(client, &mut input, 0.0);
                if !client.host("ride").aloft() {
                    break;
                }
            }
            tick(client, &mut input, SETTLE_S);
            log::info!("{ground} {start} -> {}", folder.display());
        }
    }
    let listed = listed.trim_end_matches(",\n").to_owned() + "\n]\n";
    let out = dir.join("takeoff.json");
    std::fs::write(&out, listed).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

/// One frame of 60 a second, or as many as `s` seconds hold.
fn tick(client: &mut Client, input: &mut Input, s: f64) {
    for _ in 0..(s * 60.0).round().max(1.0) as u32 {
        client.update(1.0 / 60.0, input);
    }
}

/// The ground near `at` whose rise over [`RUN_M`] is nearest [`SLOPE`],
/// above the sea, and the way up it along the ground.
fn sloped(land: &Land, at: DVec3) -> (DVec3, DVec3) {
    let centre = land.under(at);
    let up = centre.up;
    let east = up.any_orthonormal_vector();
    let north = up.cross(east);
    let step = LOOK_M / f64::from(LOOKS);
    let radius_m = land.0.sphere().radius_m();
    let steepest = |point: DVec3| {
        (0..16)
            .map(|k| {
                let turn = f64::from(k) * core::f64::consts::TAU / 16.0;
                let way = east * turn.cos() + north * turn.sin();
                let ahead = land.under(point + way * RUN_M / 2.0).point;
                let behind = land.under(point - way * RUN_M / 2.0).point;
                ((ahead - behind).dot(up) / RUN_M, way)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap_or((0.0, east))
    };
    let (point, (rise, way)) = (-LOOKS..=LOOKS)
        .flat_map(|i| (-LOOKS..=LOOKS).map(move |j| (i, j)))
        .map(|(i, j)| {
            land.under(centre.point + (east * f64::from(i) + north * f64::from(j)) * step)
        })
        .filter(|footing| footing.point.length() > radius_m + 1.0)
        .map(|footing| (footing.point, steepest(footing.point)))
        .min_by(|a, b| (a.1.0 - SLOPE).abs().total_cmp(&(b.1.0 - SLOPE).abs()))
        .unwrap_or((centre.point, (0.0, east)));
    log::info!(
        "a slope of {:.1} degrees, {:.0} m away",
        rise.atan().to_degrees(),
        (point - centre.point).length()
    );
    (point, way)
}

/// The world seen from `eye` looking at `at`, `up` up.
fn picture(
    client: &mut Client,
    headless: &mut render::Headless,
    eye: DVec3,
    at: DVec3,
    up: DVec3,
) -> Vec<u8> {
    let mut frame = client.settled_frame();
    let renderer = &mut headless.renderer;
    renderer.apply(client.drain_terrain_changes());
    renderer.apply_volumes(client.drain_volume_changes());
    renderer.apply_skinned(client.drain_skinned_changes());
    renderer.apply_pictures(client.drain_picture_changes());
    renderer.apply_models(client.drain_model_changes());
    frame.camera.position = eye;
    frame.camera.rotation = looking(at - eye, up);
    headless.render(&frame)
}
