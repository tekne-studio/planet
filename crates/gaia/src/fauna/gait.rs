//! A gait: when in a stride each foot is down, how high it lifts, how the
//! body rides it. A kind lists its gaits from the slowest; the speed blends
//! one into the next.

use core::f64::consts::PI;

use crate::fauna::ease::smoothstep;
use crate::fauna::rig::LEGS;

/// One gait of a kind, as data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gait {
    /// The speed it is wholly in, metres a second at size 1, and over how
    /// many metres a second below that it blends in from the gait before.
    /// The first gait's are never read.
    pub from_mps: f64,
    pub blend_mps: f64,
    /// Strides a second standing still, and how many more each metre a
    /// second adds: a faster animal lengthens its stride more than it
    /// quickens it.
    pub hz: f64,
    pub hz_per_mps: f64,
    /// The furthest a foot goes under the body while it is down, metres:
    /// what the legs reach. Past it the stride quickens, as an animal's
    /// does, never lengthens.
    pub stance_m: f64,
    /// Of a stride, how much each foot is down.
    pub duty: f64,
    /// Where in a stride each foot lands, strides, in the rig's order of
    /// legs. Unwrapped so that one gait blends into the next without a leg
    /// going round the long way.
    pub lands: [f64; LEGS],
    /// How high a foot lifts in the middle of its swing, metres.
    pub lift_m: f64,
    /// How far the body rises and falls, metres, and how many times a stride.
    pub bounce_m: f64,
    pub bounces: f64,
    /// Where in a stride the body is lowest and its nose too, strides.
    pub sink: f64,
    /// How far the body rocks nose up and down, radians, once a stride.
    pub rock: f64,
    /// How far the neck nods, radians, and how many times a stride.
    pub nod: f64,
    pub nods: f64,
    /// The neck over the horizontal and the head under it, radians.
    pub neck: f64,
    pub head: f64,
    /// How far the tail lifts off hanging, 0 to 1.
    pub tail: f64,
    /// Where in a stride its beats are heard, strides: hooves that land
    /// together, or nearly, are one beat. Between two gaits, the nearer's.
    pub beats: &'static [f64],
    /// How hard its feet come down, 0 to 1: how loud a beat is. Between two
    /// gaits, the nearer's, so a beat is heard at one of a few loudnesses.
    pub force: f64,
}

/// A kind's gaits, from the slowest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gaits(pub &'static [Gait]);

/// Where a foot is in its stride, relative to where it stands at rest.
#[derive(Clone, Copy, Debug)]
pub struct Hoof {
    /// Ahead of rest, metres.
    pub along_m: f64,
    /// Off the ground, metres.
    pub lift_m: f64,
    pub down: bool,
    /// How far through its swing, 0 to 1; 0 while down.
    pub swing: f64,
}

impl Gaits {
    /// The gait at a pace, metres a second at size 1: each faster gait
    /// blended in over the speeds below its own.
    pub fn at(&self, pace: f64) -> Gait {
        let mut gaits = self.0.iter();
        let first = *gaits.next().expect("a kind has a gait");
        gaits.fold(first, |gait, next| {
            let t = smoothstep(next.from_mps - next.blend_mps, next.from_mps, pace);
            gait.mix(next, t)
        })
    }
}

impl Gait {
    /// Strides a second at a pace: quick enough that no foot is down
    /// longer than its stance.
    pub fn hz_at(&self, pace: f64) -> f64 {
        (self.hz + self.hz_per_mps * pace).max(pace * self.duty / self.stance_m)
    }

    /// How fast a swinging foot goes past the body at a pace, metres a
    /// second at size 1: as far as the body goes while a foot is down, in
    /// the time a foot is up.
    pub fn swing_mps(&self, pace: f64) -> f64 {
        pace * self.duty / (1.0 - self.duty)
    }

    /// Where a leg's foot is, `stride` strides along, at a pace. Down, it
    /// moves back under the body as fast as the body moves forward, so it
    /// stays where it landed; up, it swings forward to land again.
    pub fn hoof(&self, leg: usize, stride: f64, pace: f64) -> Hoof {
        let t = (stride - self.lands[leg]).rem_euclid(1.0);
        // How far the body goes while a foot is down.
        let reach = pace * self.duty / self.hz_at(pace);
        let moving = smoothstep(0.02, 0.5, pace);
        if t < self.duty {
            let s = t / self.duty;
            Hoof {
                along_m: reach * (0.5 - s),
                lift_m: 0.0,
                down: true,
                swing: 0.0,
            }
        } else {
            let s = (t - self.duty) / (1.0 - self.duty);
            let ease = s * s * (3.0 - 2.0 * s);
            Hoof {
                along_m: reach * (ease - 0.5),
                lift_m: self.lift_m * moving * libm::sin(PI * s),
                down: false,
                swing: s,
            }
        }
    }

    fn mix(&self, other: &Gait, t: f64) -> Gait {
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        Gait {
            from_mps: self.from_mps,
            blend_mps: self.blend_mps,
            hz: lerp(self.hz, other.hz),
            hz_per_mps: lerp(self.hz_per_mps, other.hz_per_mps),
            stance_m: lerp(self.stance_m, other.stance_m),
            duty: lerp(self.duty, other.duty),
            lands: core::array::from_fn(|leg| lerp(self.lands[leg], other.lands[leg])),
            lift_m: lerp(self.lift_m, other.lift_m),
            bounce_m: lerp(self.bounce_m, other.bounce_m),
            bounces: lerp(self.bounces, other.bounces),
            sink: lerp(self.sink, other.sink),
            rock: lerp(self.rock, other.rock),
            nod: lerp(self.nod, other.nod),
            nods: lerp(self.nods, other.nods),
            neck: lerp(self.neck, other.neck),
            head: lerp(self.head, other.head),
            tail: lerp(self.tail, other.tail),
            beats: if t < 0.5 { self.beats } else { other.beats },
            force: if t < 0.5 { self.force } else { other.force },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fauna::horse::HORSE;

    use super::*;

    /// A foot that is down does not slide: it moves back under the body as
    /// fast as the body moves forward.
    #[test]
    fn a_foot_down_stays_put() {
        for pace in [1.5, 3.6, 9.0, 12.5] {
            let gait = HORSE.gaits.at(pace);
            let hz = gait.hz_at(pace);
            let dt = 1e-3;
            for leg in 0..LEGS {
                let stride = gait.lands[leg] + gait.duty * 0.5;
                let a = gait.hoof(leg, stride, pace);
                let b = gait.hoof(leg, stride + dt * hz, pace);
                assert!(a.down && b.down);
                let slip = (b.along_m - a.along_m) + pace * dt;
                assert!(slip.abs() < 1e-9, "{pace} m/s, leg {leg}: {slip}");
            }
        }
    }

    /// The horse gallops "po-co-tó": a hind alone, the other hind and the
    /// diagonal fore almost together, the leading fore alone, then a flight
    /// with no hoof down.
    #[test]
    fn the_gallop_says_po_co_to() {
        let pace = 9.0;
        let gait = HORSE.gaits.at(pace);
        let [lf, rf, lh, rh] = gait.lands;
        let (po, co, co2, to) = (lh, rh, lf, rf);
        assert!(po < co && co < co2 && co2 < to, "{:?}", gait.lands);
        let short = co - po;
        assert!(co2 - co < short / 2.0, "co is a pair: {:?}", gait.lands);
        assert!(
            (to - co2 - short).abs() < 0.05,
            "short, short: {:?}",
            gait.lands
        );
        let down = |stride: f64| {
            (0..LEGS)
                .filter(|&leg| gait.hoof(leg, stride, pace).down)
                .count()
        };
        let flight = (0..1000)
            .filter(|&i| down(f64::from(i) / 1000.0) == 0)
            .count();
        assert!(
            (150..350).contains(&flight),
            "{} of a stride in the air",
            f64::from(flight as u32) / 1000.0
        );
    }

    /// The horse walks in four even beats, never fewer than two hooves down
    /// and three most of the time.
    #[test]
    fn the_walk_keeps_three_hooves_down() {
        let pace = 1.5;
        let gait = HORSE.gaits.at(pace);
        let down = |stride: f64| {
            (0..LEGS)
                .filter(|&leg| gait.hoof(leg, stride, pace).down)
                .count()
        };
        let counts: Vec<usize> = (0..1000).map(|i| down(f64::from(i) / 1000.0)).collect();
        assert!(counts.iter().all(|&n| n >= 2));
        let three = counts.iter().filter(|&&n| n >= 3).count();
        assert!(three > 600, "{three} of 1000 with three down");
    }

    /// The horse trots in diagonal pairs.
    #[test]
    fn the_trot_pairs_the_diagonals() {
        let pace = 3.6;
        let gait = HORSE.gaits.at(pace);
        for i in 0..20 {
            let stride = f64::from(i) * 0.05;
            let [lf, rf, lh, rh] = core::array::from_fn(|leg| gait.hoof(leg, stride, pace).along_m);
            assert!((lf - rh).abs() < 1e-9 && (rf - lh).abs() < 1e-9);
        }
    }
}
