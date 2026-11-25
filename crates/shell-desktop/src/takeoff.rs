//! Pictures of a pegasus taking off under its rider, and landing, with a
//! fixed clock and seed: how a take-off is looked at. `bun run shot
//! --takeoff out/takeoff` mounts the nearest pegasus and, on the flattest
//! ground near and on a slope of about 17 degrees facing up it, from a
//! standing start and from a trot, presses the walk and fly key as a
//! person does, holds forward from then on, and draws frames from the
//! press until it flies level: every other one to the top of its spring,
//! every one as its wings open, then every sixth. Then it lands it and
//! draws every frame as its wings fold away, from the last aloft. Each is
//! seen from its side, the sun behind the eye, from behind its rider and
//! from over it. The eye goes along with it over the ground; from its side
//! it keeps its height, so the climb is seen, and from behind and over it
//! it rises with it, so the wings are. Each frame is
//! `<ground>-<start>/<view>-<ms>.png` taking off and
//! `<ground>-<start>/fold-<view>-<ms>.png` landing, and `takeoff.json`
//! lists them with how high the body was over where it pressed, or over
//! the ground under it.

use std::fmt::Write as _;
use std::path::Path;

use client::{Client, Input, Key};
use gaia::fauna::{Ground, horse::HORSE};
use glam::DVec3;
use render::write_png;

use crate::animals::{Land, flattest, looking};
use crate::shot::drawn;

/// Frames a second.
const FPS: f64 = 60.0;
/// The last frame drawn taking off, counted from the press: 1.8 s.
const LAST: u32 = 108;
/// The frames drawn taking off, counted at 60 a second from the press:
/// every other one to the top of the spring, `top_s` from the press,
/// every one as the wings open over `open_s` and the one after, then
/// every sixth.
fn frames(top_s: f64, open_s: f64) -> impl Iterator<Item = u32> {
    let top = (top_s * FPS).round() as u32;
    let open = top + (open_s * FPS).round() as u32 + 1;
    (0..top)
        .step_by(2)
        .chain(top..=open)
        .chain((open + 1..=LAST).filter(|frame| frame % 6 == 0))
}
/// The slope looked for, rise per metre: about 17 degrees.
const SLOPE: f64 = 0.306;
/// How far around the pegasus a slope is looked for, metres, in how many
/// steps each way, and over how long a run its rise is read.
const LOOK_M: f64 = 400.0;
const LOOKS: i32 = 20;
const RUN_M: f64 = 8.0;
/// From its side, how far away the eye stands, how high over the ground
/// where it pressed, and how high it looks at; from behind and from over
/// it, the same.
const SIDE: [f64; 3] = [5.5, 1.4, 1.6];
const BEHIND: [f64; 3] = [5.0, 2.4, 1.6];
const ABOVE: [f64; 3] = [1.5, 7.0, 1.6];
/// How long it stands, and trots, before the press, seconds.
const SETTLE_S: f64 = 1.0;
const TROT_S: f64 = 2.5;
/// Landing, how near the ground it is drawn from, metres: the last of
/// these aloft is the first frame of the fold.
const NEAR_M: f64 = 1.0;

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
    let winged = HORSE.wings.ok_or("a horse has no wings")?;
    let spring = winged.flight.spring;
    let (top_s, open_s) = (spring.gather_s + spring.rise_s, winged.wing.open_s);
    let folds = (winged.wing.fold_s * FPS).round() as u32 + 1;
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
            let views = [
                ("side", SIDE, side),
                ("behind", BEHIND, -forward),
                ("above", ABOVE, side),
            ];
            // Over it, the eye looks down with the way it goes up.
            let eye_up = |view: &str| match view {
                "above" => forward,
                _ => up,
            };
            let mut ticked = 0;
            for frame in frames(top_s, open_s) {
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
                for (view, [away, eye, look], from) in views {
                    let along = match view {
                        "side" => along,
                        _ => at,
                    };
                    let target = along + up * look;
                    let eye = along + from * away + up * eye;
                    let picture = picture(client, &mut headless, eye, target, eye_up(view));
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
            // Landed again for the next, its wings drawn folding away from
            // the last frame aloft, the eye along with it over the ground
            // under it.
            client.command_json(r#"{"type":"set_mode","mode":"ride.horse"}"#);
            let seen = |client: &mut Client, headless: &mut render::Headless| {
                let at = client.host("ride").body().position;
                let under = land.under(at).point;
                let pictures = views.map(|(view, [away, eye, look], from)| {
                    let target = under + up * look;
                    let eye = under + from * away + up * eye;
                    picture(client, headless, eye, target, eye_up(view))
                });
                ((at - under).dot(up), pictures)
            };
            let mut last = None;
            for _ in 0..900 {
                tick(client, &mut input, 0.0);
                if !client.host("ride").aloft() {
                    break;
                }
                let at = client.host("ride").body().position;
                if (at - land.under(at).point).dot(up) < NEAR_M {
                    last = Some(seen(client, &mut headless));
                }
            }
            for frame in 0..=folds {
                let (over_m, pictures) = match (frame, last.take()) {
                    (0, Some(last)) => last,
                    _ => {
                        if frame > 0 {
                            tick(client, &mut input, 0.0);
                        }
                        seen(client, &mut headless)
                    }
                };
                let ms = frame * 1000 / 60;
                for ((view, ..), picture) in views.iter().zip(pictures) {
                    let out = folder.join(format!("fold-{view}-{ms:04}.png"));
                    write_png(&out, size[0], size[1], &picture)?;
                }
                let _ = writeln!(
                    listed,
                    r#"  {{"ground":"{ground}","start":"{start}","fold_ms":{ms},"over_m":{over_m:.3}}},"#
                );
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
pub(crate) fn tick(client: &mut Client, input: &mut Input, s: f64) {
    for _ in 0..(s * 60.0).round().max(1.0) as u32 {
        client.update(1.0 / 60.0, input);
    }
}

/// The ground near `at` whose rise over [`RUN_M`] is nearest [`SLOPE`],
/// above the sea, and the way up it along the ground.
pub(crate) fn sloped(land: &Land, at: DVec3) -> (DVec3, DVec3) {
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
pub(crate) fn picture(
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
