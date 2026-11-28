//! How a body leaves the wood of a trunk. A tree is no perch: its trunk
//! stops a body from outside as a wall does, and its top is a floor only
//! to slide off. A body found in a trunk's wood, standing on its top or
//! coming down on it goes out by the side to where it turns every way
//! clear of the wood, and comes down on the ground beside it.
//!
//! A body meets the wood where the trunk's top is the floor it stands on
//! or comes down to: no floor at or over that top, the ground or what is
//! built, between it and its feet ([`met`]). It looks for its way out as
//! it meets the wood, the nearest way its own walk goes all along, and
//! goes along it frame by frame ([`step`]): at a walk's stride in the wood
//! or on the top, and coming down, eased aside as it comes down, clear as
//! its feet reach the highest top of the trees it leaves. So where it ends
//! is the same at any frame rate. On its way out, every trunk but the ones
//! it leaves stops it as a wall, and a tree it meets on the way joins
//! them; out, the ones it left are walls to it too, with no floor, until
//! what it is down on holds it. Stopped on the way, it looks again from
//! where it stopped; with no way out, the wood holds it as anything built
//! would.

use glam::DVec3;
use topology::{BLOCK_M, Column, Grid, SurfacePoint};
use worldgen::Generator;

use crate::cells::Seat;
use crate::collision::{self, BODY_HALF, STEP_M, Spine};
use crate::controller::Built;

/// How fast a body in the wood of a trunk, or on its top, goes out of it
/// by the side, metres a second: a walk's stride.
const MPS: f64 = 2.0;
/// How steeply a body coming down on a trunk is eased off it: metres down
/// for each metre aside, so it is clear of the wood as its feet come to
/// the top.
const STEEP: f64 = 3.0;
/// Within this over a trunk's top, metres, feet are on it.
const ON_M: f64 = 1e-3;
/// How many ways out of wood are looked along, evenly around.
const WAYS: u32 = 16;
/// How far a way out is looked along, blocks, a step of how many at a
/// time, and how many times the step it leaves the wood in is halved.
const OUT_MOST: i32 = 12;
const OUT_STEP: f64 = 0.5;
const OUT_HALVINGS: usize = 6;
/// How much shorter a way out reads the more it goes the way the body
/// faces: a body takes the side it heads to when two are near alike.
const HEADING: f64 = 0.25;
/// How high over a trunk's top a body coming down meets it at most,
/// metres: the steepness times the longest way out.
const REACH_M: f64 = STEEP * OUT_MOST as f64 * BLOCK_M;
/// How far a body's spine reaches from its middle, blocks, that the wood
/// about it is read wide enough for at the longest way out: every kind's
/// longest, where blocks are smallest. A body reaching farther looks for
/// its way out less far.
const REACH_MOST: i32 = 6;
/// How many columns every way from the one a body stands in its way out
/// meets: the longest way, the longest reach, and its width past them.
const SPAN: i32 = OUT_MOST + REACH_MOST + 2;
const SIDE: usize = (2 * SPAN + 1) as usize;
/// How many trees a body leaves at once, each by its root: more, and it
/// leaves every one.
const LEFT_MOST: usize = 4;

/// What of a body a way out of wood clears: its width at its middle and
/// at each point of its spine, and the way it faces, blocks along `u` and
/// `v` a metre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub spine: Spine,
    pub toward: [f64; 2],
}

impl Footprint {
    /// Its width at its middle alone, facing nowhere.
    #[cfg(test)]
    pub const NONE: Footprint = Footprint {
        spine: Spine::NONE,
        toward: [0.0; 2],
    };

    /// Each offset from the middle a width of the body stands at, blocks
    /// along `u` and `v`, the middle's first.
    pub fn offsets(&self) -> impl Iterator<Item = [f64; 2]> + '_ {
        core::iter::once([0.0; 2]).chain(self.spine.points().map(|(offset, _)| offset))
    }

    /// How far its spine reaches from its middle, blocks, the longer way:
    /// none for a body on foot.
    pub fn reach(&self) -> f64 {
        (self.spine.points())
            .map(|([du, dv], _)| du.hypot(dv))
            .fold(0.0, f64::max)
    }
}

/// The trees a body leaves, each by the column its root is in: on its way
/// out, their wood is no wall to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Left {
    roots: [Option<Column>; LEFT_MOST],
    /// More than [`LEFT_MOST`] at once: it leaves every tree.
    all: bool,
}

impl Left {
    /// No tree: every trunk is a wall.
    pub const NONE: Left = Left {
        roots: [None; LEFT_MOST],
        all: false,
    };

    fn add(&mut self, root: Column) {
        if self.has(root) {
            return;
        }
        match self.roots.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => *slot = Some(root),
            None => self.all = true,
        }
    }

    /// The trees it leaves and the ones `other` leaves.
    fn and(mut self, other: Left) -> Left {
        self.all |= other.all;
        other
            .roots
            .into_iter()
            .flatten()
            .for_each(|root| self.add(root));
        self
    }

    /// Whether it leaves the tree rooted in `root`'s column.
    pub fn has(&self, root: Column) -> bool {
        self.all || self.roots.contains(&Some(root))
    }

    /// Whether it leaves every tree `other` does.
    fn holds(&self, other: &Left) -> bool {
        self.all || (!other.all && other.roots.iter().flatten().all(|&root| self.has(root)))
    }
}

/// What the trees a body leaves are to it; every other trunk is to it
/// what it is to any body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leaves {
    /// Nothing: its way out goes through their wood.
    Through,
    /// Walls from the ground up, with no floor: out of their wood and not
    /// down yet, it never comes back onto them.
    Walls,
    /// What is built is: their tops a floor at any height, where it has
    /// no way out.
    Built,
}

/// A body that met the wood of a trunk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaving {
    way: Way,
    /// Where it was left last: by its way, or where it stood. Found
    /// farther from where it goes, it went its own way.
    at: SurfacePoint,
    /// The highest top of the trees it leaves, metres over the datum.
    top_m: f64,
    left: Left,
}

/// Where a body that met the wood is on its way out of it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Way {
    /// Going to where it turns every way clear of the wood, else where its
    /// footprint facing as it does is.
    To(SurfacePoint),
    /// Out, and coming down beside the wood: done once what it is down on
    /// holds it.
    Down,
    /// With no way out: held as anything built holds a body, until it is
    /// off the wood.
    Held,
}

impl Leaving {
    /// Whether it is on its way out of the wood: going out, or out and
    /// coming down beside it.
    #[cfg(test)]
    pub fn going(&self) -> bool {
        self.way != Way::Held
    }

    /// The trees it leaves and what they are to it, out and coming down
    /// or held: `None` while it goes out, when they are to it what they
    /// are to any body.
    pub fn leaves(&self) -> Option<(Left, Leaves)> {
        match self.way {
            Way::To(_) => None,
            Way::Down => Some((self.left, Leaves::Walls)),
            Way::Held => Some((self.left, Leaves::Built)),
        }
    }

    /// Looks for the way out of the wood a body met: the highest top of
    /// it and the trees it is of, as [`met`] finds them, raised to the
    /// highest top of those trees about it. Of [`WAYS`] ways evenly
    /// around, the nearest to where it turns every way clear of all wood,
    /// the one it faces when two are near alike; else the nearest to where
    /// its footprint facing as it does is clear; each along a way where no
    /// trunk but the ones it leaves is under its footprint. Else the
    /// nearest to where its footprint is clear, past other trunks. Of
    /// each, the first `walk`, every trunk but the ones it leaves a wall,
    /// goes all along; with none, it is held.
    fn start(
        point: SurfacePoint,
        feet_m: f64,
        footprint: &Footprint,
        (top_m, left): (f64, Left),
        generator: &Generator,
        built: Built<'_>,
        walk: impl Fn(SurfacePoint, [f64; 2], Left) -> SurfacePoint,
    ) -> Leaving {
        let stand = Stand::read(point, feet_m, &left, generator, built);
        let reach = footprint.reach();
        let most = (f64::from(OUT_MOST)).min(f64::from(SPAN - 2) - reach);
        let under = |[du, dv]: [f64; 2]| {
            (footprint.offsets())
                .map(|[u, v]| stand.swept([du + u, dv + v], 0.0))
                .max()
                .unwrap_or(Wood::Clear)
        };
        let turns = |at: [f64; 2]| match (under(at), stand.swept(at, reach)) {
            (Wood::Other, _) => Probe::Stopped,
            (_, Wood::Clear) => Probe::Out,
            _ => Probe::In,
        };
        let stands = |at: [f64; 2]| match under(at) {
            Wood::Other => Probe::Stopped,
            Wood::Left => Probe::In,
            Wood::Clear => Probe::Out,
        };
        let passes = |at: [f64; 2]| match under(at) {
            Wood::Clear => Probe::Out,
            Wood::Left | Wood::Other => Probe::In,
        };
        let grid = generator.sphere().blocks();
        let goes = |[du, dv]: [f64; 2]| {
            let to = grid.wrapped(SurfacePoint::new(point.sector, point.u + du, point.v + dv));
            (walk(point, [du, dv], left) == to).then_some(to)
        };
        let probes: [&dyn Fn([f64; 2]) -> Probe; 3] = [&turns, &stands, &passes];
        let to = (probes.into_iter())
            .find_map(|probe| ways_out(footprint.toward, most, probe).find_map(goes));
        Leaving {
            way: to.map_or(Way::Held, Way::To),
            at: point,
            top_m: top_m.max(stand.top_m),
            left,
        }
    }
}

/// The wood a body with its feet `feet_m` over the datum at a point of a
/// seat meets under its `footprint`: the highest top among it, and the
/// trees it is of. A trunk under the width at a point of the footprint is
/// met where the body is in its wood, or where its top is the floor the
/// body stands on or comes down to: where no floor as high as the top or
/// higher and within a step over the feet, the ground or what is built,
/// is there or under the body's middle; a floor flush with the top holds
/// the body. Never where the trunk is a roof over the feet, nor where its
/// top is more than [`REACH_M`] under them. `None` with none met, and off
/// the planet.
pub(crate) fn met(
    seat: Seat,
    point: SurfacePoint,
    feet_m: f64,
    footprint: &Footprint,
    generator: &Generator,
    built: Built<'_>,
) -> Option<(f64, Left)> {
    let Seat::Sector(_) = seat else {
        return None;
    };
    let grid = generator.sphere().blocks();
    // The highest floor at a point within a step over the feet, one they
    // stand on, or are set back on after a step dipped them under it, as
    // high as a top met can be or higher: read only where a top under the
    // feet asks it, the middle's first.
    let floor_m = |at: SurfacePoint| {
        let on = Seat::Sector(at.sector);
        let ground_m = seat::ground_m(generator, on.body(), grid.direction(at));
        let ground_m = Some(ground_m).filter(|&ground_m| ground_m <= feet_m + STEP_M);
        (built
            .floor_over(generator, on, at, feet_m, feet_m - REACH_M)
            .into_iter())
        .chain(ground_m)
        .fold(f64::NEG_INFINITY, f64::max)
    };
    let mut middle_m = None;
    let mut met: Option<(f64, Left)> = None;
    for [du, dv] in footprint.offsets() {
        let at = grid.wrapped(SurfacePoint::new(point.sector, point.u + du, point.v + dv));
        let mut here_m = None;
        let wood =
            (built.trunks).rooted(generator, built.cells, Seat::Sector(at.sector), at, feet_m);
        let tops = wood.filter_map(|(footing, root)| footing.floor_m.map(|top_m| (top_m, root)));
        for (top_m, root) in tops {
            if feet_m > top_m + REACH_M {
                continue;
            }
            let higher = top_m <= feet_m + STEP_M && {
                let middle_m = *middle_m.get_or_insert_with(|| floor_m(point));
                let aside = [du, dv] != [0.0; 2];
                middle_m >= top_m || (aside && *here_m.get_or_insert_with(|| floor_m(at)) >= top_m)
            };
            if higher {
                continue;
            }
            let (highest_m, left) = met.get_or_insert((top_m, Left::NONE));
            *highest_m = highest_m.max(top_m);
            left.add(root);
        }
    }
    met
}

/// One step of `dt` of a body at `point` of the planet that may leave the
/// wood, `leaving` it already or not, its feet going from one height to
/// another, `landed` where what it was down on held it last: where its
/// way takes it, and how it leaves the wood still. It meets the wood as
/// [`met`] says, and looks for its way out as it meets it or meets a tree
/// it did not leave, which joins the ones it leaves. It goes the share of
/// the rest of its way [`share`] says while `walk`, which walks a step of
/// blocks along `u` and `v` from a point, every trunk but the ones it
/// leaves a wall, walks the whole rest of it: a walk is held where each of
/// its pieces ends, so a share walked alone could be stopped by a corner
/// the whole way, as it was looked for, goes by. Stopped, it looks again
/// from where it is. It is out at the end of its way, and once a step of
/// its own takes it off the wood and farther from where it goes: what it
/// does from there is its own, the trees it left walls to it until it is
/// down. A step that takes it nearer, as coming down under a frame still
/// turning to the ground does, leaves it on its way. Over the wood and
/// not coming down, it stays on its way while it meets the wood, as a jump
/// goes over a trunk, and is out once it does not.
#[allow(clippy::too_many_arguments)]
pub(crate) fn step(
    leaving: Option<Leaving>,
    point: SurfacePoint,
    [from_m, to_m]: [f64; 2],
    footprint: &Footprint,
    landed: bool,
    dt: f64,
    generator: &Generator,
    built: Built<'_>,
    walk: impl Fn(SurfacePoint, [f64; 2], Left) -> SurfacePoint,
) -> (Option<Leaving>, SurfacePoint) {
    let seat = Seat::Sector(point.sector);
    let met = met(seat, point, to_m, footprint, generator, built);
    let start = |at: SurfacePoint, met: (f64, Left)| {
        Leaving::start(at, to_m, footprint, met, generator, built, &walk)
    };
    // Out of the wood at a point: coming down beside it, else on what
    // holds it over it, and done.
    let out = |leaving: Leaving, at: SurfacePoint| {
        let clear = clear(at, to_m, footprint, &leaving.left, generator, built);
        clear.then_some(Leaving {
            way: Way::Down,
            at,
            ..leaving
        })
    };
    let leaving = match (leaving, met) {
        (None, None) => return (None, point),
        (Some(leaving), None) => leaving,
        (Some(leaving), Some((top_m, left)))
            if leaving.way != Way::Down && leaving.left.holds(&left) =>
        {
            Leaving {
                top_m: leaving.top_m.max(top_m),
                ..leaving
            }
        }
        (None, Some(met)) => start(point, met),
        // A tree it meets on its way joins the ones it leaves, which stay
        // left until it is down: a tree it is still in never turns to a
        // wall around it.
        (Some(leaving), Some((top_m, left))) => {
            start(point, (leaving.top_m.max(top_m), leaving.left.and(left)))
        }
    };
    let to = match leaving.way {
        Way::To(to) => to,
        Way::Down => {
            return (
                (!landed).then_some(Leaving {
                    at: point,
                    ..leaving
                }),
                point,
            );
        }
        Way::Held => return (met.map(|_| leaving), point),
    };
    let grid = generator.sphere().blocks();
    let blocks = |from: SurfacePoint, to: SurfacePoint| {
        let [du, dv] = toward(grid, from, to);
        du.hypot(dv)
    };
    if met.is_none() && blocks(point, to) > blocks(leaving.at, to) {
        return (out(leaving, point), point);
    }
    let way = toward(grid, point, to);
    let share = share(
        way[0].hypot(way[1]) * BLOCK_M,
        leaving.top_m,
        [from_m, to_m],
        dt,
    );
    if share <= 0.0 {
        let stays = Leaving {
            at: point,
            ..leaving
        };
        return (
            met.map_or_else(|| out(leaving, point), |_| Some(stays)),
            point,
        );
    }
    let along = |share: f64| {
        let [du, dv] = way.map(|blocks| blocks * share.min(1.0));
        grid.wrapped(SurfacePoint::new(point.sector, point.u + du, point.v + dv))
    };
    let again = |at: SurfacePoint| start(at, (leaving.top_m, leaving.left));
    if walk(point, way, leaving.left) != along(1.0) {
        return (Some(again(point)), point);
    }
    let went = along(share);
    let leaving = match share >= 1.0 {
        false => Leaving {
            at: went,
            ..leaving
        },
        true => out(leaving, went).unwrap_or_else(|| again(went)),
    };
    (Some(leaving), went)
}

/// Whether no wood of the trees `left` names is under a body's footprint
/// at a point, its feet `feet_m` over the datum, a wall or a floor to it.
fn clear(
    point: SurfacePoint,
    feet_m: f64,
    footprint: &Footprint,
    left: &Left,
    generator: &Generator,
    built: Built<'_>,
) -> bool {
    let grid = generator.sphere().blocks();
    footprint.offsets().all(|[du, dv]| {
        let at = grid.wrapped(SurfacePoint::new(point.sector, point.u + du, point.v + dv));
        let seat = Seat::Sector(at.sector);
        (built
            .trunks
            .rooted(generator, built.cells, seat, at, feet_m))
        .all(|(footing, root)| footing.floor_m.is_none() || !left.has(root))
    })
}

/// Blocks along `from`'s `u` and `v` to `to`: within a sector, how far
/// apart they are; across a seam, the chord between them on the plane
/// that touches the sphere at `from`.
fn toward(grid: Grid, from: SurfacePoint, to: SurfacePoint) -> [f64; 2] {
    if from.sector == to.sector {
        return [to.u - from.u, to.v - from.v];
    }
    let chord = DVec3::from(grid.direction(to)) - DVec3::from(grid.direction(from));
    let (du, dv) = collision::address_step(grid, from, 1.0, chord);
    [du, dv]
}

/// The share of a way out of a trunk's wood `way_m` long a body takes in
/// a step of `dt` that took its feet from one height to another, the
/// highest top of the trees it leaves `top_m`. In the wood or on its top it
/// goes out at [`MPS`]. Over the top and coming down, it is eased off as
/// it comes down, [`STEEP`] times as far down as aside, and is clear as
/// its feet come to the top: never popped and never stepped. Over the top
/// and not coming down, it stays: a jump and a take-off go over a trunk as
/// they go over a wall.
fn share(way_m: f64, top_m: f64, [from_m, to_m]: [f64; 2], dt: f64) -> f64 {
    if way_m <= 0.0 {
        return 1.0;
    }
    if from_m <= top_m + ON_M {
        return (MPS * dt / way_m).min(1.0);
    }
    let from_m = from_m.min(top_m + STEEP * way_m);
    match to_m < from_m {
        true => ((from_m - to_m.max(top_m)) / (from_m - top_m)).min(1.0),
        false => 0.0,
    }
}

/// What a probe of a way out finds a body at: still in the wood, out of
/// it, or stopped on the way by a trunk it does not leave.
#[derive(Clone, Copy, PartialEq)]
enum Probe {
    In,
    Out,
    Stopped,
}

/// The ways out of the wood for a body in it or over it, blocks along
/// `u` and `v`, as `probe` reads a body moved by an offset, nearest
/// first: of [`WAYS`] ways evenly around, out to `most` blocks, the one
/// nearer `toward` first when two are near alike, `toward` any length and
/// none for no heading. A way stopped before it is out is none. Each is
/// looked for only once the one before it is turned down.
fn ways_out<'a>(
    toward: [f64; 2],
    most: f64,
    probe: &'a dyn Fn([f64; 2]) -> Probe,
) -> impl Iterator<Item = [f64; 2]> + 'a {
    let length = toward[0].hypot(toward[1]);
    let toward = match length > 0.0 {
        true => toward.map(|along| along / length),
        false => [0.0; 2],
    };
    // The ways turned down or found to lead nowhere, a bit each.
    let mut done = 0_u32;
    core::iter::from_fn(move || {
        let (way, at) = way_out(toward, most, &mut done, probe)?;
        done |= 1 << way;
        Some(at)
    })
}

/// The nearest of the ways out not `done`, as [`ways_out`] reads them:
/// which of the [`WAYS`] it is, and where it ends. A way that leads
/// nowhere is marked done.
fn way_out(
    toward: [f64; 2],
    most: f64,
    done: &mut u32,
    probe: &dyn Fn([f64; 2]) -> Probe,
) -> Option<(u32, [f64; 2])> {
    // The best way so far: how far it reads, which it is and where it goes.
    let mut best: Option<(f64, u32, [f64; 2])> = None;
    let open = *done;
    'ways: for way in (0..WAYS).filter(|way| open & (1 << way) == 0) {
        let angle = core::f64::consts::TAU * f64::from(way) / f64::from(WAYS);
        let along = [libm::cos(angle), libm::sin(angle)];
        let at = |blocks: f64| along.map(|along| along * blocks);
        let reads = 1.0 - HEADING * (along[0] * toward[0] + along[1] * toward[1]);
        // No way read shorter than the best so far is left past this.
        let worth = |blocks: f64| best.is_none_or(|(best, ..)| blocks * (1.0 - HEADING) < best);
        let (mut deep, mut clear) = (0.0, OUT_STEP);
        loop {
            if clear > most {
                *done |= 1 << way;
                continue 'ways;
            }
            if !worth(clear) {
                continue 'ways;
            }
            match probe(at(clear)) {
                Probe::In => (deep, clear) = (clear, clear + OUT_STEP),
                Probe::Out => break,
                Probe::Stopped => {
                    *done |= 1 << way;
                    continue 'ways;
                }
            }
        }
        // Between two probes half a block apart no trunk is missed: what
        // the footprint covers between them, it covers at one of them.
        for _ in 0..OUT_HALVINGS {
            let middle = (deep + clear) / 2.0;
            match probe(at(middle)) {
                Probe::Out => clear = middle,
                Probe::In | Probe::Stopped => deep = middle,
            }
        }
        if best.is_none_or(|(best, ..)| clear * reads < best) {
            best = Some((clear * reads, way, at(clear)));
        }
    }
    best.map(|(_, way, at)| (way, at))
}

/// What stands in a column about a body leaving the wood, worst last:
/// nothing, the wood it leaves, or another trunk's.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Wood {
    Clear,
    Left,
    Other,
}

/// The wood about a body leaving it, [`SPAN`] columns every way from the
/// one it stands in, read as it looks for its way out.
struct Stand {
    wood: [Wood; SIDE * SIDE],
    /// The lowest `u` and `v` of the first column, blocks from the body.
    corner: [f64; 2],
    /// The highest top of the trees it leaves among it, metres over the
    /// datum: none with none.
    top_m: f64,
}

impl Stand {
    /// The wood about a body at `point` with its feet at `feet_m`, a wall
    /// or a floor to it, never a roof: the trees `left` names its own, the
    /// rest another's.
    fn read(
        point: SurfacePoint,
        feet_m: f64,
        left: &Left,
        generator: &Generator,
        built: Built<'_>,
    ) -> Stand {
        let grid = generator.sphere().blocks();
        let low = [point.u, point.v].map(|at| at.floor() - f64::from(SPAN));
        let mut wood = [Wood::Clear; SIDE * SIDE];
        let mut top_m = f64::NEG_INFINITY;
        for (index, column_wood) in wood.iter_mut().enumerate() {
            let [i, j] = [index % SIDE, index / SIDE].map(|n| n as f64);
            let middle = SurfacePoint::new(point.sector, low[0] + i + 0.5, low[1] + j + 0.5);
            let column = grid.column_of(grid.wrapped(middle));
            if let Some((top, root)) = (built.trunks).wood(built.cells, column, feet_m) {
                *column_wood = match left.has(root) {
                    true => {
                        top_m = top_m.max(top);
                        Wood::Left
                    }
                    false => Wood::Other,
                };
            }
        }
        Stand {
            wood,
            corner: [low[0] - point.u, low[1] - point.v],
            top_m,
        }
    }

    /// The worst wood a body at `at`, blocks from where it stands, turned
    /// any way with its spine out to `reach` blocks from its middle, meets:
    /// what is in a column its width at a point of it spans.
    fn swept(&self, at: [f64; 2], reach: f64) -> Wood {
        let range = |axis: usize| {
            let index = |blocks: f64| {
                (blocks - self.corner[axis])
                    .floor()
                    .clamp(0.0, (SIDE - 1) as f64) as usize
            };
            index(at[axis] - reach - BODY_HALF - 1.0)..=index(at[axis] + reach + BODY_HALF)
        };
        let mut worst = Wood::Clear;
        for j in range(1) {
            for i in range(0) {
                let wood = self.wood[j * SIDE + i];
                if wood <= worst {
                    continue;
                }
                let corner = [self.corner[0] + i as f64, self.corner[1] + j as f64];
                if sweeps(at, reach, corner) {
                    worst = wood;
                }
            }
        }
        worst
    }
}

/// Whether a body at `at`, turned any way with its spine out to `reach`
/// blocks from its middle, meets the column whose corner, its lowest `u`
/// and `v`, is at `corner`, both blocks from one place: its width at a
/// point of it spans the column as [`collision::columns`] spans one. With
/// no reach, the columns its width spans where it stands.
fn sweeps(at: [f64; 2], reach: f64, corner: [f64; 2]) -> bool {
    let [du, dv] = [0, 1].map(|axis| {
        let (low, high) = (corner[axis] - BODY_HALF, corner[axis] + 1.0 + BODY_HALF);
        (low - at[axis]).max(at[axis] - high).max(0.0)
    });
    du * du + dv * dv <= reach * reach
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every kind's longest body, at its largest build and where blocks
    /// are smallest, at a sector's corner, reaches no farther than the wood
    /// about a body is read wide enough for at the longest way out.
    #[test]
    fn every_kind_reaches_within_the_wood_read_about_it() {
        for kind in gaia::fauna::KINDS {
            let size = (kind.builds.iter())
                .map(|build| build.size.1)
                .fold(0.0, f64::max);
            let reach_m = kind.reach_m().into_iter().fold(0.0, f64::max) * size;
            let blocks = reach_m * core::f64::consts::SQRT_2 / BLOCK_M;
            assert!(
                blocks <= f64::from(REACH_MOST),
                "{}: {blocks} blocks",
                kind.name
            );
        }
    }

    /// Whether a body with its feet at `feet_m` at `point` turns every way
    /// there clear of the wood of every trunk, its spine out to `reach`
    /// blocks from its middle.
    pub(crate) fn turns_clear(
        point: SurfacePoint,
        feet_m: f64,
        reach: f64,
        generator: &Generator,
        built: Built<'_>,
    ) -> bool {
        let stand = Stand::read(point, feet_m, &Left::NONE, generator, built);
        stand.swept([0.0; 2], reach) == Wood::Clear
    }
}
