//! Wings of boxes: data any flier fills, a [`Wing`], and a pair of them on
//! an animal, [`Wings`], snapped open as it takes off, beaten as hard as its
//! flight asks, held out to glide and folded away as it lands.
//!
//! A wing is three bones from the shoulder, arm, forearm and hand, along a
//! leading edge thick at the shoulder and thin at the tip, and rows of
//! feathers laid back from them: long flight feathers, and rows of shorter
//! coverts over their roots. Each feather lies in its bone's plane a little
//! over the one before, as shingles lie, and is wider than the gap to the
//! next, so no daylight shows through from above or below. On the upstroke
//! the hand folds back at the wrist and its feathers close; gliding, the
//! primaries part at their tips like fingers.

use core::f64::consts::TAU;

use glam::{DAffine3, DMat3, DVec3, Vec3};
use scene::BoxPart;

use crate::fauna::ease::smoothstep;

/// A wing as a kind has it: where it meets the body, its bones, edge and
/// feathers, how it beats and how fast it spreads. The right wing, in the
/// rig's space at size 1; the left mirrors it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Wing {
    /// Where it meets the body.
    pub shoulder: DVec3,
    /// The bones, metres: arm, forearm, hand.
    pub bones_m: [f64; 3],
    /// The leading edge along each bone: how deep back from it, and how
    /// thick, metres.
    pub edge_m: [[f64; 2]; 3],
    /// The rows of feathers, the lowest first: each lies over the one
    /// before.
    pub rows: &'static [Row],
    /// Beats a second, held out and beating hard.
    pub beat_hz: (f64, f64),
    /// How far a beat swings the wing up and down, radians, held out and
    /// beating hard.
    pub stroke: (f64, f64),
    /// How long it takes to open, and to fold away, seconds.
    pub open_s: f64,
    pub fold_s: f64,
    /// How far a hard beat lifts the body, metres.
    pub heave_m: f64,
}

/// A row of feathers: where along the wing it grows, how many and how big,
/// and which way they lie.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Row {
    /// Where along the wing it grows, from and to: 0 the shoulder, 1 the
    /// elbow, 2 the wrist, 3 the tip.
    pub along: [f64; 2],
    pub count: usize,
    /// How long its first feather is and its last, metres.
    pub long_m: [f64; 2],
    /// How wide and thick each is, metres: wider than the row's length
    /// over its count, so each overlaps the next.
    pub wide_m: f64,
    pub thick_m: f64,
    /// How far each turns from straight back toward the tip, radians, at
    /// the first and the last.
    pub fan: [f64; 2],
    /// How far apart its tips part gliding, radians between neighbours.
    pub fingers: f64,
    /// How far over the wing's plane it lies, metres.
    pub over_m: f64,
    /// Which of the wing's colours it takes: coverts, secondaries,
    /// primaries.
    pub paint: usize,
}

/// The colours of a wing: its coverts and edge, its secondaries, its
/// primaries.
pub type WingPaint = [Vec3; 3];

/// How fast the effort comes after what the flight asks, per second.
const EFFORT_PER_S: f64 = 3.0;

/// Between these efforts the wings go from held out to beating: a beat is
/// heard from 0.2.
const HELD_EFFORT: (f64, f64) = (0.05, 0.3);
/// How far up the wing is held, gliding, radians: a shallow dihedral.
const DIHEDRAL: f64 = 0.1;
/// How far each bone sweeps back as the wing folds away, radians.
const FOLD_SWEEP: [f64; 3] = [1.1, 2.2, 1.0];
/// Each bone's sweep back held out, radians: the arm back, the forearm a
/// little forward, the hand back again.
const SWEEP: [f64; 3] = [0.25, -0.12, 0.2];
/// How far back the hand folds at the wrist on the upstroke, radians, and
/// how far the forearm follows it.
const WRIST_FOLD: f64 = 1.15;
const ELBOW_FOLD: f64 = 0.45;
/// A feather's rounded tip: a narrower cap past it, as long and as wide
/// against its width.
const CAP: [f64; 2] = [0.3, 0.6];
/// How far each feather of a row lies over the one before, against its
/// thickness.
const SHINGLE: f64 = 0.3;
/// The share of a beat where the downstroke strikes: the wing at its
/// middle, coming down fastest.
const STRIKE: f64 = 0.5;
/// Where in a beat wings open from folded: raised, so the first downstroke
/// takes the air.
const OPEN_BEAT: f64 = 0.25;
/// Opening, the feathers fan out in this share of the time the bones take.
const FAN_LEAD: f64 = 0.6;

/// A pair of wings on an animal: how far open and where in a beat.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Wings {
    pub wing: &'static Wing,
    /// How far out, 0 folded away to 1 spread.
    pub grown: f64,
    /// How far its feathers are fanned out, 0 closed to 1 spread: ahead of
    /// the bones as they open, with them as they fold.
    pub fanned: f64,
    /// Where in a beat, in beats: up at a quarter, down at three.
    pub beat: f64,
    /// How hard they beat, 0 held out to glide to 1 beating hard.
    pub effort: f64,
}

impl Wings {
    /// Folded away.
    pub fn new(wing: &'static Wing) -> Wings {
        Wings {
            wing,
            grown: 0.0,
            fanned: 0.0,
            beat: 0.0,
            effort: 0.0,
        }
    }

    /// Opens them while `out`, else folds them away, and beats them as
    /// hard as the flight asks, `want` from 0, held, to 1, hard. Says
    /// whether a downstroke struck: a beat hard enough to be heard passing
    /// the middle of its stroke going down, while they are out. Folding
    /// away, they settle unheard: the animal has landed.
    pub fn update(&mut self, dt: f64, out: bool, want: f64) -> bool {
        if out && self.grown <= 0.0 {
            self.beat = OPEN_BEAT;
        }
        let rate = match out {
            true => dt / self.wing.open_s,
            false => -dt / self.wing.fold_s,
        };
        self.grown = (self.grown + rate).clamp(0.0, 1.0);
        self.fanned = match out {
            true => (self.fanned + rate / FAN_LEAD).clamp(self.grown, 1.0),
            false => self.grown,
        };
        let want = if out { want.clamp(0.0, 1.0) } else { 0.0 };
        self.effort += (want - self.effort) * (1.0 - libm::exp(-EFFORT_PER_S * dt));
        let (held, hard) = self.wing.beat_hz;
        let was = self.beat;
        // A wing held out still stirs: a slow beat, a little stroke.
        self.beat += dt * (held + (hard - held) * self.effort) * self.grown;
        let struck = libm::floor(self.beat - STRIKE) > libm::floor(was - STRIKE);
        struck && out && self.grown > 0.5 && self.effort > 0.2
    }

    /// How far they are held out to glide, 0 to 1: grown out and beating
    /// no more than a stir. It rises as the beats stop and falls as they
    /// come back or the wings fold.
    pub fn held_out(&self) -> f64 {
        self.grown * (1.0 - smoothstep(HELD_EFFORT.0, HELD_EFFORT.1, self.effort))
    }

    /// How far a beat lifts the body now, metres at size 1: up on the
    /// downstroke.
    pub fn heave(&self) -> f64 {
        -self.wing.heave_m * self.effort * self.grown * libm::cos(TAU * self.beat)
    }

    /// Their boxes, from `body`, the rig's space to world, appended to
    /// `out`: the shoulders `build` apart as the body is broad. None while
    /// folded away.
    pub fn boxes(&self, body: DAffine3, build: f64, paint: WingPaint, out: &mut Vec<BoxPart>) {
        if self.grown <= 0.0 {
            return;
        }
        let wing = self.wing;
        let grow = smoothstep(0.0, 1.0, self.grown);
        let fold = 1.0 - grow;
        let fan = smoothstep(0.0, 1.0, self.fanned);
        let flap = libm::sin(TAU * self.beat);
        let (held, hard) = wing.stroke;
        let stroke = held + (hard - held) * self.effort;
        // Rising, the hand folds back at the wrist and its feathers close;
        // gliding, they part at the tips.
        let flex = self.effort * libm::cos(TAU * self.beat).max(0.0);
        let glide = 1.0 - self.effort;
        let lift = DIHEDRAL + stroke * flap;
        let rises = [
            lift,
            lift + 0.15 * stroke * flap,
            1.1 * lift + 0.25 * stroke * flap - 0.3 * flex,
        ];
        let sweeps = [
            SWEEP[0],
            SWEEP[1] + ELBOW_FOLD * flex,
            SWEEP[2] + WRIST_FOLD * flex,
        ];
        for side in [-1.0, 1.0] {
            let mut put = |transform: DAffine3, color: Vec3| {
                out.push(BoxPart {
                    transform: body * transform,
                    color,
                    shine: 0.0,
                    shadow_only: false,
                    sheet: true,
                });
            };
            // Folding away, each bone sweeps back along the flank, lies
            // flat and shrinks into the shoulder.
            let ways: [DVec3; 3] = core::array::from_fn(|bone| {
                let (sweep, rise) = (sweeps[bone] + fold * FOLD_SWEEP[bone], rises[bone] * grow);
                DVec3::new(
                    side * libm::cos(rise) * libm::cos(sweep),
                    libm::sin(rise),
                    libm::cos(rise) * libm::sin(sweep),
                )
            });
            let shoulder = DVec3::new(
                side * wing.shoulder.x * build,
                wing.shoulder.y,
                wing.shoulder.z,
            );
            let mut joints = [shoulder; 4];
            for bone in 0..3 {
                joints[bone + 1] = joints[bone] + ways[bone] * wing.bones_m[bone] * grow;
            }
            // Out of the wing's plane along a bone, and back from it in
            // that plane; folded `folded`, what lies back comes to lie
            // along it.
            let normal = |way: DVec3| (DVec3::Y - way * way.y).normalize_or(DVec3::Y);
            let back = |way: DVec3, folded: f64| {
                (way.cross(normal(way)) * side)
                    .normalize_or(DVec3::Z)
                    .lerp(way, folded)
                    .normalize_or(DVec3::Z)
            };
            for bone in 0..3 {
                let way = ways[bone];
                let [deep, thick] = wing.edge_m[bone].map(|m| m * grow);
                let long = wing.bones_m[bone] * grow;
                let from = joints[bone] + back(way, fold) * deep * 0.45 - way * thick;
                put(
                    plate(from, way, normal(way), long + 2.0 * thick, deep, thick),
                    paint[0],
                );
            }
            for row in wing.rows {
                let [first, last] = row.along;
                for k in 0..row.count {
                    let t = match row.count {
                        1 => 0.5,
                        n => k as f64 / (n - 1) as f64,
                    };
                    let along = first + (last - first) * t;
                    let bone = (libm::floor(along) as usize).min(2);
                    let at = joints[bone].lerp(joints[bone + 1], along - bone as f64);
                    let way = ways[bone];
                    let plane = normal(way);
                    let turn = row.fan[0] + (row.fan[1] - row.fan[0]) * t;
                    let spread =
                        (turn * (1.0 - 0.45 * flex) + row.fingers * k as f64 * glide) * fan;
                    let back = back(way, 1.0 - fan);
                    let toward = (back * libm::cos(spread) + way * libm::sin(spread)).normalize();
                    let (wide, thick) = (row.wide_m * fan, row.thick_m * fan);
                    let from = at + plane * (row.over_m * fan + SHINGLE * thick * k as f64);
                    let long = (row.long_m[0] + (row.long_m[1] - row.long_m[0]) * t) * fan;
                    let color = paint[row.paint.min(2)];
                    put(plate(from, toward, plane, long, wide, thick), color);
                    let [cap_long, cap_wide] = CAP;
                    put(
                        plate(
                            from + toward * long,
                            toward,
                            plane,
                            wide * cap_long,
                            wide * cap_wide,
                            thick,
                        ),
                        color,
                    );
                }
            }
        }
    }
}

/// A flat box from `from` along `toward` for `length`, `width` across it in
/// the plane whose up is nearest `up`, and `thick` through it.
fn plate(from: DVec3, toward: DVec3, up: DVec3, length: f64, width: f64, thick: f64) -> DAffine3 {
    let along = toward.normalize_or(DVec3::X);
    let normal = (up - along * along.dot(up)).normalize_or(DVec3::Y);
    let across = along.cross(normal);
    DAffine3::from_mat3_translation(
        DMat3::from_cols(along * length, normal * thick, across * width),
        from + along * length / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use crate::fauna::horse::{HORSE, WING};

    use super::*;

    /// Spread and held out, how far apart the furthest corners of its
    /// boxes reach across the body, metres at size 1.
    fn span(wings: &Wings) -> f64 {
        let mut boxes = Vec::new();
        wings.boxes(DAffine3::IDENTITY, 1.0, [Vec3::ONE; 3], &mut boxes);
        let reach = boxes
            .iter()
            .flat_map(|part| {
                [-0.5, 0.5].into_iter().flat_map(move |x| {
                    [-0.5, 0.5].into_iter().flat_map(move |y| {
                        [-0.5, 0.5]
                            .into_iter()
                            .map(move |z| part.transform.transform_point3(DVec3::new(x, y, z)).x)
                    })
                })
            })
            .fold(0.0f64, |most, x| most.max(x.abs()));
        2.0 * reach
    }

    /// A pegasus's wings span about half again its length, held out to
    /// glide.
    #[test]
    fn a_pegasus_spans_half_again_its_length() {
        let wings = Wings {
            grown: 1.0,
            fanned: 1.0,
            beat: 0.0,
            ..Wings::new(&WING)
        };
        let length = HORSE.length_m() / 1.16;
        let span = span(&wings);
        assert!(
            (1.4 * length..1.6 * length).contains(&span),
            "span {span:.2} m against a length of {length:.2} m"
        );
    }

    /// In the wing's shape, each feather of a row is wider than the spacing
    /// along the row, with a tenth to spare, so the row lies shingled. That
    /// it shows no daylight posed is seen in `bun run shot --wings`.
    #[test]
    fn a_rows_feathers_are_wider_than_their_spacing() {
        for row in WING.rows {
            let reach: f64 = (row.along[1] - row.along[0]) / 3.0
                * WING.bones_m.iter().copied().fold(0.0, f64::max);
            let gap = reach / (row.count - 1) as f64;
            assert!(row.wide_m > gap * 1.1, "{row:?}: {gap:.3} m apart");
        }
    }

    /// Out, they open in the time the wing says, their feathers fanned
    /// ahead of the bones, raised, so the first downstroke strikes within
    /// a beat; a beat hard enough strikes once a beat; held out to glide,
    /// none strikes. They fold in their own time, the feathers with the
    /// bones. Folded, none is drawn.
    #[test]
    fn they_open_beat_and_fold_away() {
        let mut wings = Wings::new(&WING);
        let mut boxes = Vec::new();
        wings.boxes(DAffine3::IDENTITY, 1.0, [Vec3::ONE; 3], &mut boxes);
        assert!(boxes.is_empty());
        let dt = 1.0 / 60.0;
        let mut strikes = 0;
        let mut first = None;
        for frame in 0..600 {
            let struck = wings.update(dt, true, 1.0);
            strikes += usize::from(struck);
            if struck && first.is_none() {
                first = Some(frame as f64 * dt);
            }
            if frame == 0 {
                assert!(wings.fanned > wings.grown, "{wings:?}");
            }
            if frame == (WING.open_s / dt).ceil() as usize {
                assert_eq!((wings.grown, wings.fanned), (1.0, 1.0));
            }
        }
        let first = first.expect("a first downstroke");
        assert!(
            first < 1.0 / WING.beat_hz.1 * 0.75,
            "first strikes at {first:.2} s"
        );
        let beats = wings.beat;
        assert!(
            (strikes as f64 - beats).abs() <= 2.0,
            "{strikes} in {beats:.1}"
        );
        let mut gliding = 0;
        for _ in 0..600 {
            gliding += usize::from(wings.update(dt, true, 0.0));
        }
        assert!(gliding <= 2, "{gliding} strikes gliding");
        for frame in 0..(WING.fold_s / dt).ceil() as usize {
            wings.update(dt, false, 0.0);
            assert_eq!(wings.fanned, wings.grown, "folding, frame {frame}");
            assert!(wings.grown > 0.0 || frame + 1 >= (WING.fold_s / dt) as usize);
        }
        assert_eq!(wings.grown, 0.0);
    }

    /// Held out to glide is grown out and still: none folded, none beating
    /// hard, all of it gliding, and it rises as the beats stop and falls as
    /// they come back.
    #[test]
    fn held_out_rises_as_the_beats_stop_and_falls_as_they_return() {
        let dt = 1.0 / 60.0;
        let mut wings = Wings::new(&WING);
        assert_eq!(wings.held_out(), 0.0, "folded");
        for _ in 0..300 {
            wings.update(dt, true, 1.0);
        }
        assert_eq!(wings.held_out(), 0.0, "beating hard");
        let mut last = 0.0;
        for _ in 0..300 {
            wings.update(dt, true, 0.0);
            let held_out = wings.held_out();
            assert!(held_out >= last, "{last} to {held_out}");
            last = held_out;
        }
        assert!(last > 0.99, "gliding: {last}");
        for _ in 0..300 {
            wings.update(dt, true, 1.0);
            let held_out = wings.held_out();
            assert!(held_out <= last, "{last} to {held_out}");
            last = held_out;
        }
        assert_eq!(last, 0.0, "beating again");
    }
}
