//! A rider sits on a horse's back, never in it: the ride's pose fitted
//! round the side under the saddle keeps every leg out of every box of the
//! trunk, and a thing held in the hand and its beam out of every box of
//! the horse, on the smallest pony and the largest draught alike.

use avatar::{Avatar, Bone, Pose};
use client::{Step, Straddle};
use gaia::fauna::horse::HORSE;
use gaia::fauna::{Animal, Footing, Going, Ground, Look, Traits};
use glam::{DAffine3, DVec3, Quat, Vec3};

/// A humanoid's legs, in units of its hips' height at rest: where a hip
/// joint stands from the middle, a thigh's and a shin's length, and how
/// far over the seat the hip joints are.
const HIP_X: f64 = 0.1;
const THIGH: f64 = 0.5;
const SHIN: f64 = 0.5;
const SEAT: f64 = 0.12;
/// How thick a thigh and a shin are, metres, from the middle out.
const THIGH_M: f64 = 0.07;
const SHIN_M: f64 = 0.05;

/// A horse of `size` and `build`.
fn horse(size: f64, build: f64) -> Animal {
    let traits = Traits {
        size,
        build,
        ..Traits::drawn(&HORSE, 7, None)
    };
    Animal::new(&HORSE, Look::of(&HORSE, traits))
}

/// What it sits astride: its side under the saddle.
fn straddle(horse: &Animal) -> Straddle {
    let steps = (horse.flanks().iter())
        .map(|flank| Step {
            top_m: flank.top_m as f32,
            bottom_m: flank.bottom_m as f32,
            out_m: flank.out_m as f32,
        })
        .collect();
    Straddle { steps }
}

/// How far `at` is from a box from `low` to `high`, less than zero inside.
fn away(at: DVec3, low: DVec3, high: DVec3) -> f64 {
    let outside = (low - at).max(at - high);
    match outside.max_element() > 0.0 {
        true => outside.max(DVec3::ZERO).length(),
        false => outside.max_element(),
    }
}

#[test]
fn a_rider_sits_on_the_back_with_nothing_inside_the_horse() {
    let carry = HORSE.carry.expect("a horse carries");
    // The smallest pony and its stockiest, a saddle horse, the largest
    // draught; riders short and tall.
    for (size, build) in [(0.66, 1.08), (0.66, 1.25), (1.0, 1.0), (1.16, 1.4)] {
        let horse = horse(size, build);
        let straddle = straddle(&horse);
        for hips_m in [0.85, 0.95, 1.05] {
            let held =
                ride_client::seated().astride(&straddle.in_units(hips_m as f32), SEAT as f32);
            let hips = held.bone("hips").unwrap_or_default();
            let mut nearest = f64::MAX;
            for (side, upper, lower) in [
                (-1.0, "leftUpperLeg", "leftLowerLeg"),
                (1.0, "rightUpperLeg", "rightLowerLeg"),
            ] {
                let thigh = hips * held.bone(upper).unwrap();
                let shin = thigh * held.bone(lower).unwrap();
                let along = |turn: glam::Quat| (turn * Vec3::NEG_Y).as_dvec3();
                // From the saddle point, metres, in the horse's frame: the
                // figure faces the way it does.
                let hip = DVec3::new(side * HIP_X, SEAT, 0.0) * hips_m;
                let knee = hip + along(thigh) * THIGH * hips_m;
                let sole = knee + along(shin) * SHIN * hips_m;
                for section in HORSE.rig.trunk {
                    let out = section.width / 2.0 * build;
                    let low = DVec3::new(-out, section.bottom, section.from_z);
                    let high = DVec3::new(out, section.top, section.to_z);
                    let (low, high) = ((low - carry.saddle) * size, (high - carry.saddle) * size);
                    let (low, high) = (low.with_x(-out * size), high.with_x(out * size));
                    for t in 0..=32 {
                        let t = f64::from(t) / 32.0;
                        nearest = nearest
                            .min(away(hip.lerp(knee, t), low, high) - THIGH_M)
                            .min(away(knee.lerp(sole, t), low, high) - SHIN_M);
                    }
                }
            }
            assert!(
                nearest >= 0.0,
                "size {size} build {build}, hips {hips_m} m: {nearest:.3} m in"
            );
        }
    }
}

/// The ground, level at zero.
struct Flat;

impl Ground for Flat {
    fn under(&self, at: DVec3) -> Footing {
        Footing {
            point: DVec3::new(at.x, 0.0, at.z),
            up: DVec3::Y,
            surface: 0,
        }
    }
}

/// An arm holding a thing out ahead, as a flashlight is held: the upper
/// arm down by the side and a little ahead, the forearm out ahead at the
/// height of the chest, a little in. A right arm rests along `x`.
fn holding() -> Vec<(&'static str, Quat)> {
    let upper = Quat::from_rotation_arc(Vec3::X, Vec3::new(0.12, -0.8, -0.58).normalize());
    let ahead = upper.inverse() * Vec3::new(-0.12, 0.18, -1.0).normalize();
    vec![
        ("rightUpperArm", upper),
        ("rightLowerArm", Quat::from_rotation_arc(Vec3::X, ahead)),
    ]
}

/// The thing in the hand, in the hand's frame: the body's axes at the
/// wrist. As long and as wide as a flashlight, held out ahead.
fn thing() -> DAffine3 {
    DAffine3::from_translation(DVec3::new(0.0, 0.0, -0.12))
        * DAffine3::from_scale(DVec3::new(0.09, 0.09, 0.25))
}

/// Its tip and the way its beam goes from there, ahead and falling a
/// little, as a flashlight's, in the hand's frame; and how far along the
/// beam is looked at, metres, in how many steps.
const TIP: DVec3 = DVec3::new(0.0, 0.0, -0.25);
const BEAM: DVec3 = DVec3::new(0.0, -0.12, -0.993);
const BEAM_M: f64 = 4.0;
const BEAM_STEPS: u32 = 200;

#[test]
fn a_thing_held_astride_is_held_clear_of_the_horse() {
    let avatars = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/avatars");
    let avatars: Vec<(&str, Avatar)> = ["Olivia", "Kyle", "Bizdude", "Shiro"]
        .into_iter()
        .map(|name| {
            let bytes = std::fs::read(avatars.join(format!("{name}.vrm"))).unwrap();
            (name, Avatar::from_vrm(&bytes).unwrap())
        })
        .collect();
    for (size, build) in [(0.66, 1.08), (0.66, 1.25), (1.0, 1.0), (1.16, 1.4)] {
        let mut horse = horse(size, build);
        for _ in 0..120 {
            horse.step(
                1.0 / 60.0,
                Going::standing(DVec3::ZERO, DVec3::NEG_Z),
                &Flat,
            );
        }
        let saddle = horse.saddle().expect("a horse carries");
        let mut parts = Vec::new();
        horse.boxes(&mut parts);
        let into: Vec<DAffine3> = parts.iter().map(|part| part.transform.inverse()).collect();
        let straddle = straddle(&horse);
        for (name, avatar) in &avatars {
            let rest_m = avatar.hips_height_m();
            let straddle = straddle.in_units(rest_m);
            let held = ride_client::seated().astride(&straddle, SEAT as f32);
            let mut pose = Pose::rest();
            for (bone, turn) in held.bones.iter().chain(&straddle.reach(&holding())) {
                pose.set_toward(bone, *turn, 1.0);
            }
            pose.hips_toward(held.hips, 1.0);
            let (_, wrist) = avatar.posed(&pose, Bone::RIGHT_HAND);
            // Drawn as the core draws a rider: its feet hung under the
            // saddle by its hips, less its seat, its axes the saddle's.
            let hips_m = f64::from((held.hips.y - SEAT as f32) * rest_m);
            let feet = saddle.translation - saddle.matrix3.y_axis * hips_m;
            let hand = DAffine3::from_mat3_translation(
                saddle.matrix3,
                feet + saddle.matrix3 * wrist.as_dvec3(),
            );
            let thing = hand * thing();
            // Points through the thing, every one outside every box.
            let mut deepest = f64::MIN;
            for i in 0..=4 {
                for j in 0..=4 {
                    for k in 0..=8 {
                        let at =
                            DVec3::new(f64::from(i) / 4.0, f64::from(j) / 4.0, f64::from(k) / 8.0);
                        let at = thing.transform_point3(at - 0.5);
                        for into in &into {
                            let local = into.transform_point3(at);
                            deepest = deepest.max(0.5 - local.abs().max_element());
                        }
                    }
                }
            }
            assert!(
                deepest < 0.0,
                "size {size} build {build}, {name}: the thing is in the horse ({deepest:.3})"
            );
            // Its beam, from its tip ahead, falling a little, lights the
            // way and never the horse first.
            let (tip, beam) = (hand.transform_point3(TIP), hand.transform_vector3(BEAM));
            let met = (0..=BEAM_STEPS)
                .map(|n| f64::from(n) * BEAM_M / f64::from(BEAM_STEPS))
                .find(|&far_m| {
                    let at = tip + beam * far_m;
                    (into.iter()).any(|into| into.transform_point3(at).abs().max_element() < 0.5)
                });
            assert_eq!(
                met, None,
                "size {size} build {build}, {name}: the beam meets the horse"
            );
        }
    }
}
