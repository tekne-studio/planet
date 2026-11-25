//! Two bones that reach for a point: a leg from its root to its foot.

use glam::DVec3;

/// A leg solved: where its joint went and where its end landed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reached {
    pub joint: DVec3,
    pub end: DVec3,
}

/// How much of their length two bones ease into straight over: a leg
/// almost straight bends its joint by a hair's move of its end, so it
/// never quite straightens, and stretches instead.
const SOFT: f64 = 0.06;

/// Two bones of `upper` and `lower` metres from `root` toward `target`, the
/// joint between them bent toward `pole`. Near straight, the bones ease
/// toward it and stretch to keep the end on the target, so the joint moves
/// as smoothly as the end. A target out of reach stretches both bones, up
/// to `stretch` times their length, before the end falls short of it: in
/// boxes a few percent longer is not seen, a floating hoof is.
pub(crate) fn solve(
    root: DVec3,
    target: DVec3,
    upper: f64,
    lower: f64,
    pole: DVec3,
    stretch: f64,
) -> Reached {
    let to = target - root;
    let distance = to.length().max(1e-6);
    let along = to / distance;
    // How far out the end sits, of the bones' whole length: the distance,
    // until it eases toward straight and never gets there. The bones grow
    // to keep the end on the target, and past `stretch` it falls short.
    let length = upper + lower;
    let out = distance / length;
    let eased = match out > 1.0 - SOFT {
        true => 1.0 - SOFT * libm::exp(-(out - 1.0 + SOFT) / SOFT),
        false => out,
    };
    let scale = (out / eased).clamp(1.0, stretch);
    let (a, b) = (upper * scale, lower * scale);
    let d = (eased * (a + b)).max((a - b).abs() + 1e-4);
    let cos = ((a * a + d * d - b * b) / (2.0 * a * d)).clamp(-1.0, 1.0);
    let sin = (1.0 - cos * cos).sqrt();
    let bend = (pole - along * pole.dot(along)).normalize_or(along.any_orthonormal_vector());
    Reached {
        joint: root + along * (a * cos) + bend * (a * sin),
        end: root + along * d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bones_keep_their_length_and_bend_toward_the_pole() {
        let root = DVec3::new(0.0, 1.0, 0.0);
        let target = DVec3::new(0.1, 0.1, 0.2);
        let pole = DVec3::NEG_Z;
        let reached = solve(root, target, 0.5, 0.55, pole, 1.0);
        assert!(((reached.joint - root).length() - 0.5).abs() < 1e-9);
        assert!(((reached.end - reached.joint).length() - 0.55).abs() < 1e-9);
        assert!((reached.end - target).length() < 1e-9);
        let mid = (root + target) / 2.0;
        assert!((reached.joint - mid).dot(pole) > 0.0);
    }

    /// Eased toward straight, the joint moves no faster than the end does
    /// as far as the bones reach, and past it: it never snaps straight.
    #[test]
    fn near_straight_the_joint_moves_as_smoothly_as_the_end() {
        let root = DVec3::new(0.0, 1.0, 0.0);
        let at = |y: f64| solve(root, DVec3::new(0.0, y, 0.0), 0.5, 0.5, DVec3::Z, 1.08);
        let step = 0.001;
        let mut y = 0.3;
        while y > -0.3 {
            let (here, next) = (at(y), at(y - step));
            let moved = (next.joint - here.joint).length();
            assert!(moved < 3.0 * step, "at {y}: the joint moves {moved} m");
            let short = (here.end - DVec3::new(0.0, y, 0.0)).length();
            assert!(y < -0.07 || short < 1e-9, "at {y}: {short} m short");
            y -= step;
        }
    }

    #[test]
    fn out_of_reach_it_stretches_then_falls_short() {
        let root = DVec3::new(0.0, 1.0, 0.0);
        let near = solve(root, DVec3::new(0.0, -0.03, 0.0), 0.5, 0.5, DVec3::Z, 1.05);
        assert!((near.end - DVec3::new(0.0, -0.03, 0.0)).length() < 1e-6);
        let far = solve(root, DVec3::new(0.0, -0.5, 0.0), 0.5, 0.5, DVec3::Z, 1.05);
        assert!(far.end.y > -0.5 + 0.4);
    }
}
