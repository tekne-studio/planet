//! What holds a body up: the ground the terrain is meshed from, and the cells
//! of any volume, the solid parts and the trunks under or over it.
//!
//! Nature is a surface: one ground per direction, no cave, no overhang,
//! nothing to be inside of. So a footing over nature is the ground height,
//! read from the generator rather than from a mesh of it, because a mesh is
//! filtered by how far the camera is and what a body stands on may not be. A
//! volume has an inside, and its footing (`build`) carries a roof as well,
//! which is what `admits` asks about.

use glam::DVec3;
use topology::{BLOCK_M, Grid, SurfacePoint};
use worldgen::{Direction, Generator};

/// A rise a walker takes in its stride. One block; two is a wall, to be
/// jumped or flown.
pub const STEP_M: f64 = BLOCK_M;
/// A standing body, metres. What a roof has to clear.
pub const BODY_M: f64 = 1.8;
/// Metres between probes along a ray looking for ground.
const PROBE_M: f64 = BLOCK_M / 2.0;
/// Times the stretch of a ray between the last probe over the ground and
/// the first under it is halved to find where it goes in: to a sixteenth
/// of a probe.
const HALVINGS: usize = 4;
/// Half the width of a standing body in address space, in blocks: how near
/// a wall it comes, and how far past the edge of a floor it still stands.
pub const BODY_HALF: f64 = 0.6;
/// The most points on a body's spine, ahead and behind together.
const SPINE_POINTS: usize = 8;
/// How far apart the points of a body's spine are, at most, metres: under
/// the width a point stands on plus a column, so no column between two is
/// missed.
const SPINE_APART_M: f64 = 1.0;

/// What a body at one direction stands on, and what is over its head.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Footing {
    /// Top of the solid at or under the feet, metres over the datum sphere.
    /// `None` where there is nothing to stand on, so the body falls.
    pub floor_m: Option<f64>,
    /// Bottom of the solid over the feet, where there is one in reach. Always
    /// `None` over open nature, which has no roof.
    pub ceiling_m: Option<f64>,
}

impl Footing {
    /// Open air: nothing under, nothing over.
    pub const OPEN: Footing = Footing {
        floor_m: None,
        ceiling_m: None,
    };

    /// A ground that is only ever a height, which is all of nature.
    pub fn solid(ground_m: f64) -> Footing {
        Footing {
            floor_m: Some(ground_m),
            ceiling_m: None,
        }
    }

    /// Both at once: the higher floor and the lower roof. What a body over
    /// the ground and a volume together stands on.
    pub fn with(self, other: Footing) -> Footing {
        let pick = |a: Option<f64>, b: Option<f64>, f: fn(f64, f64) -> f64| match (a, b) {
            (Some(a), Some(b)) => Some(f(a, b)),
            (a, b) => a.or(b),
        };
        Footing {
            floor_m: pick(self.floor_m, other.floor_m, f64::max),
            ceiling_m: pick(self.ceiling_m, other.ceiling_m, f64::min),
        }
    }

    /// Metres of open air over the floor, unbounded under the sky.
    pub fn room_m(self) -> f64 {
        match (self.floor_m, self.ceiling_m) {
            (Some(floor_m), Some(ceiling_m)) => ceiling_m - floor_m,
            _ => f64::INFINITY,
        }
    }
}

/// What holds a body up at a column, the ground and what is built apart.
/// Together they are its [`Footing`]; apart, they are what [`aloft`] asks
/// about.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Held {
    pub ground: Footing,
    /// `None` with nothing built near.
    pub built: Option<Footing>,
}

impl Held {
    /// Nothing under, nothing over.
    pub const OPEN: Held = Held {
        ground: Footing::OPEN,
        built: None,
    };

    /// Both as one: what a body stands on and has over its head.
    pub fn footing(self) -> Footing {
        self.built
            .map_or(self.ground, |built| self.ground.with(built))
    }

    /// The highest floor built here, if any.
    fn built_m(self) -> Option<f64> {
        self.built.and_then(|built| built.floor_m)
    }
}

/// Whether a body `body_m` tall in the air on a jump may go from one column
/// to another: onto ground no higher than a step over `feet_m`, as a
/// walker, with room over it, and over what is built only with its feet at
/// `arc_m` over it, or no higher than what was built under it where it
/// was, a floor it goes on over. So the ground holds a jump up as it holds
/// a walker, and what is built is cleared by the arc or met.
pub fn aloft(from: Held, to: Held, feet_m: f64, arc_m: f64, body_m: f64) -> bool {
    let ground = to
        .ground
        .floor_m
        .is_none_or(|floor_m| floor_m <= feet_m + STEP_M);
    let built = to
        .built_m()
        .is_none_or(|floor_m| floor_m <= from.built_m().map_or(arc_m, |from_m| from_m.max(arc_m)));
    let (was, will) = (from.footing(), to.footing());
    let room = will.floor_m.is_none() || will.room_m() >= body_m.min(was.room_m());
    ground && built && room
}

/// The columns a standing body's width spans at a point, in blocks of its
/// sector, `u` fastest: it stands on and is stopped by what is in any of
/// them. One past an edge of the sector is the neighbour's.
pub fn columns(point: SurfacePoint) -> impl Iterator<Item = [i32; 2]> {
    let span = |at: f64| (at - BODY_HALF).floor() as i32..=(at + BODY_HALF).floor() as i32;
    span(point.v).flat_map(move |y| span(point.u).map(move |x| [x, y]))
}

/// The footing of one direction.
///
/// Collision is the ground in full detail, never the filtered one: what a body
/// stands on may not change with how far away the camera is. So the footprint
/// asked for is zero.
pub fn footing(generator: &Generator, direction: Direction, _feet_m: f64) -> Footing {
    Footing::solid(generator.sample_at(direction, 0.0).height_m)
}

/// Whether a body `body_m` tall on feet may go from one footing to another:
/// a floor within one step up, and room over it to stand.
///
/// A body already under a low roof keeps whatever room it had, so a place too
/// tight to stand in is never a place to be stuck in.
pub fn admits(from: Footing, to: Footing, feet_m: f64, body_m: f64) -> bool {
    match to.floor_m {
        // Nothing within reach to stand on: a ledge, and walking off one is
        // allowed. Falling is the ground's answer, not a wall's.
        None => true,
        Some(floor_m) => floor_m <= feet_m + STEP_M && to.room_m() >= body_m.min(from.room_m()),
    }
}

/// Whether a point of a body on its feet, `height_m` over the datum and
/// `body_m` tall, goes from `was` to `will`, the body's middle on `seat`:
/// as [`admits`] lets a walker, its feet where [`feet_m`] has them.
pub fn afoot(seat: Held, was: Held, will: Held, height_m: f64, body_m: f64) -> bool {
    let (seat, was) = (seat.footing(), was.footing());
    admits(was, will.footing(), feet_m(height_m, seat, was), body_m)
}

/// Where the feet are at a point of a body's spine standing on `point`,
/// metres, the body `height_m` over the datum on `seat`: as high as the
/// body's, or more by as far as the point's floor is over the body's. It
/// follows a rise and keeps the body's height down a fall.
pub fn feet_m(height_m: f64, seat: Footing, point: Footing) -> f64 {
    match (seat.floor_m, point.floor_m) {
        (Some(seat_m), Some(point_m)) => height_m + (point_m - seat_m).max(0.0),
        _ => height_m,
    }
}

/// Whether a body on its feet that stood on a floor stays on one under
/// its feet `height_m`: a step down it walks onto, as [`admits`] takes one
/// up, so a stair is walked down as it is walked up; from deeper it falls.
pub fn steps_down(height_m: f64, floor_m: f64) -> bool {
    height_m - floor_m <= STEP_M
}

/// A displacement on the planet, world space, as blocks along each
/// address axis of a column of `grid`, `radius_m` from its centre.
pub fn address_step(grid: Grid, point: SurfacePoint, radius_m: f64, delta: DVec3) -> (f64, f64) {
    let tangents = grid.tangents(point);
    // Metres per block along each address axis, at this height.
    let (tu, tv) = (
        DVec3::from(tangents.du) * radius_m,
        DVec3::from(tangents.dv) * radius_m,
    );
    // Solve `du * tu + dv * tv = delta` on the tangent plane. The axes are
    // not orthogonal, so this is a 2x2 system, not two dot products.
    let (a, b, c) = (tu.dot(tu), tu.dot(tv), tv.dot(tv));
    let (p, q) = (tu.dot(delta), tv.dot(delta));
    let det = a * c - b * b;
    ((c * p - b * q) / det, (a * q - b * p) / det)
}

/// Whether a body turns where it stands from its spine `was` to its spine
/// `will`, its middle on `seat`: each point admitted from where it was to
/// where it swings, as a step admits it. `footing` reads what holds a
/// body at an offset from the middle, blocks along `u` and `v`.
pub fn turns(
    seat: Held,
    was: &Spine,
    will: &Spine,
    footing: impl Fn([f64; 2]) -> Held,
    admits: impl Fn(Held, Held, Held, f64) -> bool,
) -> bool {
    (was.points().zip(will.points()))
        .all(|((was, along_m), (will, _))| admits(seat, footing(was), footing(will), along_m))
}

/// How many pieces a step of `du, dv` blocks goes in a cell at a time
/// along either axis, so no column it crosses is jumped over and no wall
/// is thinner than a step: one at least.
pub fn cell_by_cell(du: f64, dv: f64) -> u32 {
    du.abs().max(dv.abs()).ceil().max(1.0) as u32
}

/// Where a step of `du, dv` from a column of a grid ends, in address
/// space, gone in `pieces`: where it asked, or short of whatever stopped
/// it. `footing` reads what holds a body at a column, and `admits` says
/// whether a point of the body `along_m` ahead of its middle goes from
/// one to another, the middle on the first.
///
/// A rise of one block is taken in stride and more is a wall, so a cave
/// wall stops a body exactly the way a cliff does and the only way past
/// either is to jump or to fly. Blocked, the step is tried one address
/// axis at a time, which is what slides a body along a wall instead of
/// sticking it to one.
///
/// A walker's step goes [`cell_by_cell`], so no column it crosses is
/// jumped over. A flyer's goes in one: at its speeds, a cell at a time is
/// a look at every cell it crosses, and a wall thinner than its step is
/// passed.
///
/// Each point of the body's `spine` goes the same piece and is admitted
/// as the body is, its feet as high as the body's or on a floor of its
/// own that is higher: so a long body stops with its nose at a wall,
/// backs with its rump to one, slides along one whole, and flies one
/// only when all of it clears.
pub fn walk(
    grid: Grid,
    from: SurfacePoint,
    [du, dv]: [f64; 2],
    pieces: u32,
    spine: &Spine,
    footing: impl Fn(SurfacePoint) -> Held,
    admits: impl Fn(Held, Held, Held, f64) -> bool,
) -> SurfacePoint {
    // Where a point of the body is with the pieces taken along each
    // axis: a share of the step past its offset, in the frame of the
    // column it started from, so the whole step ends where it asked.
    let at = |[along_u, along_v]: [u32; 2], [off_u, off_v]: [f64; 2]| {
        let share = |taken: u32| f64::from(taken) / f64::from(pieces);
        let (u, v) = (from.u + du * share(along_u), from.v + dv * share(along_v));
        grid.wrapped(SurfacePoint::new(from.sector, u + off_u, v + off_v))
    };
    let (mut taken, mut here) = ([0, 0], footing(from));
    let mut reached = [Held::OPEN; SPINE_POINTS];
    for (point, (offset, _)) in reached.iter_mut().zip(spine.points()) {
        *point = footing(at(taken, offset));
    }
    for _ in 0..pieces {
        let piece = [[1, 1], [1, 0], [0, 1]].into_iter().find_map(|[u, v]| {
            if du * f64::from(u) == 0.0 && dv * f64::from(v) == 0.0 {
                return None;
            }
            let tried = [taken[0] + u, taken[1] + v];
            let there = footing(at(tried, [0.0, 0.0]));
            if !admits(here, here, there, 0.0) {
                return None;
            }
            let mut reaches = reached;
            for (point, (offset, along_m)) in reaches.iter_mut().zip(spine.points()) {
                let will = footing(at(tried, offset));
                if !admits(here, *point, will, along_m) {
                    return None;
                }
                *point = will;
            }
            Some((tried, there, reaches))
        });
        match piece {
            Some((tried, there, reaches)) => (taken, here, reached) = (tried, there, reaches),
            None => break,
        }
    }
    if taken == [0, 0] {
        from
    } else {
        at(taken, [0.0, 0.0])
    }
}

/// A body's length along the way it faces: points ahead of where it
/// stands to its nose and behind it to its rump, each kept off what stops
/// the body. A body on foot is where it stands, and has none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spine {
    /// Each point's offset from where the body stands, blocks along `u`
    /// and `v`, and how far ahead of it, metres, behind it less than none.
    points: [[f64; 2]; SPINE_POINTS],
    along_m: [f64; SPINE_POINTS],
    len: usize,
}

impl Spine {
    pub const NONE: Spine = Spine {
        points: [[0.0; 2]; SPINE_POINTS],
        along_m: [0.0; SPINE_POINTS],
        len: 0,
    };

    /// Points `ahead_m` along `per_m`, blocks a metre, and `behind_m`
    /// against it, each end one and the rest even between it and the
    /// middle, at most [`SPINE_APART_M`] apart while there are points
    /// enough.
    pub fn along(per_m: [f64; 2], ahead_m: f64, behind_m: f64) -> Spine {
        let mut spine = Spine::NONE;
        for (reach_m, sign) in [(ahead_m, 1.0), (behind_m, -1.0)] {
            let count = ((reach_m / SPINE_APART_M).ceil() as usize).min(SPINE_POINTS / 2);
            for point in 1..=count {
                let m = sign * reach_m * point as f64 / count as f64;
                spine.points[spine.len] = per_m.map(|blocks| blocks * m);
                spine.along_m[spine.len] = m;
                spine.len += 1;
            }
        }
        spine
    }

    /// Each point's offset, blocks along `u` and `v`, and how far ahead of
    /// the middle it is, metres.
    pub fn points(&self) -> impl Iterator<Item = ([f64; 2], f64)> + '_ {
        self.points[..self.len]
            .iter()
            .copied()
            .zip(self.along_m[..self.len].iter().copied())
    }
}

/// How far a ray from `from` along `toward` (unit, world space) runs before it
/// goes under the ground, up to `reach_m`. What keeps a third person camera
/// out of the hill its body is standing against.
pub fn clear_run_m(generator: &Generator, from: DVec3, toward: DVec3, reach_m: f64) -> f64 {
    let radius_m = generator.sphere().radius_m();
    let under = |at: DVec3| {
        let direction = at.normalize_or(DVec3::Y).to_array();
        at.length() - radius_m < generator.sample_at(direction, 0.0).height_m
    };
    ground_run_m(from, toward, reach_m, under).map_or(reach_m, |hit_m| (hit_m - PROBE_M).max(0.0))
}

/// How far a ray from `from` along `toward`, unit, runs before it goes into
/// whatever `under` says is below the ground, up to `reach_m`: `None` when
/// it stays over it that far. Probed every [`PROBE_M`], then halved in to
/// where it goes under.
pub fn ground_run_m(
    from: DVec3,
    toward: DVec3,
    reach_m: f64,
    under: impl Fn(DVec3) -> bool,
) -> Option<f64> {
    let steps = (reach_m / PROBE_M).floor().max(0.0) as usize;
    let step = (1..=steps).find(|&i| under(from + toward * (i as f64 * PROBE_M)))?;
    let (mut over_m, mut under_m) = ((step - 1) as f64 * PROBE_M, step as f64 * PROBE_M);
    for _ in 0..HALVINGS {
        let middle_m = (over_m + under_m) / 2.0;
        match under(from + toward * middle_m) {
            true => under_m = middle_m,
            false => over_m = middle_m,
        }
    }
    Some(under_m)
}
