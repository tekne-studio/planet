//! Wings of boxes: data any flier fills, a [`Wing`], and a pair of them on
//! an animal, [`Wings`], unfolded from its flanks as it takes off, beaten
//! as hard as its flight asks, held out to glide and folded back away as it
//! lands.
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

use glam::{DAffine3, DMat3, DQuat, DVec3, Vec3};
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

/// Folded at rest, as a bird's, the right wing in the rig: in a plane along
/// the flank whose foot leans out from upright by `FOLD_LEAN`, radians. Each
/// bone's way, back along the body, down that plane and out of it: the arm
/// back from the shoulder, and out as far as the body is broad, so the
/// plane clears the barrel at every build; the forearm forward and down,
/// so the wrist sits under the shoulder; the hand back along the flank.
const FOLD_LEAN: f64 = 0.2;
const FOLDED: [[f64; 3]; 3] = [[1.0, 0.12, 0.27], [-0.74, 0.67, 0.0], [0.97, -0.26, 0.0]];
/// Folded, its feathers lie back toward the tail in that plane, this far
/// down it for each unit back.
const TAIL_DOWN: f64 = 0.05;
/// Coming out, how far the folded wing lifts off the flank, turned about
/// the body's length, before it swings out, radians.
const LIFT: f64 = 0.9;
/// Coming out, the share of the way from gone to spread where each part
/// moves, from and to: it grows from nothing at the shoulder, folded along
/// the flank, and goes on growing as it lifts, the shoulder swings it out
/// as the elbow and the wrist straighten after it, and the feathers fan
/// last, the tip trailing. Folding away plays it back, shrinking into the
/// shoulder as it folds.
const GROWS: [f64; 2] = [0.0, 0.7];
const LIFTS: [f64; 2] = [0.1, 0.42];
const JOINTS: [[f64; 2]; 3] = [[0.25, 0.66], [0.3, 0.8], [0.4, 0.92]];
const FANS: [f64; 2] = [0.58, 1.0];

/// How far along a wing is in coming out, each part 0 folded to 1 spread:
/// one description, read opening and folding alike, of how far out it is.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Out {
    /// How big it has grown, from nothing to its full size.
    size: f64,
    /// How far it has lifted off the flank, folded.
    lift: f64,
    /// How far its shoulder, elbow and wrist have opened.
    joints: [f64; 3],
    /// How far its feathers have fanned.
    fan: f64,
}

impl Out {
    /// How far each part is out when the wing is `grown` out.
    fn of(grown: f64) -> Out {
        let turn = |[from, to]: [f64; 2]| smoothstep(from, to, grown);
        Out {
            size: turn(GROWS),
            lift: turn(LIFTS),
            joints: JOINTS.map(turn),
            fan: turn(FANS),
        }
    }
}

/// A wing's bones as drawn, the right one's in the rig.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Pose {
    /// Each bone's turn: its length along `x`, the wing's up along `y` and
    /// back from its leading edge along `z`.
    turns: [DQuat; 3],
    /// Its joints: the shoulder, the elbow, the wrist and the tip.
    at: [DVec3; 4],
    /// How far the hand folds back rising, 0 to 1.
    flex: f64,
}

/// A pair of wings on an animal: how far open and where in a beat.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Wings {
    pub wing: &'static Wing,
    /// How far out, 0 gone to 1 spread: it comes out folded, unfolds and
    /// fans over the wing's `open_s`, and folds away over its `fold_s` the
    /// same way back.
    pub grown: f64,
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
        // Steps of a frame do not sum to one exactly: what is left of a
        // wing folded away is nothing, so it is drawn no more.
        if !out && self.grown < 1e-9 {
            self.grown = 0.0;
        }
        let want = if out { want.clamp(0.0, 1.0) } else { 0.0 };
        self.effort += (want - self.effort) * (1.0 - libm::exp(-EFFORT_PER_S * dt));
        let (held, hard) = self.wing.beat_hz;
        let was = self.beat;
        // A wing held out still stirs: a slow beat, a little stroke. One
        // coming out or folding away holds where it is in its beat, so it
        // opens raised for the first downstroke.
        let spread = f64::from(u8::from(self.grown >= 1.0));
        self.beat += dt * (held + (hard - held) * self.effort) * spread;
        let struck = libm::floor(self.beat - STRIKE) > libm::floor(was - STRIKE);
        struck && out && self.effort > 0.2
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
        let Pose { turns, at, flex } = self.pose(build);
        let Out { size, fan, .. } = Out::of(self.grown);
        let (_, tail) = folded(build);
        let glide = 1.0 - self.effort;
        for side in [-1.0, 1.0] {
            // The left wing is the right one mirrored.
            let mirror = |v: DVec3| DVec3::new(side * v.x, v.y, v.z);
            let mut put = |from: DVec3,
                           toward: DVec3,
                           up: DVec3,
                           [long, wide, thick]: [f64; 3],
                           color: Vec3| {
                out.push(BoxPart {
                    transform: body
                        * plate(mirror(from), mirror(toward), mirror(up), long, wide, thick),
                    color,
                    shine: 0.0,
                    shadow_only: false,
                    sheet: true,
                });
            };
            for bone in 0..3 {
                let turn = turns[bone];
                let (way, normal, back) = (turn * DVec3::X, turn * DVec3::Y, turn * DVec3::Z);
                let [deep, thick] = wing.edge_m[bone].map(|m| m * size);
                let long = wing.bones_m[bone] * size;
                let from = at[bone] + back * deep * 0.45 - way * thick;
                put(
                    from,
                    way,
                    normal,
                    [long + 2.0 * thick, deep, thick],
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
                    let base = at[bone].lerp(at[bone + 1], along - bone as f64);
                    let turn = turns[bone];
                    let (way, plane, back) = (turn * DVec3::X, turn * DVec3::Y, turn * DVec3::Z);
                    // Fanned, it turns from straight back toward the tip;
                    // closed, it lies back along the flank whatever its
                    // bone does, as it trails coming out.
                    let spread =
                        turn_of(row, t) * (1.0 - 0.45 * flex) + row.fingers * k as f64 * glide;
                    let fanned = back * libm::cos(spread) + way * libm::sin(spread);
                    let toward = tail.lerp(fanned, fan).normalize_or(fanned);
                    let (wide, thick) = (row.wide_m * size, row.thick_m * size);
                    let from = base + plane * (row.over_m * size + SHINGLE * thick * k as f64);
                    let long = (row.long_m[0] + (row.long_m[1] - row.long_m[0]) * t) * size;
                    let color = paint[row.paint.min(2)];
                    put(from, toward, plane, [long, wide, thick], color);
                    let [cap_long, cap_wide] = CAP;
                    put(
                        from + toward * long,
                        toward,
                        plane,
                        [wide * cap_long, wide * cap_wide, thick],
                        color,
                    );
                }
            }
        }
    }

    /// The right wing's bones now, in the rig, as big as it has grown:
    /// spread as the beat has it; folded, as at rest; between, each joint
    /// turned from its folded bend to its spread one, the shoulder lifting
    /// the folded wing first and the wrist last.
    fn pose(&self, build: f64) -> Pose {
        let wing = self.wing;
        let Out {
            size, lift, joints, ..
        } = Out::of(self.grown);
        let (spread, flex) = self.spread();
        let (folded, ..) = folded(build);
        let bends = |turns: [DQuat; 3]| {
            [
                turns[0],
                turns[0].inverse() * turns[1],
                turns[1].inverse() * turns[2],
            ]
        };
        let (from, to) = (bends(folded), bends(spread));
        let raised = from[0].slerp(DQuat::from_rotation_z(LIFT) * from[0], lift);
        let mut turns = [raised.slerp(to[0], joints[0]); 3];
        for bone in 1..3 {
            turns[bone] = turns[bone - 1] * from[bone].slerp(to[bone], joints[bone]);
        }
        let mut at = [DVec3::new(wing.shoulder.x * build, wing.shoulder.y, wing.shoulder.z); 4];
        for bone in 0..3 {
            at[bone + 1] = at[bone] + turns[bone] * DVec3::X * wing.bones_m[bone] * size;
        }
        Pose { turns, at, flex }
    }

    /// Each bone's turn spread, as the beat has it, and how far the hand
    /// folds back rising, 0 to 1.
    fn spread(&self) -> ([DQuat; 3], f64) {
        let flap = libm::sin(TAU * self.beat);
        let (held, hard) = self.wing.stroke;
        let stroke = held + (hard - held) * self.effort;
        // Rising, the hand folds back at the wrist and its feathers close;
        // gliding, they part at the tips.
        let flex = self.effort * libm::cos(TAU * self.beat).max(0.0);
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
        let turns = core::array::from_fn(|bone| {
            DQuat::from_rotation_y(-sweeps[bone]) * DQuat::from_rotation_z(rises[bone])
        });
        (turns, flex)
    }
}

/// Each bone's turn folded at `build`, the right wing's in the rig, and the
/// way its feathers lie.
fn folded(build: f64) -> ([DQuat; 3], DVec3) {
    let (sin, cos) = (libm::sin(FOLD_LEAN), libm::cos(FOLD_LEAN));
    let (down, out) = (DVec3::new(sin, -cos, 0.0), DVec3::new(cos, sin, 0.0));
    let turn = |[back, along, off]: [f64; 3]| {
        let way = (DVec3::Z * back + down * along + out * off * build).normalize();
        let up = (out - way * way.dot(out)).normalize();
        DQuat::from_mat3(&DMat3::from_cols(way, up, way.cross(up)))
    };
    (FOLDED.map(turn), (DVec3::Z + down * TAIL_DOWN).normalize())
}

/// How far the `t`th of a row's feathers turns from straight back toward
/// the tip, fanned, radians.
fn turn_of(row: &Row, t: f64) -> f64 {
    row.fan[0] + (row.fan[1] - row.fan[0]) * t
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

    /// Out, they unfold in the time the wing says, raised, so the first
    /// downstroke strikes within a beat of their being out; a beat hard
    /// enough strikes once a beat; held out to glide, none strikes. They
    /// fold away in their own time. Folded away, none is drawn.
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
            if frame == (WING.open_s / dt).ceil() as usize {
                assert_eq!(wings.grown, 1.0);
            }
        }
        let first = first.expect("a first downstroke");
        // The beat quickens as they open, so the first downstroke falls
        // within a hard beat of their being out.
        assert!(
            first < WING.open_s + 1.0 / WING.beat_hz.1 * 0.75,
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

    /// Every build a herd of horses has, the narrowest and the broadest of
    /// each.
    fn builds() -> impl Iterator<Item = f64> {
        HORSE
            .builds
            .iter()
            .flat_map(|build| [build.build.0, build.build.1])
    }

    /// The right wing's boxes at `build`, `grown` out, raised where it
    /// opens in its beat: its edge, then its feathers.
    fn right(grown: f64, build: f64) -> Vec<BoxPart> {
        let wings = Wings {
            grown,
            beat: OPEN_BEAT,
            ..Wings::new(&WING)
        };
        let mut boxes = Vec::new();
        wings.boxes(DAffine3::IDENTITY, build, [Vec3::ONE; 3], &mut boxes);
        boxes.split_off(boxes.len() / 2)
    }

    /// A box's eight corners.
    fn corners(part: &BoxPart) -> impl Iterator<Item = DVec3> + '_ {
        (0..8).map(|corner| {
            let at = |bit: u32| f64::from((corner >> bit) & 1) - 0.5;
            part.transform
                .transform_point3(DVec3::new(at(0), at(1), at(2)))
        })
    }

    /// How deep a box goes into an upright one from `lo` to `hi`: the least
    /// overlap along the axes that could part them, below zero when apart.
    fn depth(part: &BoxPart, lo: DVec3, hi: DVec3) -> f64 {
        let edges = part.transform.matrix3;
        let edges = [edges.x_axis, edges.y_axis, edges.z_axis];
        let half = (hi - lo) / 2.0;
        let apart = (lo + hi) / 2.0 - part.transform.translation;
        let upright = [DVec3::X, DVec3::Y, DVec3::Z];
        let crossed = edges
            .iter()
            .flat_map(|edge| upright.map(|axis| edge.cross(axis)));
        upright
            .into_iter()
            .chain(edges)
            .chain(crossed)
            .filter(|axis| axis.length() > 1e-9)
            .map(|axis| {
                let axis = axis.normalize();
                let its: f64 = edges.iter().map(|edge| (edge.dot(axis) / 2.0).abs()).sum();
                its + half.dot(axis.abs()) - apart.dot(axis).abs()
            })
            .fold(f64::MAX, f64::min)
    }

    /// The deepest each of the right wing's boxes goes into the horse's
    /// trunk at `build`, `grown` out.
    fn into_trunk(grown: f64, build: f64) -> Vec<f64> {
        right(grown, build)
            .iter()
            .map(|part| {
                HORSE.rig.trunk.iter().fold(f64::MIN, |deepest, section| {
                    let x = section.width / 2.0 * build;
                    let lo = DVec3::new(-x, section.bottom, section.from_z);
                    let hi = DVec3::new(x, section.top, section.to_z);
                    deepest.max(depth(part, lo, hi))
                })
            })
            .collect()
    }

    /// Folded, and just out before it lifts, every feather lies close
    /// along the flank, no further out of the trunk's widest than
    /// `CLOSE_M`, and behind the shoulder, but for the half of a feather's
    /// width that a covert rooted at the shoulder reaches ahead of it.
    #[test]
    fn folded_it_lies_along_the_flank() {
        const CLOSE_M: f64 = 0.3;
        let ahead = WING.rows.iter().map(|row| row.wide_m).fold(0.0, f64::max) / 2.0;
        for build in builds() {
            let widest = HORSE
                .rig
                .trunk
                .iter()
                .map(|section| section.width)
                .fold(0.0, f64::max)
                / 2.0
                * build;
            for grown in [0.01, 0.05, LIFTS[0]] {
                for part in &right(grown, build)[3..] {
                    for corner in corners(part) {
                        assert!(
                            corner.x < widest + CLOSE_M,
                            "build {build}, {grown} out: {corner:.2} is {:.2} m out of the flank",
                            corner.x - widest
                        );
                        assert!(
                            corner.z > WING.shoulder.z - ahead,
                            "build {build}, {grown} out: {corner:.2} is ahead of the shoulder"
                        );
                    }
                }
            }
        }
    }

    /// Open, the wing is the table's spread as the beat has it: each bone
    /// its length from the joint before and turned as spread, nothing of
    /// the fold left in it.
    #[test]
    fn open_it_is_the_tables_spread() {
        for beat in [0.0, OPEN_BEAT, 0.5, 0.75] {
            let wings = Wings {
                grown: 1.0,
                beat,
                effort: 0.7,
                ..Wings::new(&WING)
            };
            let Pose { turns, at, .. } = wings.pose(1.0);
            let (spread, _) = wings.spread();
            for bone in 0..3 {
                let long = at[bone + 1].distance(at[bone]);
                assert!(
                    (long - WING.bones_m[bone]).abs() < 1e-9,
                    "bone {bone}: {long}"
                );
                for axis in [DVec3::X, DVec3::Y] {
                    let (now, spread) = (turns[bone] * axis, spread[bone] * axis);
                    assert!(
                        now.distance(spread) < 1e-9,
                        "bone {bone} at {beat}: {now} against {spread}"
                    );
                }
            }
        }
    }

    /// Coming out, the wing's tip only ever moves out from the body,
    /// frame by frame; folding away it plays back, so it only moves in.
    #[test]
    fn coming_out_the_tip_moves_out() {
        let wings = |grown| Wings {
            grown,
            beat: OPEN_BEAT,
            ..Wings::new(&WING)
        };
        let frames = (WING.open_s * 60.0).round() as usize;
        let mut last = f64::MIN;
        for frame in 1..=frames {
            let grown = frame as f64 / frames as f64;
            let tip = wings(grown).pose(1.0).at[3].x;
            assert!(
                tip >= last,
                "{grown:.3} out: the tip at {tip:.3} from {last:.3}"
            );
            last = tip;
        }
    }

    /// Coming out, no box of a wing passes through the horse's trunk, at
    /// every build a herd has, deeper than the same box does open: the root
    /// is where the wing meets the body, and lies into it. A feather may
    /// touch the trunk by its own thickness more.
    #[test]
    fn coming_out_it_passes_through_no_body() {
        let touch = WING.rows.iter().map(|row| row.thick_m).fold(0.0, f64::max);
        for build in builds() {
            let open = into_trunk(1.0, build);
            for step in 1..100 {
                let grown = f64::from(step) / 100.0;
                for (part, (now, open)) in into_trunk(grown, build).iter().zip(&open).enumerate() {
                    assert!(
                        *now <= open.max(0.0) + touch,
                        "build {build}, {grown} out: box {part} {now:.3} m in, open {open:.3}"
                    );
                }
            }
        }
    }
}
