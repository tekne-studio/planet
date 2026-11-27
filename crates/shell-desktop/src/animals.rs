//! Pictures of an animal at its paces, with a fixed clock and seed: how a
//! gait is looked at. `bun run shot --animals out/animals` writes, for each
//! pace of a horse, a sheet of eight steps of one stride, seen from the
//! side, on the flattest ground near the shot's place, backing among
//! them; for each pace, its turn at full lock seen from over it; and for
//! each pace that jumps, a sheet of its jump from the gather to the
//! landing. `--yaw` turns the
//! eye about the animal, from the side at 0 to behind it at 90;
//! `--against-sun` turns it to face the sun, the animal between; `--clock`
//! sets the hour, and the log says how high the sun stands.

use std::path::Path;

use client::Client;
use gaia::fauna::{Animal, Footing, Going, Ground, Leap, Look, horse::HORSE};
use glam::{DMat3, DQuat, DVec3};
use render::{Headless, write_png};
use worldgen::Generator;

/// Steps across one stride.
const STEPS: usize = 8;
/// How far from the animal the eye stands, and how high over the ground,
/// metres.
const AWAY_M: f64 = 4.2;
const EYE_M: f64 = 0.9;
/// How long an animal goes at its pace before the first step is taken,
/// seconds: its body settled into the gait.
const SETTLE_S: f64 = 2.0;
/// Pictures across a jump, from its gather to the end of its landing, and
/// how long the landing is looked at, seconds.
const JUMP_STEPS: usize = 10;
const LANDED_S: f64 = 0.35;
/// The steps it is stepped in between pictures, seconds.
const TICK_S: f64 = 1.0 / 240.0;
/// The paces shown, by the words the ride plugin gives them, and their
/// speeds, metres a second: negative backward, at the top of a backing.
const PACES: [(&str, f64); 5] = [
    ("walk", 1.5),
    ("trot", 3.6),
    ("gallop", 9.0),
    ("run", 11.7),
    ("back", -1.4),
];
/// A turn is looked at from this high over the animal, metres, and this
/// far to its side.
const OVER_M: f64 = 6.0;
const ASIDE_M: f64 = 2.5;
/// The time between pictures of a turn, seconds.
const TURN_STEP_S: f64 = 0.25;

/// Where the eye looks from.
#[derive(Default)]
pub struct View {
    /// Degrees about the animal from its side, toward behind it.
    pub yaw_deg: f64,
    /// The eye faces the sun, the animal between them.
    pub against_sun: bool,
    /// The animal's look, by its seed.
    pub seed: u64,
}

/// The ground the generator makes, and nothing built: where a hoof lands.
pub(crate) struct Land<'a>(pub(crate) &'a Generator);

impl Ground for Land<'_> {
    fn under(&self, at: DVec3) -> Footing {
        let sphere = self.0.sphere();
        let grid = sphere.blocks();
        let direction = grid.direction(grid.surface_point(at.to_array()));
        let up = DVec3::from(direction);
        let sample = self.0.sample(direction);
        Footing {
            point: up * (sphere.radius_m() + sample.height_m),
            up,
            surface: sample.material as u8,
        }
    }
}

/// Writes a sheet for each pace into `dir`, on the flattest ground near
/// where the shot stands.
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
    // The eye's side of the animal: away from the sun against it, else a
    // fixed way of the sector; the animal goes across it.
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
    let yaw = DQuat::from_axis_angle(up, -view.yaw_deg.to_radians());
    let toward_eye = yaw * side;
    let beside = toward_eye * AWAY_M;
    let over = up * OVER_M + toward_eye * ASIDE_M;

    // The eye stands `from` the point over the ground the animal is on.
    let shoot = |headless: &mut Headless, animal: &Animal, ground: DVec3, from: DVec3| {
        let target = ground + up * EYE_M;
        let eye = target + from;
        let mut frame = base.clone();
        // The body the shot set up stands where the animal goes.
        frame.skinned.clear();
        frame.camera.position = eye;
        frame.camera.rotation = looking(target - eye, up);
        animal.boxes(&mut frame.boxes);
        headless.render(&frame)
    };
    let sheet = |name: &str, pictures: &[Vec<u8>]| {
        let out = dir.join(format!("{name}.png"));
        write_png(
            &out,
            size[0] * pictures.len() as u32,
            size[1],
            &side_by_side(pictures, size),
        )
        .map(|()| out)
    };

    for (name, speed_mps) in PACES {
        let look = Look::new(&HORSE, view.seed, None);
        let scale = look.traits.size;
        let mut animal = Animal::new(&HORSE, look);
        let pace = speed_mps.abs() / scale.sqrt();
        let stride_s = scale.sqrt() / HORSE.gaits.at(pace).hz_at(pace);
        // It starts back along its way, to stand by the eye's point when the
        // pictures are taken.
        let start = here.point - heading * speed_mps * (SETTLE_S + stride_s / 2.0);
        let going = |t: f64| Going {
            position: land.under(start + heading * speed_mps * t).point,
            heading,
            speed_mps,
            turn_rps: 0.0,
            graze: 0.0,
            balk_s: None,
            leap: None,
            wings: false,
        };
        let mut t = 0.0;
        let step_to = |animal: &mut Animal, until: f64, t: &mut f64| {
            while *t + 1e-9 < until {
                let dt = TICK_S.min(until - *t);
                *t += dt;
                animal.step(dt, going(*t), &land);
            }
        };
        step_to(&mut animal, SETTLE_S, &mut t);
        let mut pictures = Vec::new();
        for step in 0..STEPS {
            step_to(
                &mut animal,
                SETTLE_S + stride_s * step as f64 / STEPS as f64,
                &mut t,
            );
            pictures.push(shoot(&mut headless, &animal, animal.position(), beside));
        }
        let out = sheet(name, &pictures)?;
        log::info!("{name}: a stride of {stride_s:.3} s -> {}", out.display());
    }

    let carry = HORSE.carry.as_ref().ok_or("a horse carries no rider")?;
    // Each pace at full lock, seen from over it: the neck bends into the
    // turn before the body follows.
    for ((name, speed_mps), pace) in PACES.iter().zip(carry.paces) {
        let mut animal = Animal::new(&HORSE, Look::new(&HORSE, view.seed, None));
        let turn_rps = pace.turn_per_s;
        let (mut at, mut way) = (here.point, heading);
        let mut t = 0.0;
        let mut pictures = Vec::new();
        for step in 0..STEPS {
            // Straight, then turning: the first picture is the neck before
            // the turn.
            let until = SETTLE_S + TURN_STEP_S * step as f64;
            while t + 1e-9 < until {
                let dt = TICK_S.min(until - t);
                t += dt;
                let turning = match t > SETTLE_S {
                    true => turn_rps,
                    false => 0.0,
                };
                way = DQuat::from_axis_angle(up, turning * dt) * way;
                at = land.under(at + way * speed_mps * dt).point;
                let going = Going {
                    position: at,
                    heading: way,
                    speed_mps: *speed_mps,
                    turn_rps: turning,
                    graze: 0.0,
                    balk_s: None,
                    leap: None,
                    wings: false,
                };
                animal.step(dt, going, &land);
            }
            pictures.push(shoot(&mut headless, &animal, at, over));
        }
        let out = sheet(&format!("{name}-turn"), &pictures)?;
        log::info!("{name}: turning {turn_rps:.1} rad/s -> {}", out.display());
    }
    // Each pace's jump tapped, its least, and the one from a halt.
    let charged = client::Jump::from(carry.jump);
    let jumps = PACES
        .iter()
        .filter_map(|&(name, speed_mps)| {
            let pace = carry
                .paces
                .iter()
                .find(|pace| pace.speed_mps == speed_mps)?;
            Some((name, speed_mps, pace.jump?))
        })
        .chain(carry.standing.map(|least| ("halt", 0.0, least)));
    for (name, speed_mps, least) in jumps {
        let look = Look::new(&HORSE, view.seed, None);
        let mut animal = Animal::new(&HORSE, look);
        // The arc the ride's controller flies on the planet.
        let gather_s = least.gather_s;
        let arc = charged.arc(least.height_m, speed_mps, client::GRAVITY_MPS2);
        let (rise_mps, launch_mps, air_s) = (arc.rise_mps(), arc.speed_mps, arc.air_s());
        let leaves_s = SETTLE_S + gather_s;
        let lands_s = leaves_s + air_s;
        // How far it has gone at `t`: at its pace, then sprung, then at its
        // pace again.
        let gone = |t: f64| {
            let aloft_s = (t - leaves_s).clamp(0.0, air_s);
            speed_mps * (t - aloft_s) + launch_mps * aloft_s
        };
        // It leaves the ground at the middle of its way, by the eye's point.
        let start = here.point - heading * (gone(leaves_s) + arc.length_m() / 2.0);
        let ground = |t: f64| land.under(start + heading * gone(t)).point;
        let going = |t: f64| {
            let (lift_m, leap) = match t {
                t if t < SETTLE_S => (0.0, None),
                t if t < leaves_s => {
                    let gathered = charged.gathered(t - SETTLE_S);
                    (0.0, Some(Leap::Gathering(gathered)))
                }
                t if t < lands_s => {
                    let s = t - leaves_s;
                    (
                        rise_mps * s - arc.fall_mps2 * s * s / 2.0,
                        Some(Leap::Flying),
                    )
                }
                _ => (0.0, None),
            };
            Going {
                position: ground(t) + up * lift_m.max(0.0),
                heading,
                speed_mps: if leap == Some(Leap::Flying) {
                    launch_mps
                } else {
                    speed_mps
                },
                turn_rps: 0.0,
                graze: 0.0,
                balk_s: None,
                leap,
                wings: false,
            }
        };
        let mut t = 0.0;
        let mut pictures = Vec::new();
        // From the gather to the landing's end, evenly.
        let span_s = gather_s + air_s + LANDED_S;
        for step in 0..JUMP_STEPS {
            let until = SETTLE_S + span_s * step as f64 / (JUMP_STEPS - 1) as f64;
            while t + 1e-9 < until {
                let dt = TICK_S.min(until - t);
                t += dt;
                animal.step(dt, going(t), &land);
            }
            pictures.push(shoot(&mut headless, &animal, ground(t), beside));
        }
        let out = sheet(&format!("{name}-jump"), &pictures)?;
        log::info!(
            "{name}: a jump {:.1} m high, {:.1} m long, {air_s:.2} s in the air -> {}",
            arc.height_m,
            arc.length_m(),
            out.display()
        );
    }
    Ok(())
}

/// How far around the shot's place a flat ground is looked for, metres,
/// in how many steps each way, and how far a stride's ground runs either
/// side of it.
const LOOK_M: f64 = 600.0;
const LOOKS: i32 = 12;
const STRIDE_M: f64 = 12.0;

/// The flattest ground near `at`, so a stride is seen against the
/// horizon and not a hillside.
pub(crate) fn flattest(land: &Land, at: DVec3) -> Footing {
    flattest_where(land, at, |_| true)
}

/// The same, of the ground above the sea that `kept` keeps.
pub(crate) fn flattest_where(land: &Land, at: DVec3, kept: impl Fn(&Footing) -> bool) -> Footing {
    let centre = land.under(at);
    let up = centre.up;
    let east = up.any_orthonormal_vector();
    let north = up.cross(east);
    let rise = |footing: &Footing| {
        [east, north, -east, -north]
            .map(|way| land.under(footing.point + way * STRIDE_M).point)
            .iter()
            .map(|p| (p - footing.point).dot(up).abs())
            .fold(0.0, f64::max)
    };
    let step = LOOK_M / f64::from(LOOKS);
    let above_sea = |footing: &Footing| footing.point.length() > land.0.sphere().radius_m() + 1.0;
    (-LOOKS..=LOOKS)
        .flat_map(|i| (-LOOKS..=LOOKS).map(move |j| (i, j)))
        .map(|(i, j)| {
            land.under(centre.point + (east * f64::from(i) + north * f64::from(j)) * step)
        })
        .filter(|footing| above_sea(footing) && kept(footing))
        .min_by(|a, b| rise(a).total_cmp(&rise(b)))
        .inspect(|f| {
            log::info!(
                "flattest: rises {:.2} m over {STRIDE_M} m, {:.0} m away; here {:.2}",
                rise(f),
                (f.point - centre.point).length(),
                rise(&centre)
            )
        })
        .unwrap_or(centre)
}

/// The camera's turn that looks along `forward`, `up` up.
pub(crate) fn looking(forward: DVec3, up: DVec3) -> DQuat {
    let back = -forward.normalize();
    let right = up.cross(back).normalize();
    DQuat::from_mat3(&DMat3::from_cols(right, back.cross(right), back))
}

/// Pictures of one size, left to right, as one.
pub(crate) fn side_by_side(pictures: &[Vec<u8>], [width, height]: [u32; 2]) -> Vec<u8> {
    let row = width as usize * 4;
    let mut sheet = Vec::with_capacity(row * height as usize * pictures.len());
    for y in 0..height as usize {
        for picture in pictures {
            sheet.extend_from_slice(&picture[y * row..(y + 1) * row]);
        }
    }
    sheet
}
