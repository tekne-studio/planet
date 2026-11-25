//! An animal's legs over a long life: every frame, every leg stands under
//! its body, bends the way its joint bends and reaches the ground it is
//! on, through slopes, steps, turns, every pace and ground that changes
//! under it.

use gaia::fauna::horse::HORSE;
use gaia::fauna::{Animal, Footing, Going, Ground, Kind, LEGS, Look};
use glam::{DQuat, DVec3};

/// How high the stair in [`Hills`] is, metres.
const STAIR_M: f64 = 0.3;

/// Rolling ground with a stair in it, `lift_m` higher everywhere: what a
/// world swapped under a herd does.
struct Hills {
    lift_m: f64,
}

impl Ground for Hills {
    fn under(&self, at: DVec3) -> Footing {
        let rolling = 1.5 * (at.x * 0.07).sin() + 0.8 * (at.z * 0.11).cos();
        let stair = if (at.x * 0.1).rem_euclid(4.0) > 2.0 {
            STAIR_M
        } else {
            0.0
        };
        Footing {
            point: DVec3::new(at.x, rolling + stair + self.lift_m, at.z),
            up: DVec3::Y,
            surface: 0,
        }
    }
}

/// Checks every leg of a drawn animal: none points at the sky, none bends
/// against its joint, and each reaches its ground.
pub fn legs_hold(animal: &Animal, ground: &impl Ground, frame: usize) {
    legs_bend(animal, frame);
    for (leg, foot) in animal.feet().iter().enumerate().take(LEGS) {
        let off = (ground.under(*foot).point - *foot).length();
        assert!(
            off < 1e-6,
            "frame {frame}, foot {leg} is {off} m off the ground"
        );
    }
}

/// Checks every leg of a drawn animal, on the ground or in the air: none
/// points at the sky, none bends against its joint, and each foot down
/// reaches where it stands.
fn legs_bend(animal: &Animal, frame: usize) {
    let Some((body, legs)) = animal.legs() else {
        return;
    };
    let up = body.matrix3.y_axis.normalize();
    let size = animal.look.traits.size;
    for (leg, pose) in legs.iter().enumerate() {
        let below = (pose.root - pose.fetlock).dot(up);
        assert!(below > 0.0, "frame {frame}, leg {leg} points up: {below}");
        // The way the joint is asked to bend, across the leg: it never lies
        // along the leg, where the joint could flip to either side.
        let along = (pose.fetlock - pose.root).normalize();
        let across = pose.pole - along * pose.pole.dot(along);
        assert!(
            across.length() > 0.2,
            "frame {frame}, leg {leg} lies along its pole: {pose:?}"
        );
        let middle = (pose.root + pose.fetlock) / 2.0;
        let bend = (pose.joint - middle).dot(across.normalize());
        assert!(
            bend > -1e-6,
            "frame {frame}, leg {leg} bends the wrong way: {bend}"
        );
        // A foot down reaches its ground, to less than a hoof at the end of
        // a long stance; one in the air may swing short of where it goes.
        let short = (pose.fetlock - pose.target).length();
        assert!(
            !animal.planted()[leg] || short < 0.15 * size,
            "frame {frame}, leg {leg} falls {short} m short: {pose:?}"
        );
    }
}

/// Lives an animal of `kind` for `frames`, walking, trotting, galloping,
/// turning on the spot and grazing, with its ground lifted under it now
/// and then.
pub fn live(kind: &'static Kind, seed: u64, frames: usize) {
    let mut animal = Animal::new(kind, Look::new(kind, seed, None));
    let dt = 1.0 / 60.0;
    let mut position = DVec3::ZERO;
    let mut heading = DVec3::X;
    let mut speed_mps: f64 = 0.0;
    for frame in 0..frames {
        let s = frame as f64 * dt;
        let ground = Hills {
            lift_m: if (s / 40.0).rem_euclid(2.0) > 1.0 {
                25.0
            } else {
                0.0
            },
        };
        let wanted: f64 = match (s / 12.0) as usize % 5 {
            0 => 0.0,
            1 => 1.5,
            2 => 3.6,
            3 => 9.0,
            _ => 0.4,
        };
        // Eased as a rider's paces are, at 4 m/s each second up, 6 down.
        speed_mps += (wanted - speed_mps).clamp(-6.0 * dt, 4.0 * dt);
        let turn_rps = 0.8 * (s * 0.3).sin();
        heading = DQuat::from_rotation_y(turn_rps * dt) * heading;
        position += heading * speed_mps * dt;
        position = ground.under(position).point;
        let going = Going {
            position,
            heading,
            speed_mps,
            turn_rps,
            graze: if speed_mps == 0.0 { 1.0 } else { 0.0 },
            balk_s: None,
            leap: None,
            wings: false,
        };
        animal.step(dt, going, &ground);
        legs_hold(&animal, &ground, frame);
    }
}

#[test]
fn a_horse_keeps_its_legs_for_twenty_minutes() {
    for seed in 0..3 {
        live(&HORSE, seed, 60 * 60 * 20);
    }
}

/// Rolling ground with no stair, `roll` times as steep as [`Hills`]: at
/// 3, slopes of up to about 17 degrees.
struct Rolling {
    roll: f64,
}

impl Ground for Rolling {
    fn under(&self, at: DVec3) -> Footing {
        let rolling = 1.5 * (at.x * 0.07).sin() + 0.8 * (at.z * 0.11).cos();
        Footing {
            point: DVec3::new(at.x, self.roll * rolling, at.z),
            up: DVec3::Y,
            surface: 0,
        }
    }
}

/// How sharply a leg turns back on itself from one frame to the next, the
/// most over a run, metres: of its root, its joint and its fetlock, how far
/// each one's move this frame goes against its move the frame before,
/// past what it kept of its speed. Smooth motion makes little of it; a
/// leg thrown somewhere for a frame and back, a tremble, makes it all.
fn tremble(speed_mps: f64, roll: f64, fps: f64) -> [f64; 3] {
    let ground = Rolling { roll };
    let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 1, None));
    let dt = 1.0 / fps;
    let heading = DVec3::new(1.0, 0.0, 0.3).normalize();
    let mut was: Vec<[[DVec3; 3]; LEGS]> = Vec::new();
    let mut most = [0.0f64; 3];
    for frame in 0..(20.0 * fps) as usize {
        let position = ground.under(heading * speed_mps * frame as f64 * dt).point;
        let going = Going {
            position,
            heading,
            speed_mps,
            ..Going::standing(position, heading)
        };
        animal.step(dt, going, &ground);
        let (_, legs) = animal.legs().expect("drawn");
        was.push(legs.map(|leg| [leg.root, leg.joint, leg.fetlock]));
        // The first two seconds settle it into its gait.
        let k = was.len() - 1;
        if (frame as f64) < 2.0 * fps {
            continue;
        }
        let each = (0..LEGS).flat_map(|leg| (0..3).map(move |part| (leg, part)));
        for (leg, part) in each {
            let at = |i: usize| was[i][leg][part];
            let turned = |i: usize| at(i) - 2.0 * at(i - 1) + at(i - 2);
            let against = -turned(k).dot(turned(k - 1));
            most[part] = most[part].max(against.max(0.0).sqrt());
        }
    }
    most
}

/// Its legs never tremble: over a slope and on the flat, at every pace up
/// to the run, at 60 frames a second and at 30, the body carries each leg's
/// root without a jolt, and each joint turns no more sharply than the foot
/// it reaches for. What a frame has the most of: a hoof leaving at the run.
#[test]
fn a_horse_s_legs_never_tremble() {
    for fps in [60.0, 30.0] {
        for roll in [0.0, 3.0] {
            for speed_mps in [1.5, 3.6, 9.0, 12.5] {
                let [root, joint, fetlock] = tremble(speed_mps, roll, fps);
                let at = format!("{speed_mps} m/s, {roll} roll, {fps} fps");
                // Smooth motion turns by its acceleration times the frame
                // squared: 360 m/s2 is a jolt, a tenth of a metre at 60.
                assert!(root < 360.0 / (fps * fps), "{at}: the root jolts {root} m");
                assert!(
                    joint < 2.0 * fetlock + 0.02,
                    "{at}: the joint snaps {joint} m, its foot {fetlock} m"
                );
            }
        }
    }
}

/// Level ground at zero.
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

/// Steps an animal on flat ground at `speed_mps` for `seconds`, and says
/// the legs that came down, in order, and whether every foot down stayed
/// where it landed.
fn stepping(speed_mps: f64, seconds: f64) -> (Vec<usize>, bool) {
    let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 1, None));
    let dt = 1.0 / 60.0;
    let (mut fell, mut held) = (Vec::new(), true);
    let mut was: Option<([DVec3; LEGS], [bool; LEGS])> = None;
    for frame in 0..(seconds / dt) as usize {
        let going = Going {
            position: DVec3::X * speed_mps * frame as f64 * dt,
            heading: DVec3::X,
            speed_mps,
            turn_rps: 0.0,
            graze: 0.0,
            balk_s: None,
            leap: None,
            wings: false,
        };
        animal.step(dt, going, &Flat);
        let (feet, planted) = (animal.feet(), animal.planted());
        if let Some((feet_was, planted_was)) = was {
            for leg in 0..LEGS {
                if planted_was[leg] && planted[leg] {
                    held &= (feet[leg] - feet_was[leg]).length() < 1e-9;
                }
                // The first stride settles it into its gait.
                if !planted_was[leg] && planted[leg] && frame as f64 * dt > 2.0 {
                    fell.push(leg);
                }
            }
        }
        was = Some((feet, planted));
    }
    (fell, held)
}

#[test]
fn a_horse_keeps_its_legs_backing_over_hills() {
    let ground = Hills { lift_m: 0.0 };
    let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 2, None));
    let dt = 1.0 / 60.0;
    let (mut position, mut heading) = (DVec3::ZERO, DVec3::X);
    for frame in 0..60 * 120 {
        let s = frame as f64 * dt;
        let turn_rps = 1.2 * (s * 0.4).sin();
        heading = DQuat::from_rotation_y(turn_rps * dt) * heading;
        position = ground.under(position - heading * 0.6 * dt).point;
        let going = Going {
            position,
            heading,
            speed_mps: -0.6,
            turn_rps,
            graze: 0.0,
            balk_s: None,
            leap: None,
            wings: false,
        };
        animal.step(dt, going, &ground);
        legs_hold(&animal, &ground, frame);
    }
}

#[test]
fn a_horse_steps_backward_in_its_walks_order_reversed() {
    let (forward, held) = stepping(0.6, 8.0);
    assert!(held, "walking, a foot down stays where it landed");
    let (backward, held) = stepping(-0.6, 8.0);
    assert!(held, "backing, a foot down stays where it landed");
    assert!(backward.len() >= 8, "it steps: {backward:?}");
    // Each leg follows the one the walk has before it.
    let next = |order: &[usize], leg: usize| {
        let at = order.iter().position(|&l| l == leg).unwrap();
        order[(at + 1) % LEGS]
    };
    let walk: Vec<usize> = forward[..LEGS].to_vec();
    for pair in backward.windows(2) {
        assert_eq!(
            next(&walk, pair[1]),
            pair[0],
            "backing {backward:?} against walking {walk:?}"
        );
    }
}

/// A polar bear: another table, and no new code. Short thick legs on broad
/// paws, a long low trunk, a long snout, small round ears, no mane and a
/// stub of a tail; it walks and it lopes.
static BEAR: Kind = {
    use gaia::fauna::{
        Build, Coat, Ears, Gait, Gaits, Habitat, Herding, Leg, Marks, Neck, Part, Patches, Rig,
        Role, Section, Tail,
    };
    use glam::Vec3;
    const FUR: Vec3 = Vec3::new(0.78, 0.76, 0.66);
    const DARK: Vec3 = Vec3::new(0.02, 0.02, 0.025);
    const WALK: Gait = Gait {
        from_mps: 0.0,
        blend_mps: 0.0,
        hz: 0.6,
        hz_per_mps: 0.2,
        stance_m: 0.75,
        duty: 0.65,
        lands: [0.25, 0.75, 0.0, 0.5],
        lift_m: 0.09,
        bounce_m: 0.015,
        bounces: 2.0,
        sink: 0.3,
        rock: 0.02,
        nod: 0.05,
        nods: 2.0,
        neck: 0.15,
        head: -0.25,
        tail: 0.0,
        beats: &[0.0, 0.25, 0.5, 0.75],
        force: 0.3,
    };
    const LOPE: Gait = Gait {
        from_mps: 4.0,
        blend_mps: 1.2,
        hz: 1.8,
        hz_per_mps: 0.06,
        stance_m: 0.75,
        duty: 0.3,
        lands: [0.6, 0.7, 0.2, 0.3],
        lift_m: 0.2,
        bounce_m: 0.07,
        bounces: 1.0,
        sink: 0.7,
        rock: 0.09,
        nod: 0.1,
        nods: 1.0,
        neck: 0.1,
        head: -0.2,
        tail: 0.0,
        beats: &[0.2, 0.3, 0.65],
        force: 0.8,
    };
    Kind {
        name: "polar bear",
        number: 2,
        rig: Rig {
            fore: Leg {
                root: DVec3::new(-0.22, 0.72, -0.55),
                foot: DVec3::new(-0.24, 0.0, -0.62),
                upper_m: 0.36,
                lower_m: 0.3,
                upper: [0.24, 0.26],
                lower: [0.2, 0.2],
                sweep: 0.35,
            },
            hind: Leg {
                root: DVec3::new(-0.23, 0.78, 0.55),
                foot: DVec3::new(-0.24, 0.0, 0.66),
                upper_m: 0.4,
                lower_m: 0.32,
                upper: [0.26, 0.3],
                lower: [0.2, 0.2],
                sweep: 0.3,
            },
            pastern_m: 0.05,
            hoof_m: 0.06,
            hoof: [0.26, 0.3],
            fold: 0.6,
            fold_per_lift: 2.0,
            trunk: &[
                Section {
                    from_z: -0.8,
                    to_z: -0.2,
                    bottom: 0.55,
                    top: 1.2,
                    width: 0.7,
                    cuts: [1, 1],
                },
                Section {
                    from_z: -0.25,
                    to_z: 0.85,
                    bottom: 0.6,
                    top: 1.25,
                    width: 0.75,
                    cuts: [1, 1],
                },
            ],
            pivot: DVec3::new(0.0, 0.9, 0.0),
            roll_follow: 0.4,
            neck: Neck {
                root: DVec3::new(0.0, 0.95, -0.75),
                length_m: 0.45,
                into_m: 0.2,
                width_m: 0.4,
                depth_m: 0.42,
                cuts: 1,
                graze_neck: -0.6,
                graze_head: -0.5,
            },
            head: &[
                Part {
                    centre: DVec3::new(0.0, 0.02, -0.14),
                    size: DVec3::new(0.34, 0.3, 0.3),
                    role: Role::Hair,
                },
                Part {
                    centre: DVec3::new(0.0, -0.04, -0.38),
                    size: DVec3::new(0.18, 0.16, 0.24),
                    role: Role::Hair,
                },
                Part {
                    centre: DVec3::new(0.0, 0.0, -0.51),
                    size: DVec3::new(0.08, 0.06, 0.03),
                    role: Role::Muzzle,
                },
            ],
            ears: Ears {
                at: DVec3::new(0.12, 0.15, -0.04),
                size: DVec3::new(0.08, 0.07, 0.04),
                back: 0.1,
            },
            eye: Part {
                centre: DVec3::new(0.1, 0.07, -0.27),
                size: DVec3::new(0.02, 0.03, 0.03),
                role: Role::Eye,
            },
            tail: Tail {
                root: DVec3::new(0.0, 1.0, 0.88),
                dock_m: 0.1,
                dock_width_m: 0.1,
                hair: [0.0, 0.0],
                hang: 1.2,
                hair_hang: 1.2,
            },
        },
        gaits: Gaits(&[WALK, LOPE]),
        coats: &[Coat {
            name: "white",
            weight: 1,
            hair: FUR,
            mane: FUR,
            points: FUR,
            muzzle: DARK,
            hoof: DARK,
            patched: false,
            marked: 0.0,
        }],
        builds: &[Build {
            weight: 1,
            size: (0.9, 1.15),
            build: (0.95, 1.1),
            mane_m: (0.0, 0.0),
            tail_m: (0.0, 0.0),
            feathers: false,
            coats: &[],
        }],
        marks: Marks {
            faces: &[],
            socks: &[],
            patches: Patches {
                blots: (0, 0),
                radius_m: (0.0, 0.0),
                y: (0.0, 0.0),
                z: (0.0, 0.0),
            },
            white: FUR,
            pale_hoof: DARK,
            skin: DARK,
            eye: DARK,
        },
        herding: Herding {
            lives_on: &[Habitat::Snow, Habitat::Rock],
            steep: 0.8,
            region_m: 400.0,
            chance: 0.2,
            size: (1, 2),
            spread_m: 6.0,
            roam_m: 80.0,
            bout_s: (120.0, 240.0),
            wander_mps: 1.2,
            turn_per_s: 0.8,
            restless: (0.0, 0.4),
            trot_mps: 2.5,
        },
        carry: None,
        wings: None,
        voice_cents: (-100.0, 100.0),
    }
};

/// The bear lives as the horse does, on its own table: drawn, walking and
/// loping, its legs held.
#[test]
fn a_bear_is_a_table() {
    live(&BEAR, 5, 60 * 60 * 3);
    let mut bear = Animal::new(&BEAR, Look::new(&BEAR, 5, None));
    let ground = Hills { lift_m: 0.0 };
    bear.step(0.0, Going::standing(DVec3::ZERO, DVec3::X), &ground);
    let mut boxes = Vec::new();
    bear.boxes(&mut boxes);
    assert!(boxes.len() > 20, "{} boxes", boxes.len());
    assert!(bear.saddle().is_none());
}

/// How far each foot of a drawn animal hangs below where its leg meets the
/// body, along the body's up, metres.
fn hang(animal: &Animal) -> [f64; LEGS] {
    let (body, legs) = animal.legs().expect("drawn");
    let up = body.matrix3.y_axis.normalize();
    legs.map(|pose| (pose.root - pose.fetlock).dot(up))
}

/// Level ground a planet's radius over its centre, where climbing is
/// going away from the centre.
struct High;

const RADIUS_M: f64 = 1.0e6;

impl Ground for High {
    fn under(&self, at: DVec3) -> Footing {
        Footing {
            point: DVec3::new(at.x, RADIUS_M, at.z),
            up: DVec3::Y,
            surface: 0,
        }
    }
}

#[test]
fn on_the_wing_its_legs_tuck_and_reach_down_only_to_land() {
    let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 0, Some("white")));
    let dt = 1.0 / 60.0;
    let mut position = DVec3::new(0.0, RADIUS_M + 20.0, 0.0);
    let fly = |animal: &mut Animal, position: DVec3| {
        let going = Going {
            position,
            heading: DVec3::X,
            speed_mps: 15.0,
            turn_rps: 0.0,
            graze: 0.0,
            balk_s: None,
            leap: None,
            wings: true,
        };
        animal.step(dt, going, &High);
    };
    // Cruising, then climbing: tucked.
    for frame in 0..360 {
        let climb_mps = if frame < 180 { 0.0 } else { 3.0 };
        position += DVec3::new(15.0, climb_mps, 0.0) * dt;
        fly(&mut animal, position);
    }
    let tucked = hang(&animal);
    // Coming down to land, the last metre: reaching.
    position.y = RADIUS_M + 4.0;
    while position.y > RADIUS_M + 0.4 {
        position += DVec3::new(15.0, -2.0, 0.0) * dt;
        fly(&mut animal, position);
    }
    let reaching = hang(&animal);
    // Off again, climbing through the same last metres: tucked.
    while position.y < RADIUS_M + 2.0 {
        position += DVec3::new(15.0, 3.0, 0.0) * dt;
        fly(&mut animal, position);
    }
    let climbing = hang(&animal);
    for leg in 0..LEGS {
        assert!(
            (climbing[leg] - tucked[leg]).abs() < 0.1,
            "leg {leg} hangs {} m tucked and {} m climbing low",
            tucked[leg],
            climbing[leg]
        );
    }
    for leg in 0..LEGS {
        assert!(
            reaching[leg] > tucked[leg] + 0.25,
            "leg {leg} hangs {} m tucked and {} m reaching",
            tucked[leg],
            reaching[leg]
        );
    }
}

/// A pegasus flown fast is drawn where it flies, its saddle with it, at
/// any speed a way flies it and any frame rate: what it covers in a frame
/// at its own speed is flight, never a jump that sets it anew on the
/// ground under it.
#[test]
fn flown_fast_it_is_drawn_where_it_flies() {
    for fps in [20.0, 30.0, 60.0, 144.0] {
        for speed_mps in [15.0, 80.0, 150.0, 400.0] {
            let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 0, Some("white")));
            let dt = 1.0 / fps;
            let mut position = DVec3::new(0.0, RADIUS_M + 100.0, 0.0);
            for frame in 0..(fps as usize * 3) {
                position += DVec3::new(speed_mps, 2.0, 0.0) * dt;
                let going = Going {
                    position,
                    heading: DVec3::X,
                    speed_mps,
                    turn_rps: 0.0,
                    graze: 0.0,
                    balk_s: None,
                    leap: None,
                    wings: true,
                };
                animal.step(dt, going, &High);
                if frame >= 2 {
                    flies_where_it_is(&animal, position, frame);
                }
            }
            // Landing from high up it dives faster than it goes along the
            // ground.
            position.y = RADIUS_M + 500.0;
            animal.step(0.0, standing_aloft(position), &High);
            for frame in 0..fps as usize {
                position.y -= 400.0 * dt;
                animal.step(dt, standing_aloft(position), &High);
                if frame >= 2 {
                    flies_where_it_is(&animal, position, frame);
                }
            }
        }
    }
}

/// Aloft over `position`, still along the ground.
fn standing_aloft(position: DVec3) -> Going {
    Going {
        wings: true,
        ..Going::standing(position, DVec3::X)
    }
}

/// Its saddle is drawn over where it flies.
fn flies_where_it_is(animal: &Animal, position: DVec3, frame: usize) {
    let saddle = animal.saddle().expect("drawn").translation;
    let off_m = (saddle - position).length();
    assert!(
        off_m < 3.0,
        "frame {frame}: saddle {off_m} m from where it flies"
    );
}

/// What a rider sits astride is every box of the trunk under the saddle,
/// top first from the seat, as wide and deep as the animal's build and
/// size; a kind that carries none has nothing to sit on.
#[test]
fn its_side_under_the_saddle_is_every_box_there() {
    let carry = HORSE.carry.expect("a horse carries");
    let under = HORSE
        .rig
        .trunk
        .iter()
        .filter(|section| (section.from_z..=section.to_z).contains(&carry.saddle.z))
        .count();
    for seed in 0..32 {
        let animal = Animal::new(&HORSE, Look::new(&HORSE, seed, None));
        let (size, build) = (animal.look.traits.size, animal.look.traits.build);
        let flanks = animal.flanks();
        assert_eq!(flanks.len(), under, "seed {seed}");
        assert!(
            flanks[0].top_m.abs() < 1e-9,
            "the seat is its top: {flanks:?}"
        );
        for pair in flanks.windows(2) {
            assert!(pair[0].top_m < pair[1].top_m, "top first: {flanks:?}");
        }
        let widest = HORSE
            .rig
            .trunk
            .iter()
            .map(|section| section.width)
            .fold(0.0, f64::max);
        let out = flanks.iter().map(|flank| flank.out_m).fold(0.0, f64::max);
        assert!(
            (out - widest / 2.0 * build * size).abs() < 1e-9,
            "seed {seed}"
        );
    }
    assert!(
        Animal::new(&BEAR, Look::new(&BEAR, 1, None))
            .flanks()
            .is_empty()
    );
}

/// How sharply legs move over a run, relative to the body they hang from,
/// the most of any joint or fetlock: how fast, metres a second; how
/// sharply one turns back on itself from one frame to the next, metres a
/// second; and how hard one is thrown, metres a second squared.
#[derive(Clone, Copy, Debug, Default)]
struct Sharpness {
    speed: f64,
    back: f64,
    thrown: f64,
}

impl Sharpness {
    fn within(&self, most: &Sharpness) -> bool {
        self.speed <= most.speed && self.back <= most.back && self.thrown <= most.thrown
    }
}

/// Steps `animal` `frames` times at `fps` by `go`, which says whether its
/// frame counts, and says how sharply its legs moved through the frames
/// that do.
fn sharpness(
    animal: &mut Animal,
    fps: f64,
    frames: usize,
    mut go: impl FnMut(&mut Animal, usize) -> bool,
) -> Sharpness {
    let dt = 1.0 / fps;
    let mut was: Vec<[[DVec3; 2]; LEGS]> = Vec::new();
    let mut most = Sharpness::default();
    for frame in 0..frames {
        if !go(animal, frame) {
            continue;
        }
        let (_, legs) = animal.legs().expect("drawn");
        was.push(legs.map(|leg| [leg.joint - leg.root, leg.fetlock - leg.root]));
        let k = was.len() - 1;
        if k < 2 {
            continue;
        }
        for (leg, part) in (0..LEGS).flat_map(|leg| (0..2).map(move |part| (leg, part))) {
            let moved = |i: usize| was[i][leg][part] - was[i - 1][leg][part];
            let (now, before) = (moved(k), moved(k - 1));
            most.speed = most.speed.max(now.length() / dt);
            most.back = most.back.max((-now.dot(before)).max(0.0).sqrt() / dt);
            most.thrown = most.thrown.max((now - before).length() / (dt * dt));
        }
    }
    most
}

/// A run's speed, metres a second: its legs' sharpest, which a take-off,
/// its most sudden move, is held to.
const RUN_MPS: f64 = 12.5;

/// How sharply a pegasus's legs move taking off under its rider as a
/// rider's way takes it, from `speed_mps` at `fps` over [`Rolling`] ground
/// `roll` steep: on its feet through its gather, sprung by its kind's arc
/// to its top, then climbing eased in, as far as two seconds from the
/// press. Every frame from the press, none bends the wrong way or points
/// up and no fetlock goes under the ground.
fn taking_off(speed_mps: f64, roll: f64, fps: f64) -> Sharpness {
    let spring = HORSE.wings.expect("a horse may have wings").flight.spring;
    let ground = Rolling { roll };
    let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 0, Some("white")));
    let dt = 1.0 / fps;
    let heading = DVec3::new(1.0, 0.0, 0.3).normalize();
    let settle = (2.0 * fps) as usize;
    let rise_mps = 2.0 * spring.height_m / spring.rise_s;
    let (mut along_m, mut top_m) = (0.0, 0.0);
    sharpness(&mut animal, fps, settle + settle, |animal, frame| {
        let s = frame.saturating_sub(settle) as f64 * dt;
        let pressed = frame >= settle;
        let off = pressed && s > spring.gather_s;
        let mps = match off {
            true => speed_mps.max(spring.forward_mps),
            false => speed_mps,
        };
        along_m += mps * dt;
        let under = ground.under(heading * along_m).point;
        if s <= spring.gather_s {
            top_m = under.y;
        }
        let sprung = (s - spring.gather_s).clamp(0.0, spring.rise_s);
        let arc = rise_mps * sprung - rise_mps / spring.rise_s * sprung * sprung / 2.0;
        let climb = ((s - spring.top_s()) / 1.4).clamp(0.0, 1.0);
        let height_m = top_m + arc + 3.0 * climb * climb * (3.0 - 2.0 * climb);
        let position = match off {
            true => DVec3::new(under.x, height_m.max(under.y), under.z),
            false => under,
        };
        let going = Going {
            position,
            heading,
            speed_mps: mps,
            wings: pressed,
            ..Going::standing(position, heading)
        };
        animal.step(dt, going, &ground);
        if pressed {
            legs_bend(animal, frame);
            let (_, legs) = animal.legs().expect("drawn");
            for (leg, pose) in legs.iter().enumerate() {
                let over = pose.fetlock.y - ground.under(pose.fetlock).point.y;
                assert!(
                    over > 0.0,
                    "{speed_mps} m/s, {roll} roll, {fps} fps, {s:.3} s: fetlock {leg} {over} m over the ground"
                );
            }
        }
        pressed
    })
}

/// How sharply a horse's legs move at a run over [`Rolling`] ground
/// `roll` steep at `fps`, settled into it.
fn running(roll: f64, fps: f64) -> Sharpness {
    let ground = Rolling { roll };
    let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 0, Some("white")));
    let dt = 1.0 / fps;
    let heading = DVec3::new(1.0, 0.0, 0.3).normalize();
    sharpness(&mut animal, fps, (12.0 * fps) as usize, |animal, frame| {
        let position = ground.under(heading * RUN_MPS * frame as f64 * dt).point;
        let going = Going {
            position,
            heading,
            speed_mps: RUN_MPS,
            ..Going::standing(position, heading)
        };
        animal.step(dt, going, &ground);
        frame as f64 * dt > 2.0
    })
}

/// Its legs take it off whole: from a halt, a walk and a trot, on the flat
/// and up slopes of about 17 degrees, at 60 frames a second and at 30, they
/// gather, drive and tuck no more sharply than they run on that ground at
/// that rate, none thrown from one frame to the next; none bends the wrong
/// way, and no hoof goes through the ground.
#[test]
fn taking_off_its_legs_gather_drive_and_tuck_whole() {
    for fps in [60.0, 30.0] {
        for roll in [0.0, 3.0] {
            let run = running(roll, fps);
            for speed_mps in [0.0, 1.5, 3.6] {
                let taking_off = taking_off(speed_mps, roll, fps);
                assert!(
                    taking_off.within(&run),
                    "{speed_mps} m/s, {roll} roll, {fps} fps: {taking_off:?} against a run's {run:?}"
                );
            }
        }
    }
}
