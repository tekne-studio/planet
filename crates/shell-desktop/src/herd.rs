//! Pictures of a herd and what is built, with a fixed clock and seed: how a
//! herd keeping out of what is built is looked at. `bun run shot
//! --herd out/herd` lays a platform where the nearest free horse grazes and
//! writes a sheet of the place from above at an angle: before it is laid,
//! as it lands, and a while after, the herd gone round it.

use std::path::Path;

use client::{Client, Input};
use glam::DVec3;
use render::write_png;

use crate::animals::{looking, side_by_side};
use crate::shot::drawn;

/// How far the eye stands from the middle of the platform along the
/// ground, and how high over it, metres.
const AWAY_M: f64 = 30.0;
const OVER_M: f64 = 18.0;
/// How long the herds settle before anything is laid, seconds.
const SETTLE_S: f64 = 1.0;
/// When the pictures after the platform lands are taken, seconds after it.
const AFTER_S: [f64; 3] = [0.0, 4.0, 15.0];

/// Writes `herd.png` into `dir`: the place before a platform is laid where
/// a horse grazes, then as it lands and [`AFTER_S`] after, then wide enough
/// to hold the herd beside it.
pub fn run(client: &mut Client, size: [u32; 2], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut input = Input::default();
    step(client, &mut input, SETTLE_S);
    let here = DVec3::from(client.up()) * client.generator().sphere().radius_m();
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .filter(|near| !near.ridden)
        .min_by(|a, b| (a.at - here).length().total_cmp(&(b.at - here).length()))
        .copied()
        .ok_or("no horse near")?;
    let grid = client.generator().sphere().blocks();
    let (mut headless, _) = drawn(client, size)?;
    let at = horse.at;
    let mut pictures = vec![picture(client, &mut headless, at)];
    // The body goes to the horse, lays a platform there and steps off it,
    // to under the eye: the herd stays in view, and the body out of it.
    client.teleport(grid.surface_point(at.to_array()));
    step(client, &mut input, 1.0 / 60.0);
    for command in [
        r#"{"type":"build.take","tool":"create"}"#,
        r#"{"type":"build.platform","side":32}"#,
        r#"{"type":"build.lay","base":"deck"}"#,
    ] {
        client.command_json(command);
        step(client, &mut input, 0.5);
    }
    client.command_json(r#"{"type":"build.take","tool":null}"#);
    let under_eye = at + at.normalize().any_orthonormal_vector() * AWAY_M;
    client.teleport(grid.surface_point(under_eye.to_array()));
    let mut was_s = 0.0;
    for after_s in AFTER_S {
        step(client, &mut input, after_s - was_s);
        was_s = after_s;
        pictures.push(picture(client, &mut headless, at));
        let near: Vec<String> = client
            .host("ride")
            .animals()
            .iter()
            .map(|near| format!("{:.0}", (near.at - at).length()))
            .collect();
        log::info!("{after_s} s after: horses at {} m", near.join(", "));
    }
    // Wide, the platform and the herd beside it.
    let free: Vec<DVec3> = client
        .host("ride")
        .animals()
        .iter()
        .filter(|near| !near.ridden && (near.at - at).length() < 120.0)
        .map(|near| near.at)
        .collect();
    let herd = free.iter().sum::<DVec3>() / free.len().max(1) as f64;
    let middle = match free.is_empty() {
        true => at,
        false => (at + herd) / 2.0,
    };
    let wide = 1.0 + (herd - at).length() / AWAY_M;
    pictures.push(picture_from(client, &mut headless, middle, wide));
    let out = dir.join("herd.png");
    let sheet = side_by_side(&pictures, size);
    write_png(&out, size[0] * pictures.len() as u32, size[1], &sheet)?;
    log::info!("horse {} built over -> {}", horse.number, out.display());
    Ok(())
}

fn step(client: &mut Client, input: &mut Input, s: f64) {
    for _ in 0..(s * 60.0).round() as u32 {
        client.update(1.0 / 60.0, input);
    }
}

/// The place about `at` from above at an angle.
fn picture(client: &mut Client, headless: &mut render::Headless, at: DVec3) -> Vec<u8> {
    picture_from(client, headless, at, 1.0)
}

/// The same, `far` times as far off.
fn picture_from(
    client: &mut Client,
    headless: &mut render::Headless,
    at: DVec3,
    far: f64,
) -> Vec<u8> {
    let up = at.normalize();
    let across = up.any_orthonormal_vector();
    let from = at + (across * AWAY_M + up * OVER_M) * far;
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
