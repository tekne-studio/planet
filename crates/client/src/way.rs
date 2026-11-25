//! Ways of moving a plugin gives a body. A plugin says each as data, once,
//! as it is plugged in, the way it says its keys: a name, how the body moves
//! in it and the pose the figure holds there. The core moves the body and
//! holds the pose; the plugin asks for the way and draws what it draws. A
//! way is known by its full name, the plugin's and its own,
//! `<plugin>.<way>`, and only while its plugin is on.

use glam::{Quat, Vec2, Vec3};

use crate::given::{Gift, Given};
use crate::hover::Hover;
use crate::paces::Paces;

/// A way of moving, as a plugin gives it.
#[derive(Clone, PartialEq, Debug)]
pub struct Way {
    /// Its name in the plugin, the word after the dot.
    pub name: &'static str,
    /// How a body moves in it.
    pub profile: Profile,
    /// Who turns it: the body, or the camera.
    pub steering: Steering,
    /// The pose the figure holds in it.
    pub held: Held,
}

/// How a body moves in a way: what kind of way it is, and its numbers.
#[derive(Clone, PartialEq, Debug)]
pub enum Profile {
    /// Over the surface on a cushion, as a board glides.
    Hover(Hover),
    /// On feet at a few paces, as a horse is ridden: always steered, and
    /// always on an animal.
    Paces(Paces),
}

/// Who turns a body in a way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Steering {
    /// The body owns its heading: left and right turn it at the way's rate,
    /// round and round, forward and back are throttle and brake along it,
    /// and the camera follows behind it.
    Steered,
    /// The body goes where it is pushed, as seen from the camera, as a
    /// walker does, and turns toward it at the way's rate.
    Relative,
}

/// A pose a figure holds: bones of the humanoid by the names VRM gives them
/// (`hips`, `leftUpperLeg`), each turned from rest in its parent's frame,
/// written over whatever the figure plays, and the hips at a height, in units
/// of their height at rest. The frame is the figure's: `x` right, `y` up, `z`
/// back.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Held {
    pub bones: Vec<(&'static str, Quat)>,
    pub hips: Vec3,
}

/// What a figure sits astride: the outline of its side under the seat, as
/// steps, each from `top_m` down to `bottom_m` below the seat and reaching
/// `out_m` to each side. A back, a bench, a saddle: whatever it is.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Straddle {
    pub steps: Vec<Step>,
}

/// One step of a [`Straddle`], metres.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Step {
    pub top_m: f32,
    pub bottom_m: f32,
    pub out_m: f32,
}

/// Where a hip joint stands from the middle, and how long a thigh is, in
/// units of the hips' height at rest: a humanoid's, near enough for any.
const HIP_X: f32 = 0.1;
const THIGH: f32 = 0.5;
/// How far a leg's middle keeps from the side it goes round, in those
/// units: half its thickness, and a little air.
const CLEAR: f32 = 0.09;
/// How far back from straight down a shin hangs astride, radians.
const SHIN_BACK: f32 = 0.2;
/// The most a thigh reaches out, as a share of its length.
const OUT_MOST: f32 = 0.92;
/// Where a right shoulder joint stands from the middle, and how long the
/// arm is from it to the elbow and on to the wrist, in units of the hips'
/// height at rest: a humanoid's, near enough for any.
const SHOULDER_X: f32 = 0.2;
const UPPER_ARM: f32 = 0.3;
const FOREARM: f32 = 0.28;
/// How far past the widest step a hand that holds a thing keeps, in those
/// units: half the hand and of what it holds, with air.
const HAND_CLEAR: f32 = 0.1;

impl Straddle {
    /// It measured in `unit_m` metres.
    pub fn in_units(&self, unit_m: f32) -> Straddle {
        let steps = (self.steps.iter())
            .map(|step| Step {
                top_m: step.top_m / unit_m,
                bottom_m: step.bottom_m / unit_m,
                out_m: step.out_m / unit_m,
            })
            .collect();
        Straddle { steps }
    }

    /// How far its widest step reaches to each side.
    fn widest(&self) -> f32 {
        self.steps.iter().map(|step| step.out_m).fold(0.0, f32::max)
    }

    /// How far out from the middle a hand that holds a thing keeps astride
    /// it, in its own units: past its widest step by [`HAND_CLEAR`]. The
    /// side under the seat is wider than the neck before it, so a hand out
    /// past the one is out past the other and its mane.
    pub fn hand_out(&self) -> f32 {
        self.widest() + HAND_CLEAR
    }

    /// An arm that holds a thing, in units of the hips' height at rest,
    /// swung at the shoulder about the figure's up, from ahead toward its
    /// right, until its wrist is [`Straddle::hand_out`] from the middle, or
    /// straight out to the side at most: the hand keeps its height and the
    /// thing is held clear of what is ridden. An arm already that far out
    /// is as it was. Bones of the humanoid by their VRM names, as [`Held`]
    /// has them.
    pub fn reach(&self, arm: &[(&'static str, Quat)]) -> Vec<(&'static str, Quat)> {
        let bone = |name: &str| (arm.iter().find(|(bone, _)| *bone == name)).map(|(_, turn)| *turn);
        let Some(upper) = bone("rightUpperArm") else {
            return arm.to_vec();
        };
        let forearm = bone("rightLowerArm").unwrap_or(Quat::IDENTITY);
        // A right arm rests along `+x`, out; ahead is `-z`. The wrist seen
        // from above, at `reach` from the shoulder, `angle` from out toward
        // ahead; a turn about `+y` by minus an angle brings it that much
        // nearer out.
        let wrist = upper * Vec3::X * UPPER_ARM + upper * forearm * Vec3::X * FOREARM;
        let (reach, angle) = (
            Vec2::new(wrist.x, wrist.z).length(),
            (-wrist.z).atan2(wrist.x),
        );
        let need = self.hand_out() - SHOULDER_X;
        if reach < 1e-6 || wrist.x >= need {
            return arm.to_vec();
        }
        let to = match need < reach {
            true => (need / reach).acos(),
            false => 0.0,
        };
        let swing = Quat::from_rotation_y(-(angle - to).clamp(0.0, angle.max(0.0)));
        (arm.iter())
            .map(|&(name, turn)| match name {
                "rightUpperArm" => (name, swing * turn),
                _ => (name, turn),
            })
            .collect()
    }

    /// How far out a line down from `from` must go for each unit it drops
    /// to pass every step's outer corner at `CLEAR` or more, `from` being
    /// out from the middle and up from the seat. A step `from` is already
    /// beside, or above, or `CLEAR` inside of, asks nothing; one whose
    /// corner is nearer under it than `CLEAR`, as far out as a leg goes.
    fn slope(&self, from: Vec2) -> f32 {
        (self.steps.iter())
            .filter(|step| from.y + step.top_m > 0.0 && from.x - step.out_m < CLEAR)
            .map(|step| {
                // The corner `out` past `from` and `drop` under it; the line
                // at slope `k` passes it at (k drop - out) / sqrt(1 + k²).
                let (out, drop) = (step.out_m - from.x, from.y + step.top_m);
                let square = drop * drop - CLEAR * CLEAR;
                match square > 0.0 {
                    true => (out * drop + CLEAR * (out * out + square).sqrt()) / square,
                    false => f32::INFINITY,
                }
            })
            .fold(0.0, f32::max)
    }
}

/// `dir`, a way down, turned out from the middle (`+x`) until it goes out
/// at least `slope` for each unit it drops and `least` of its length,
/// keeping its way forward or back.
fn spread(dir: Vec3, slope: f32, least: f32) -> Vec3 {
    let along = Vec2::new(dir.y, dir.z).normalize_or(Vec2::NEG_X);
    let need = slope * -along.x;
    let out = match need.is_finite() {
        true => need / (1.0 + need * need).sqrt(),
        false => 1.0,
    };
    let out = out.max(least).max(dir.x).min(OUT_MOST);
    let along = along * (1.0 - out * out).sqrt();
    Vec3::new(out, along.x, along.y)
}

impl Held {
    /// The turn it gives a bone, by its name.
    pub fn bone(&self, name: &str) -> Option<Quat> {
        self.bones
            .iter()
            .find(|(bone, _)| *bone == name)
            .map(|(_, turn)| *turn)
    }

    /// The pose with its legs round what it sits astride, in units of the
    /// hips' height at rest, its hip joints `seat` over the seat: each thigh
    /// turned out until it clears every step of the side it passes and its
    /// knee the widest, and each shin hanging down past the steps below its
    /// knee, a little back, and out round one wider than a knee reaches. What a rider does on a pony and on a
    /// draught horse alike.
    pub fn astride(&self, straddle: &Straddle, seat: f32) -> Held {
        let down = Vec3::NEG_Y;
        let hips = self.bone("hips").unwrap_or(Quat::IDENTITY);
        let mut held = self.clone();
        for (side, upper, lower) in [
            (-1.0, "leftUpperLeg", "leftLowerLeg"),
            (1.0, "rightUpperLeg", "rightLowerLeg"),
        ] {
            // Out from the middle is `+x` on either side, here.
            let mirror = Vec3::new(side, 1.0, 1.0);
            let thigh = self.bone(upper).unwrap_or(Quat::IDENTITY);
            let was = hips * thigh * down;
            let hip = Vec2::new(HIP_X, seat);
            let knee_out = (straddle.widest() + CLEAR - HIP_X) / THIGH;
            let to = spread(was * mirror, straddle.slope(hip), knee_out);
            let thigh = hips.inverse() * Quat::from_rotation_arc(was, to * mirror) * hips * thigh;
            let knee = hip + THIGH * Vec2::new(to.x, to.y);
            let hang = Vec3::new(0.0, -SHIN_BACK.cos(), SHIN_BACK.sin());
            let hang = spread(hang, straddle.slope(knee), 0.0) * mirror;
            let shin = Quat::from_rotation_arc(down, ((hips * thigh).inverse() * hang).normalize());
            held.set(upper, thigh);
            held.set(lower, shin);
        }
        held
    }

    /// Gives a bone this turn, in place of the one it had.
    fn set(&mut self, name: &'static str, turn: Quat) {
        match self.bones.iter_mut().find(|(bone, _)| *bone == name) {
            Some((_, was)) => *was = turn,
            None => self.bones.push((name, turn)),
        }
    }
}

impl Profile {
    /// Whether a body in it rides an animal: it is mounted on one, never
    /// taken bare, and the body steps down off it as it leaves.
    pub fn carried(&self) -> bool {
        matches!(self, Profile::Paces(_))
    }

    /// What is wrong with it turned by `steering`, said; none when a body
    /// can move in it.
    fn wrong(&self, steering: Steering) -> Option<&'static str> {
        match self {
            Profile::Hover(_) => None,
            Profile::Paces(_) if steering != Steering::Steered => {
                Some("goes in paces, which are steered")
            }
            Profile::Paces(paces) if paces.paces.is_empty() => Some("has no pace"),
            Profile::Paces(paces) if paces.sprint >= paces.paces.len() => {
                Some("sprints at a pace it does not have")
            }
            Profile::Paces(_) => None,
        }
    }
}

/// Every way of moving the plugins plugged in gave.
pub(crate) type Ways = Given<Way>;

impl Gift for Way {
    const NOUN: &'static str = "way";
    const BYTES: usize = protocol::Limit::WayBytes as usize;

    fn name(&self) -> &'static str {
        self.name
    }

    fn wrong(&self) -> Option<String> {
        self.profile.wrong(self.steering).map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seated on a chair: thighs forward and a little apart, shins down.
    fn sitting(apart: f32) -> Held {
        let thigh = |side: f32| Quat::from_rotation_z(side * apart) * Quat::from_rotation_x(0.8);
        Held {
            bones: vec![
                ("leftUpperLeg", thigh(-1.0)),
                ("rightUpperLeg", thigh(1.0)),
                ("leftLowerLeg", Quat::from_rotation_x(-1.0)),
                ("rightLowerLeg", Quat::from_rotation_x(-1.0)),
            ],
            hips: Vec3::new(0.0, 0.6, 0.0),
        }
    }

    /// How long a shin is, as a thigh.
    const SHIN: f32 = THIGH;

    /// A roof of steps over a barrel, in units of the hips' height: `out`
    /// the barrel's reach to each side.
    fn roof(out: f32) -> Straddle {
        let step = |top_m, bottom_m, out_m| Step {
            top_m,
            bottom_m,
            out_m,
        };
        Straddle {
            steps: vec![
                step(0.0, 0.05, 0.07),
                step(0.05, 0.13, 0.16),
                step(0.13, 0.26, 0.24),
                step(0.26, 0.6, out),
            ],
        }
    }

    /// How near a leg of `held` comes to any step, its middle line from the
    /// hip joint `seat` over the seat to the sole: the least distance from
    /// a point of it to a step, less than zero inside one.
    fn nearest(held: &Held, straddle: &Straddle, seat: f32) -> f32 {
        let mut nearest = f32::MAX;
        for (side, upper, lower) in [
            (-1.0, "leftUpperLeg", "leftLowerLeg"),
            (1.0, "rightUpperLeg", "rightLowerLeg"),
        ] {
            let thigh = held.bone(upper).unwrap();
            let shin = held.bone(lower).unwrap();
            let hip = Vec3::new(side * HIP_X, seat, 0.0);
            let knee = hip + thigh * Vec3::NEG_Y * THIGH;
            let sole = knee + thigh * shin * Vec3::NEG_Y * SHIN;
            for t in 0..=64 {
                let t = t as f32 / 32.0;
                let at = match t <= 1.0 {
                    true => hip.lerp(knee, t),
                    false => knee.lerp(sole, t - 1.0),
                };
                for step in &straddle.steps {
                    let out = (side * at.x).abs() - step.out_m;
                    let off = (at.y + step.top_m).max(-step.bottom_m - at.y);
                    let away = Vec2::new(out.max(0.0), off.max(0.0)).length();
                    nearest = nearest.min(match away > 0.0 {
                        true => away,
                        false => out.max(off),
                    });
                }
            }
        }
        nearest
    }

    /// An arm holding a thing out ahead, as a flashlight is held.
    fn holding() -> Vec<(&'static str, Quat)> {
        let upper = Quat::from_rotation_arc(Vec3::X, Vec3::new(0.12, -0.8, -0.58).normalize());
        let ahead = upper.inverse() * Vec3::new(-0.12, 0.18, -1.0).normalize();
        vec![
            ("rightUpperArm", upper),
            ("rightLowerArm", Quat::from_rotation_arc(Vec3::X, ahead)),
            ("rightHand", Quat::IDENTITY),
        ]
    }

    /// Where the wrist of an arm stands, from the middle.
    fn wrist(arm: &[(&'static str, Quat)]) -> Vec3 {
        let upper = arm[0].1;
        let forearm = arm[1].1;
        Vec3::X * SHOULDER_X + upper * Vec3::X * UPPER_ARM + upper * forearm * Vec3::X * FOREARM
    }

    #[test]
    fn a_hand_astride_holds_its_thing_out_past_the_side_at_its_height() {
        let arm = holding();
        let was = wrist(&arm);
        // Narrower than the hand is out already: as it was.
        let narrow = Straddle {
            steps: vec![Step {
                top_m: 0.0,
                bottom_m: 0.5,
                out_m: was.x - HAND_CLEAR - 0.05,
            }],
        };
        assert_eq!(narrow.reach(&arm), arm);
        for out in [0.25, 0.35, 0.45] {
            let reached = roof(out).reach(&arm);
            let at = wrist(&reached);
            assert!((at.x - (out + HAND_CLEAR)).abs() < 1e-4, "{out}: {at}");
            assert!((at.y - was.y).abs() < 1e-4, "{out}: it rose to {at}");
            assert!(at.z < 0.0, "{out}: it is still ahead, {at}");
            assert_eq!(reached[1..], arm[1..]);
        }
        // Wider than the arm reaches: straight out to the side, no further.
        let at = wrist(&roof(2.0).reach(&arm));
        assert!(at.z.abs() < 1e-4 && at.x > was.x, "{at}");
    }

    #[test]
    fn astride_every_leg_clears_every_step_and_the_shins_hang() {
        for out in [0.24, 0.3, 0.36, 0.42] {
            for seat in [0.12, 0.2] {
                let straddle = roof(out);
                let held = sitting(0.1).astride(&straddle, seat);
                let nearest = nearest(&held, &straddle, seat);
                assert!(nearest >= CLEAR - 1e-4, "{out} {seat}: {nearest}");
                for lower in ["leftLowerLeg", "rightLowerLeg"] {
                    let upper = lower.replace("Lower", "Upper");
                    let shin = held.bone(&upper).unwrap() * held.bone(lower).unwrap();
                    let shin = shin * Vec3::NEG_Y;
                    let down = -shin.y > shin.x.abs();
                    assert!(down && shin.z > 0.0, "{out}: shin {shin}");
                }
            }
        }
    }

    #[test]
    fn astride_a_narrow_back_keeps_the_thighs_as_wide_as_they_were() {
        let wide = sitting(0.9);
        let narrow = Straddle {
            steps: vec![Step {
                top_m: 0.0,
                bottom_m: 0.5,
                out_m: 0.05,
            }],
        };
        let held = wide.astride(&narrow, 0.12);
        assert_eq!(held.bone("leftUpperLeg"), wide.bone("leftUpperLeg"));
        assert_eq!(held.bone("rightUpperLeg"), wide.bone("rightUpperLeg"));
        assert_eq!(held.hips, wide.hips);
    }
}
