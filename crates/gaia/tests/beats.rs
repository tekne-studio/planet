//! What a horse is heard by: the beats of the gait it travels at, by the
//! gait's own clock, never a foot set down as it turns, shuffles or flies.

use gaia::fauna::horse::HORSE;
use gaia::fauna::{Animal, Beat, Footing, Going, Ground, Landing, Leap, Look};
use glam::{DQuat, DVec3};

struct Flat;

impl Ground for Flat {
    fn under(&self, at: DVec3) -> Footing {
        Footing {
            point: DVec3::new(at.x, 0.0, at.z),
            up: DVec3::Y,
            surface: 3,
        }
    }
}

const DT: f64 = 1.0 / 60.0;

/// Lives a horse `seconds`, moved each frame by `going` from the time and
/// where it is, and says each beat heard with the second it was.
fn heard(
    seconds: f64,
    going: impl FnMut(f64, DVec3, DVec3) -> Going,
) -> (Animal, Vec<(f64, Beat)>) {
    heard_of(Look::new(&HORSE, 7, None), seconds, going)
}

fn heard_of(
    look: Look,
    seconds: f64,
    mut going: impl FnMut(f64, DVec3, DVec3) -> Going,
) -> (Animal, Vec<(f64, Beat)>) {
    let mut animal = Animal::new(&HORSE, look);
    let mut beats = Vec::new();
    let (mut at, mut heading) = (DVec3::ZERO, DVec3::X);
    for frame in 0..(seconds / DT) as usize {
        let s = frame as f64 * DT;
        let now = going(s, at, heading);
        (at, heading) = (now.position, now.heading);
        if let Some(beat) = animal.step(DT, now, &Flat) {
            beats.push((s, beat));
        }
    }
    (animal, beats)
}

fn standing(position: DVec3, heading: DVec3) -> Going {
    Going {
        position,
        heading,
        speed_mps: 0.0,
        turn_rps: 0.0,
        graze: 0.0,
        balk_s: None,
        leap: None,
        wings: false,
    }
}

/// Travelling straight at `speed_mps`.
fn travelling(speed_mps: f64) -> impl FnMut(f64, DVec3, DVec3) -> Going {
    move |_, at, heading| Going {
        position: at + heading * speed_mps * DT,
        speed_mps,
        ..standing(at, heading)
    }
}

/// The gaps between the beats heard after the first `settle_s`, seconds.
fn gaps(beats: &[(f64, Beat)], settle_s: f64) -> Vec<f64> {
    let times: Vec<f64> = beats
        .iter()
        .map(|(s, _)| *s)
        .filter(|&s| s > settle_s)
        .collect();
    times.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

/// Strides a second at a speed, for this horse's size.
fn strides_hz(animal: &Animal, speed_mps: f64) -> f64 {
    let pace = speed_mps / animal.look.traits.size.sqrt();
    HORSE.gaits.at(pace).hz_at(pace) / animal.look.traits.size.sqrt()
}

#[test]
fn turning_in_place_or_shuffling_a_step_is_never_heard() {
    let (_, turned) = heard(10.0, |_, at, heading| Going {
        heading: DQuat::from_rotation_y(1.2 * DT) * heading,
        turn_rps: 1.2,
        ..standing(at, heading)
    });
    assert!(turned.is_empty(), "{} beats turning", turned.len());

    // Half a metre eased every three seconds, as a herd's jitter moves it.
    let (_, shuffled) = heard(30.0, |s, at, heading| {
        let t = s.rem_euclid(3.0);
        let speed_mps = match t < 1.0 {
            true => 0.5 * 6.0 * t * (1.0 - t),
            false => 0.0,
        };
        Going {
            position: at + heading * speed_mps * DT,
            speed_mps,
            ..standing(at, heading)
        }
    });
    assert!(shuffled.is_empty(), "{} beats shuffling", shuffled.len());
}

#[test]
fn a_walk_is_heard_in_four_even_soft_beats() {
    let (animal, beats) = heard(12.0, travelling(1.5));
    let gaps = gaps(&beats, 2.0);
    let per_stride = (gaps.len() as f64 + 1.0) / (10.0 * strides_hz(&animal, 1.5));
    assert!(
        (per_stride - 4.0).abs() < 0.3,
        "{per_stride} beats a stride"
    );
    let (least, most) = spread(&gaps);
    assert!(most / least < 1.2, "uneven: {gaps:?}");
    assert!(beats.iter().all(|(_, beat)| beat.force < 0.5));
}

#[test]
fn a_trot_is_heard_in_two_even_beats() {
    let (animal, beats) = heard(12.0, travelling(3.6));
    let gaps = gaps(&beats, 2.0);
    let per_stride = (gaps.len() as f64 + 1.0) / (10.0 * strides_hz(&animal, 3.6));
    assert!(
        (per_stride - 2.0).abs() < 0.3,
        "{per_stride} beats a stride"
    );
    let (least, most) = spread(&gaps);
    assert!(most / least < 1.2, "uneven: {gaps:?}");
}

#[test]
fn a_gallop_is_heard_in_three_beats_and_a_pause_full() {
    let (animal, beats) = heard(12.0, travelling(9.0));
    let gaps = gaps(&beats, 2.0);
    let per_stride = (gaps.len() as f64 + 1.0) / (10.0 * strides_hz(&animal, 9.0));
    assert!(
        (per_stride - 3.0).abs() < 0.3,
        "{per_stride} beats a stride"
    );
    // Two short gaps, then the flight: a gap of more than twice either.
    let (least, most) = spread(&gaps);
    assert!(most > 2.0 * least, "no pause: {gaps:?}");
    let long = gaps.iter().filter(|&&gap| gap > 1.5 * least).count();
    assert!(
        (long as f64 - gaps.len() as f64 / 3.0).abs() <= 1.0,
        "one pause every three beats: {gaps:?}"
    );
    assert!(beats.iter().all(|(_, beat)| beat.force == 1.0));
}

#[test]
fn in_the_air_it_is_not_heard() {
    let winged = Look::new(&HORSE, 7, Some("white"));
    let (_, flown) = heard_of(winged, 6.0, |_, at, heading| Going {
        position: DVec3::new(at.x, 20.0, at.z) + heading * 12.0 * DT,
        speed_mps: 12.0,
        wings: true,
        ..standing(at, heading)
    });
    assert!(flown.is_empty(), "{} beats on the wing", flown.len());

    let mut jumping = travelling(9.0);
    let (_, jumped) = heard(6.0, move |s, at, heading| {
        let going = jumping(s, at, heading);
        match s > 1.0 {
            true => Going {
                position: going.position + DVec3::Y * 1.0,
                leap: Some(Leap::Flying),
                ..going
            },
            false => going,
        }
    });
    assert!(
        jumped.iter().all(|(s, _)| *s <= 1.0),
        "heard while it jumps"
    );
}

/// A pegasus flown fast and level holds its wings out to glide, and says
/// where and how fast; flown slow it beats them, and on the ground it has
/// none out. It flies far over the flat ground, as over a planet whose
/// middle is the origin, so going level is no climb.
#[test]
fn flown_fast_and_level_it_glides() {
    const HIGH_M: f64 = 1.0e6;
    let flown = |speed_mps: f64| {
        let winged = Look::new(&HORSE, 7, Some("white"));
        heard_of(winged, 6.0, move |_, at, heading| Going {
            position: DVec3::new(at.x, HIGH_M, at.z) + heading * speed_mps * DT,
            speed_mps,
            wings: true,
            ..standing(at, heading)
        })
        .0
    };
    let glide = flown(60.0).glide().expect("gliding");
    assert!(glide.held_out > 0.95, "{glide:?}");
    assert!((glide.speed_mps - 60.0).abs() < 1.0, "{glide:?}");
    assert!(glide.at.y - HIGH_M > 0.5, "at its withers: {glide:?}");
    let slow = flown(6.0).glide().map_or(0.0, |glide| glide.held_out);
    assert!(slow < 0.2, "beating slow: {slow}");
    let winged = Look::new(&HORSE, 7, Some("white"));
    let (standing_still, _) = heard_of(winged, 2.0, |_, at, heading| standing(at, heading));
    assert_eq!(standing_still.glide(), None, "on the ground");
}

/// It springs once a jump, as the ground lets it go from its gathering,
/// and lands once, as its feet meet it again; never on a balk, which
/// gathers and stays, nor lifted a moment off a bump. Taking to the wing
/// from the ground it springs once, and lands once coming down from it,
/// set down.
#[test]
fn it_springs_and_lands_once_a_jump_and_never_on_a_balk() {
    let leaps = |look: Look, leap: fn(f64) -> (Option<Leap>, bool)| {
        let mut animal = Animal::new(&HORSE, look);
        let (mut at, heading) = (DVec3::ZERO, DVec3::X);
        let (mut sprang, mut landed) = (0, Vec::new());
        for frame in 0..(4.0 / DT) as usize {
            let (leap, wings) = leap(frame as f64 * DT);
            let lift = if leap == Some(Leap::Flying) || wings {
                0.8
            } else {
                0.0
            };
            at += heading * 3.0 * DT;
            let going = Going {
                position: DVec3::new(at.x, lift, at.z),
                speed_mps: 3.0,
                leap,
                wings,
                ..standing(at, heading)
            };
            animal.step(DT, going, &Flat);
            sprang += usize::from(animal.sprang().is_some());
            landed.extend(animal.landed());
        }
        (
            sprang,
            landed
                .iter()
                .map(|landing| landing.winged)
                .collect::<Vec<_>>(),
        )
    };
    let horse = || Look::new(&HORSE, 7, None);
    let jumped = leaps(horse(), |s| match s {
        _ if (1.0..1.4).contains(&s) => (Some(Leap::Gathering((s - 1.0) / 0.4)), false),
        _ if (1.4..2.2).contains(&s) => (Some(Leap::Flying), false),
        _ => (None, false),
    });
    assert_eq!(jumped, (1, vec![false]), "a jump");
    let balked = leaps(horse(), |s| match s {
        _ if (1.0..1.3).contains(&s) => (Some(Leap::Gathering((s - 1.0) / 0.4)), false),
        _ => (None, false),
    });
    assert_eq!(balked, (0, vec![]), "a balk");
    let bumped = leaps(horse(), |s| match s {
        _ if (1.0..1.1).contains(&s) => (Some(Leap::Flying), false),
        _ => (None, false),
    });
    assert_eq!(bumped, (0, vec![]), "a bump");
    let white = || Look::new(&HORSE, 7, Some("white"));
    let winged = leaps(white(), |s| (None, (1.0..3.0).contains(&s)));
    assert_eq!(winged, (1, vec![true]), "to the wing and back down");
}

/// The longest wait between two beats of its gait at `speed_mps`, seconds:
/// a gallop's flight.
fn longest_gap_s(animal: &Animal, speed_mps: f64) -> f64 {
    let pace = speed_mps / animal.look.traits.size.sqrt();
    let beats = HORSE.gaits.at(pace).beats;
    let gaps = beats
        .iter()
        .zip(beats.iter().cycle().skip(1))
        .map(|(beat, next)| (next - beat).rem_euclid(1.0));
    gaps.fold(0.0, f64::max) / strides_hz(animal, speed_mps)
}

/// Come down at a gallop, from a jump or from the wing, it gallops on at
/// once: its next beat is heard within the longest wait of its gait,
/// never after the time a herd's shuffle is kept silent for.
#[test]
fn come_down_at_a_gallop_it_is_heard_at_its_next_beat() {
    const GALLOP_MPS: f64 = 9.6;
    let down_s = 2.0;
    let mut galloping = travelling(GALLOP_MPS);
    let flown = move |s: f64, at: DVec3, heading: DVec3| {
        let going = galloping(s, at, heading);
        match s < down_s {
            true => Going {
                position: going.position + DVec3::Y * 20.0,
                wings: true,
                ..going
            },
            false => going,
        }
    };
    let mut galloping = travelling(GALLOP_MPS);
    let jumped = move |s: f64, at: DVec3, heading: DVec3| {
        let going = galloping(s, at, heading);
        match (down_s - 0.8..down_s).contains(&s) {
            true => Going {
                position: going.position + DVec3::Y * 1.0,
                leap: Some(Leap::Flying),
                ..going
            },
            false => going,
        }
    };
    let white = Look::new(&HORSE, 7, Some("white"));
    for (what, (animal, beats)) in [
        ("from the wing", heard_of(white, 4.0, flown)),
        ("from a jump", heard(4.0, jumped)),
    ] {
        let first_s = beats
            .iter()
            .map(|(s, _)| s - down_s)
            .find(|&since_s| since_s >= 0.0)
            .unwrap_or(f64::MAX);
        let longest_s = longest_gap_s(&animal, GALLOP_MPS);
        assert!(
            first_s <= longest_s,
            "{what}: first beat {first_s:.3} s after touching down, its gait waits {longest_s:.3} s at most"
        );
    }
}

/// Coming down from the wing it lands as hard as it sank onto the ground
/// and as fast as it went along it: down slow, a soft touch; down fast,
/// hard.
#[test]
fn it_lands_from_the_wing_as_fast_as_it_sank() {
    let landing = |sink_mps: f64, speed_mps: f64| {
        let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 7, Some("white")));
        let (mut at, heading, mut lift) = (DVec3::ZERO, DVec3::X, 1.0);
        let mut landed: Vec<Landing> = Vec::new();
        for _ in 0..(4.0 / DT) as usize {
            at += heading * speed_mps * DT;
            lift = f64::max(lift - sink_mps * DT, 0.0);
            let going = Going {
                position: DVec3::new(at.x, lift, at.z),
                speed_mps,
                wings: lift > 0.0,
                ..standing(at, heading)
            };
            animal.step(DT, going, &Flat);
            landed.extend(animal.landed());
        }
        assert_eq!(landed.len(), 1, "{sink_mps} m/s: lands once");
        landed[0]
    };
    for (sink_mps, speed_mps) in [(0.6, 0.0), (3.0, 9.6)] {
        let landed = landing(sink_mps, speed_mps);
        assert!(landed.winged);
        assert!(
            (landed.sink_mps - sink_mps).abs() < 0.01
                && (landed.speed_mps - speed_mps).abs() < 1e-9,
            "{landed:?}"
        );
    }
}

/// A jump from the gallop springs and lands harder than one from a halt.
#[test]
fn a_jump_is_as_hard_as_its_pace() {
    let force = |speed_mps: f64| {
        let mut animal = Animal::new(&HORSE, Look::new(&HORSE, 7, None));
        let going = Going {
            speed_mps,
            ..standing(DVec3::ZERO, DVec3::X)
        };
        animal.step(DT, going, &Flat);
        animal.leap_force()
    };
    let (halt, gallop) = (force(0.0), force(9.0));
    assert!(halt > 0.5 && gallop == 1.0, "{halt} {gallop}");
}

fn spread(gaps: &[f64]) -> (f64, f64) {
    let least = gaps.iter().copied().fold(f64::MAX, f64::min);
    let most = gaps.iter().copied().fold(0.0, f64::max);
    (least, most)
}
