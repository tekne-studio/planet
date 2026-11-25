//! The fallback body: a figure of boxes with a walk cycle.
//!
//! Shown until a VRM avatar is loaded, and when none can be (no assets, a bad
//! file). Procedural, so it is always there.

use glam::{DAffine3, DQuat, DVec3, Vec3};
use scene::BoxPart;

use crate::figure::{Gait, Motion};
use crate::way::Held;

const SKIN: Vec3 = Vec3::new(0.92, 0.92, 0.92);
const CLOTH: Vec3 = Vec3::new(0.03, 0.03, 0.03);

/// Metres of walking per full swing of the legs.
const STRIDE_M: f64 = 1.6;
/// A leg's length: the hips stand this high at rest.
pub(crate) const LEG_M: f64 = 0.80;

/// A limb or body part in avatar space: `x` right, `y` up, `z` back.
struct Part {
    /// The joint the part swings around.
    pivot: DVec3,
    /// Centre of the box relative to the pivot.
    offset: DVec3,
    size: DVec3,
    color: Vec3,
    /// Swing around `x`, as a factor of the cycle: legs and arms alternate.
    swing: f64,
}

/// The top of the head box: pivot, offset and half its height.
pub const TOP_M: f64 = 1.42 + 0.16 + 0.14;

const PARTS: [Part; 6] = [
    Part {
        pivot: DVec3::new(0.0, 1.10, 0.0),
        offset: DVec3::new(0.0, 0.0, 0.0),
        size: DVec3::new(0.44, 0.60, 0.24),
        color: CLOTH,
        swing: 0.0,
    },
    Part {
        pivot: DVec3::new(0.0, 1.42, 0.0),
        offset: DVec3::new(0.0, 0.16, 0.0),
        size: DVec3::new(0.26, 0.28, 0.26),
        color: SKIN,
        swing: 0.0,
    },
    Part {
        pivot: DVec3::new(-0.12, LEG_M, 0.0),
        offset: DVec3::new(0.0, -LEG_M / 2.0, 0.0),
        size: DVec3::new(0.18, LEG_M, 0.20),
        color: CLOTH,
        swing: 1.0,
    },
    Part {
        pivot: DVec3::new(0.12, LEG_M, 0.0),
        offset: DVec3::new(0.0, -LEG_M / 2.0, 0.0),
        size: DVec3::new(0.18, LEG_M, 0.20),
        color: CLOTH,
        swing: -1.0,
    },
    Part {
        pivot: DVec3::new(-0.30, 1.38, 0.0),
        offset: DVec3::new(0.0, -0.30, 0.0),
        size: DVec3::new(0.14, 0.64, 0.16),
        color: SKIN,
        swing: -1.0,
    },
    Part {
        pivot: DVec3::new(0.30, 1.38, 0.0),
        offset: DVec3::new(0.0, -0.30, 0.0),
        size: DVec3::new(0.14, 0.64, 0.16),
        color: SKIN,
        swing: 1.0,
    },
];

/// The figure's boxes in world space for a body's motion, holding `held`
/// as far as boxes can: turned as its hips are, the legs apart as its upper
/// legs are, lowered to keep the feet down.
pub fn parts(motion: &Motion, held: Option<&Held>) -> Vec<BoxPart> {
    if let Some(held) = held {
        return holding(motion, held);
    }
    let body = DAffine3::from_mat3_translation(motion.basis, motion.position);

    let flying = motion.gait == Gait::Fly;
    // Superman: the whole figure tips forward around the hips, arms ahead.
    let speed = (motion.speed_mps / 40.0).min(1.0);
    let lean = if flying {
        DQuat::from_rotation_x(-1.35 * speed)
    } else {
        DQuat::IDENTITY
    };
    let hips = DVec3::new(0.0, LEG_M, 0.0);
    let pose = DAffine3::from_translation(hips)
        * DAffine3::from_quat(lean)
        * DAffine3::from_translation(-hips);

    let cycle = (motion.stride_m / STRIDE_M * core::f64::consts::TAU).sin();
    let moving = (motion.speed_mps / 5.0).min(1.0);
    PARTS
        .iter()
        .map(|part| {
            let is_arm = part.pivot.y > 1.3 && part.swing != 0.0;
            let angle = if flying {
                if is_arm { -2.9 * speed } else { 0.0 }
            } else if motion.gait.grounded() {
                part.swing * cycle * 0.7 * moving
            } else {
                part.swing * 0.5
            };
            let local = DAffine3::from_translation(part.pivot)
                * DAffine3::from_quat(DQuat::from_rotation_x(angle))
                * DAffine3::from_translation(part.offset)
                * DAffine3::from_scale(part.size);
            BoxPart {
                transform: body * pose * local,
                color: part.color,
                shine: 0.0,
                shadow_only: false,
                sheet: false,
            }
        })
        .collect()
}

/// The figure holding a pose: a limb is one box, so a knee or an elbow
/// bends nowhere and only how far each leg and each arm swings out to its
/// side is kept. A limb the pose leaves be hangs.
fn holding(motion: &Motion, held: &Held) -> Vec<BoxPart> {
    let swing = |bone: &str, rest: DVec3| swing(held, bone, rest);
    let legs = legs(held);
    // At rest the humanoid's arms are out to its sides, its left along `-x`.
    let arms = [
        swing("leftUpperArm", DVec3::NEG_X),
        swing("rightUpperArm", DVec3::X),
    ];
    let drop_m = LEG_M - hips_m(held);
    let hips = held
        .bone("hips")
        .map_or(DQuat::IDENTITY, |turn| turn.as_dquat());
    let body = DAffine3::from_mat3_translation(motion.basis, motion.position)
        * DAffine3::from_translation(DVec3::new(0.0, -drop_m, 0.0))
        * DAffine3::from_quat(hips);
    PARTS
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let swung = match index {
                2 => legs[0],
                3 => legs[1],
                4 => arms[0],
                5 => arms[1],
                _ => DQuat::IDENTITY,
            };
            let local = DAffine3::from_translation(part.pivot)
                * DAffine3::from_quat(swung)
                * DAffine3::from_translation(part.offset)
                * DAffine3::from_scale(part.size);
            BoxPart {
                transform: body * local,
                color: part.color,
                shine: 0.0,
                shadow_only: false,
                sheet: false,
            }
        })
        .collect()
}

/// How high the hips stand over the feet holding a pose, metres: the legs
/// apart lower them.
pub(crate) fn hips_m(held: &Held) -> f64 {
    let [left, right] = legs(held);
    let spread = left.to_axis_angle().1.max(right.to_axis_angle().1);
    LEG_M * spread.cos()
}

/// How far each upper leg swings out holding a pose. At rest the humanoid
/// stands with its legs down.
fn legs(held: &Held) -> [DQuat; 2] {
    [
        swing(held, "leftUpperLeg", DVec3::NEG_Y),
        swing(held, "rightUpperLeg", DVec3::NEG_Y),
    ]
}

/// A bone turned from `rest`, its direction at rest in the figure's frame,
/// as the swing about `z` that points a hanging box along it.
fn swing(held: &Held, bone: &str, rest: DVec3) -> DQuat {
    held.bone(bone).map_or(DQuat::IDENTITY, |turn| {
        let to = turn.as_dquat() * rest;
        DQuat::from_rotation_z(to.x.atan2(-to.y))
    })
}

#[cfg(test)]
mod tests {
    use glam::{DMat3, Quat};

    use super::*;

    fn standing() -> Motion {
        Motion {
            position: DVec3::ZERO,
            basis: DMat3::IDENTITY,
            gait: Gait::Idle,
            speed_mps: 0.0,
            sprint: false,
            stride_m: 0.0,
        }
    }

    /// Which way the box of a limb hangs from its joint.
    fn hangs(parts: &[BoxPart], index: usize) -> DVec3 {
        -parts[index].transform.matrix3.y_axis.normalize()
    }

    #[test]
    fn a_held_pose_points_the_arms_where_its_bones_do_and_one_that_leaves_them_hangs_them() {
        let lowered = Held {
            bones: vec![
                ("leftUpperArm", Quat::from_rotation_z(0.6)),
                ("rightUpperArm", Quat::from_rotation_z(-0.6)),
            ],
            hips: Vec3::Y,
        };
        let boxes = parts(&standing(), Some(&lowered));
        let (out, down) = (0.6_f64.cos(), 0.6_f64.sin());
        assert!(hangs(&boxes, 4).abs_diff_eq(DVec3::new(-out, -down, 0.0), 1e-6));
        assert!(hangs(&boxes, 5).abs_diff_eq(DVec3::new(out, -down, 0.0), 1e-6));

        let legs_only = Held {
            bones: vec![("leftUpperLeg", Quat::from_rotation_z(-0.3))],
            hips: Vec3::Y,
        };
        let boxes = parts(&standing(), Some(&legs_only));
        for arm in [4, 5] {
            assert!(hangs(&boxes, arm).abs_diff_eq(DVec3::NEG_Y, 1e-9));
        }
    }
}
