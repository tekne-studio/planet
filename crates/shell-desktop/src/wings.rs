//! Pictures of a horse's wings, with a fixed clock and seed: how a wing
//! is looked at. `bun run shot --wings out/wings` writes, for the look
//! `--look` born with wings in each coat a pegasus wears, a sheet of each: the
//! wings growing out as it leaves the ground, a beat in eight seen from
//! its side, from over it and from under it, a glide, and a landing as
//! they fold away. `--yaw`, `--against-sun` and `--clock` aim the eye as
//! for `--animals`.

use std::path::Path;

use client::Client;
use gaia::fauna::{Animal, Going, Ground, Look, horse::HORSE};
use glam::{DQuat, DVec3};
use render::write_png;

use crate::animals::{Land, View, flattest, looking, side_by_side};

/// Pictures in a sheet.
const STEPS: usize = 8;
/// The steps it is stepped in between pictures, seconds.
const TICK_S: f64 = 1.0 / 240.0;
/// How far from the animal the eye stands, metres: far enough for a span
/// of half again its length.
const AWAY_M: f64 = 6.0;
/// From over and from under it, how far, and how far to its side.
const OVER_M: f64 = 6.0;
const ASIDE_M: f64 = 2.0;
/// Taking off: how high it climbs, metres, over how long, seconds.
const RISE_M: f64 = 3.0;
const RISE_S: f64 = 1.4;
/// Flying: how fast, metres a second, how fast it climbs as it beats, and
/// how high it flies, metres.
const FLY_MPS: f64 = 18.0;
const CLIMB_MPS: f64 = 2.0;
const HIGH_M: f64 = 12.0;
/// Gliding: how fast, and how fast it sinks.
const GLIDE_MPS: f64 = 30.0;
const SINK_MPS: f64 = 1.5;
/// Landing: how fast it comes in, from how high, and how fast it sinks.
const LAND_MPS: f64 = 9.0;
const LAND_FROM_M: f64 = 2.5;
const LAND_SINK_MPS: f64 = 1.8;
/// How long it goes before its sheet starts, seconds: settled in.
const SETTLE_S: f64 = 2.0;

/// Where an animal is at a time of a sheet, how fast, and whether its
/// wings are out: `at(t)` is its height over the ground and its way along.
struct Flight<'a> {
    name: &'a str,
    speed_mps: f64,
    /// Over the ground and along the way, metres, at a time.
    at: &'a dyn Fn(f64) -> (f64, f64),
    /// Whether its wings are out at a time.
    out: &'a dyn Fn(f64) -> bool,
    /// When the pictures start and how long they cover, seconds.
    from_s: f64,
    span_s: f64,
    /// How far the eye is raised over its place, metres: near the ground,
    /// over the grass.
    raised_m: f64,
}

/// How far over the grass a flight near the ground is seen from, metres.
const OVER_GRASS_M: f64 = 1.5;

/// Writes the wing sheets into `dir`, on the flattest ground near where
/// the shot stands.
pub fn run(client: &mut Client, size: [u32; 2], view: &View, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let generator = client.generator().clone();
    let land = Land(&generator);
    let here = flattest(
        &land,
        DVec3::from(client.up()) * generator.sphere().radius_m(),
    );
    client.teleport(
        generator
            .sphere()
            .blocks()
            .surface_point(here.point.to_array()),
    );
    let (mut headless, base) = crate::shot::drawn(client, size)?;
    let up = here.up;
    let sun = base.sun_direction.as_dvec3();
    log::info!(
        "the sun {:.1}° over the horizon",
        sun.dot(up).asin().to_degrees()
    );
    let flat = |v: DVec3| (v - up * v.dot(up)).normalize_or(up.any_orthonormal_vector());
    let side = match view.against_sun {
        true => flat(-sun),
        false => flat(DVec3::X),
    };
    let heading = up.cross(side);
    let toward_eye = DQuat::from_axis_angle(up, -view.yaw_deg.to_radians()) * side;
    let eyes = [
        ("", toward_eye * AWAY_M),
        ("-above", up * OVER_M + toward_eye * ASIDE_M),
        ("-below", -up * OVER_M + toward_eye * ASIDE_M),
    ];

    let mut coats: Vec<&str> = HORSE
        .wings
        .ok_or("a horse has no wings")?
        .herds
        .iter()
        .flat_map(|set| set.iter().copied())
        .collect();
    coats.sort_unstable();
    coats.dedup();

    let rise = |t: f64| {
        let s = ((t - SETTLE_S) / RISE_S).clamp(0.0, 1.0);
        (RISE_M * s * s * (3.0 - 2.0 * s), 0.0)
    };
    let beat = |t: f64| (HIGH_M + CLIMB_MPS * t, FLY_MPS * t);
    let glide = |t: f64| (HIGH_M + 20.0 - SINK_MPS * t, GLIDE_MPS * t);
    let lands_s = SETTLE_S + LAND_FROM_M / LAND_SINK_MPS;
    let landing = |t: f64| {
        (
            (LAND_FROM_M - LAND_SINK_MPS * t.min(lands_s)).max(0.0),
            LAND_MPS * t,
        )
    };
    let always = |_: f64| true;
    let off_the_ground = |t: f64| t >= SETTLE_S;
    let until_down = |t: f64| t < lands_s;
    // One beat at the speed it flies, as the wing beats it at labour.
    let (slow_hz, fast_hz) = HORSE.wings.map(|w| w.wing.beat_hz).unwrap_or((1.0, 1.0));
    let beat_s = 2.0 / (slow_hz + fast_hz);
    let flights = [
        Flight {
            name: "wings-grow",
            speed_mps: 0.0,
            at: &rise,
            out: &off_the_ground,
            from_s: SETTLE_S - 0.1,
            span_s: RISE_S,
            raised_m: OVER_GRASS_M,
        },
        Flight {
            name: "wings-beat",
            speed_mps: FLY_MPS,
            at: &beat,
            out: &always,
            from_s: SETTLE_S,
            span_s: beat_s,
            raised_m: 0.0,
        },
        Flight {
            name: "wings-glide",
            speed_mps: GLIDE_MPS,
            at: &glide,
            out: &always,
            from_s: SETTLE_S,
            span_s: 1.0,
            raised_m: 0.0,
        },
        Flight {
            name: "wings-land",
            speed_mps: LAND_MPS,
            at: &landing,
            out: &until_down,
            from_s: lands_s - 0.6,
            span_s: 2.0,
            raised_m: OVER_GRASS_M,
        },
    ];
    for (coat, flight) in coats
        .iter()
        .flat_map(|&coat| flights.iter().map(move |flight| (coat, flight)))
    {
        // The beat is seen from three sides, the rest from the first.
        let seen = if flight.name == "wings-beat" {
            &eyes[..]
        } else {
            &eyes[..1]
        };
        for (from_name, from) in seen {
            let mut animal = Animal::new(&HORSE, Look::new(&HORSE, view.seed, Some(coat)));
            let start = here.point - heading * (flight.at)(flight.from_s + flight.span_s / 2.0).1;
            let place = |t: f64| {
                let (over_m, along_m) = (flight.at)(t);
                land.under(start + heading * along_m).point + up * over_m
            };
            let going = |t: f64| Going {
                position: place(t),
                heading,
                speed_mps: flight.speed_mps,
                turn_rps: 0.0,
                graze: 0.0,
                balk_s: None,
                leap: None,
                wings: (flight.out)(t),
            };
            let mut t = 0.0;
            let mut pictures = Vec::new();
            for step in 0..STEPS {
                let until = flight.from_s + flight.span_s * step as f64 / (STEPS - 1) as f64;
                while t + 1e-9 < until {
                    let dt = TICK_S.min(until - t);
                    t += dt;
                    animal.step(dt, going(t), &land);
                }
                let target = animal.position() + up * 1.2;
                let eye = target + *from + up * flight.raised_m;
                let mut frame = base.clone();
                frame.skinned.clear();
                frame.camera.position = eye;
                let eye_up = if from.dot(up).abs() > 0.9 * from.length() {
                    heading
                } else {
                    up
                };
                frame.camera.rotation = looking(target - eye, eye_up);
                animal.boxes(&mut frame.boxes);
                pictures.push(headless.render(&frame));
            }
            let out = dir.join(format!("{}{from_name}-{coat}.png", flight.name));
            write_png(
                &out,
                size[0] * pictures.len() as u32,
                size[1],
                &side_by_side(&pictures, size),
            )?;
            log::info!("{} -> {}", flight.name, out.display());
        }
    }
    Ok(())
}
