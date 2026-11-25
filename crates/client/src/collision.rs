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
use topology::{BLOCK_M, SurfacePoint};
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
