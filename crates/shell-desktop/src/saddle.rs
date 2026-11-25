//! Pictures of a rider in the saddle, with a fixed clock and seed: how a
//! seat is looked at. `bun run shot --saddle out/saddle` mounts, of the
//! horses near the shot, the smallest, one of the middle size and the
//! largest in turn and writes, for each, a sheet of the rider seen from
//! its side, from behind, from its front quarter, from above and from the
//! front, standing still, and then at each pace; then one of a pegasus
//! aloft. `--avatar` worn; `--command '{"type":"flashlight.on"}'` shows
//! how a thing in the hand is held astride.

use std::path::Path;

use client::{Client, Input, Key};
use glam::DVec3;
use render::write_png;

use crate::animals::{looking, side_by_side};
use crate::shot::drawn;

/// How far from the rider the eye stands, metres, and how high over its
/// feet it looks from and at.
const AWAY_M: f64 = 3.2;
const EYE_M: f64 = 2.6;
const AT_M: f64 = 1.6;
/// From above: how far behind the rider the eye stands, and how high
/// over its feet, metres.
const ABOVE: (f64, f64) = (1.2, 4.0);
/// How long it settles standing, and goes at a pace, before its picture,
/// seconds.
const SETTLE_S: f64 = 1.0;
const PACE_S: f64 = 2.5;

/// Writes `saddle-<n>.png` into `dir` for each horse mounted, smallest
/// first: standing, seen from the side, from behind, from the front
/// quarter, from above and from the front, then the side at a walk, a trot
/// and a gallop. Then
/// `saddle-aloft.png`: a pegasus flown, from its side and from behind.
pub fn run(client: &mut Client, size: [u32; 2], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut input = Input::default();
    // The herds take their places at the clock set over a few frames.
    step(client, &mut input, SETTLE_S);
    let here = DVec3::from(client.up()) * client.generator().sphere().radius_m();
    let mut near: Vec<client::Near> = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .filter(|near| !near.ridden)
        .collect();
    near.sort_by(|a, b| (a.at - here).length().total_cmp(&(b.at - here).length()));
    log::info!("{} horses near", near.len());
    // Of the nearest, the smallest, the one nearest the middle size and the
    // largest: a horse's speed under a rider is its size's.
    near.truncate(crate::shot::TRIED);
    near.sort_by(|a, b| a.speed_share.total_cmp(&b.speed_share));
    let middle = near.iter().min_by(|a, b| {
        (a.speed_share - 1.0)
            .abs()
            .total_cmp(&(b.speed_share - 1.0).abs())
    });
    let mut builds: Vec<client::Near> = [near.first(), middle, near.last()]
        .into_iter()
        .flatten()
        .copied()
        .collect();
    builds.dedup_by_key(|near| near.number);
    let grid = client.generator().sphere().blocks();
    let (mut headless, _) = drawn(client, size)?;
    for (n, animal) in builds.iter().enumerate() {
        // Where it is now: a herd goes on while another is ridden.
        let at = (client.host("ride").animals().iter())
            .find(|near| near.number == animal.number)
            .map_or(animal.at, |near| near.at);
        client.teleport(grid.surface_point(at.to_array()));
        step(client, &mut input, 1.0 / 60.0);
        client.drain_events();
        crate::shot::mount(client, animal.number, &mut input)?;
        step(client, &mut input, SETTLE_S);
        let mut pictures = Vec::new();
        for (side, behind) in [(1.0, 0.0), (0.0, 1.0), (-0.7, -0.7)] {
            pictures.push(picture(client, &mut headless, side, behind));
        }
        let (behind_m, over_m) = ABOVE;
        pictures.push(picture_from(
            client,
            &mut headless,
            [0.0, 1.0],
            behind_m,
            over_m,
        ));
        pictures.push(picture(client, &mut headless, 0.0, -1.0));
        // Walk, trot, then gallop, sprint held.
        input.key(Key::Forward, true);
        for pace in 0..3 {
            if pace == 2 {
                input.key(Key::Sprint, true);
            }
            step(client, &mut input, PACE_S);
            pictures.push(picture(client, &mut headless, 1.0, 0.0));
            if pace == 0 {
                // Forward pressed again steps it up a pace.
                input.key(Key::Forward, false);
                step(client, &mut input, 1.0 / 60.0);
                input.key(Key::Forward, true);
            }
        }
        input.key(Key::Forward, false);
        input.key(Key::Sprint, false);
        client.command_json(r#"{"type":"ride.leave"}"#);
        step(client, &mut input, 0.5);
        client.drain_events();
        let out = dir.join(format!("saddle-{n}.png"));
        let sheet = side_by_side(&pictures, size);
        write_png(&out, size[0] * pictures.len() as u32, size[1], &sheet)?;
        log::info!(
            "horse {} at {:.2} of the paces -> {}",
            animal.number,
            animal.speed_share,
            out.display()
        );
    }

    // A pegasus aloft, from where the shot began: taken off, flown on, and
    // looked at.
    client.teleport(grid.surface_point(here.to_array()));
    step(client, &mut input, 1.0 / 60.0);
    crate::shot::ride(client, true)?;
    input.key(Key::Forward, true);
    step(client, &mut input, PACE_S);
    let pictures = vec![
        picture(client, &mut headless, 1.0, 0.0),
        picture(client, &mut headless, 0.0, 1.0),
    ];
    input.key(Key::Forward, false);
    let out = dir.join("saddle-aloft.png");
    let sheet = side_by_side(&pictures, size);
    write_png(&out, size[0] * pictures.len() as u32, size[1], &sheet)?;
    log::info!("aloft -> {}", out.display());
    Ok(())
}

fn step(client: &mut Client, input: &mut Input, s: f64) {
    for _ in 0..(s * 60.0).round().max(1.0) as u32 {
        client.update(1.0 / 60.0, input);
    }
}

/// The rider seen from `side` along its right and `behind` along its back.
fn picture(
    client: &mut Client,
    headless: &mut render::Headless,
    side: f64,
    behind: f64,
) -> Vec<u8> {
    picture_from(client, headless, [side, behind], AWAY_M, EYE_M)
}

/// The rider seen from `away_m` off along `[side, behind]` and `eye_m`
/// over its feet.
fn picture_from(
    client: &mut Client,
    headless: &mut render::Headless,
    [side, behind]: [f64; 2],
    away_m: f64,
    eye_m: f64,
) -> Vec<u8> {
    let body = client.host("ride").body();
    let (up, back) = (body.up, -body.facing);
    let right = up.cross(back);
    let at = body.position + up * AT_M;
    let from = at + (right * side + back * behind).normalize() * away_m + up * (eye_m - AT_M);
    let mut frame = client.settled_frame();
    let renderer = &mut headless.renderer;
    renderer.apply(client.drain_terrain_changes());
    renderer.apply_volumes(client.drain_volume_changes());
    renderer.apply_skinned(client.drain_skinned_changes());
    renderer.apply_pictures(client.drain_picture_changes());
    renderer.apply_models(client.drain_model_changes());
    frame.camera.position = from;
    frame.camera.rotation = looking(at - from, up);
    headless.render(&frame)
}
