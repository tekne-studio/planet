//! Where plants stand. The address is cut into spots of eight blocks; each
//! draws from its key whether something would grow there, where its root
//! is, which shape it has and which way it turns, and a rank. Plants are
//! settled as if sown one by one from the highest rank down, each where no
//! plant sown before it shares a block: a spot grows unless a neighbour that
//! outranks it, reaches as far and grows itself is there first. So no two
//! plants share a block, across a seam as anywhere, and a wood packs as
//! close as its plants' reaches allow. The ground under a root is asked
//! last, and only of the plants that are left, so what is built over the
//! ground takes plants away and never moves one.

use std::collections::HashMap;

use topology::{Address, BLOCK_M, Column, Grid, Sector, SurfacePoint, vec3};

use super::field::{BEACH_M, Field, Place};
use super::kinds;
use super::shape::MAX_REACH;
use super::stream::Stream;
use super::{Flora, Plant, VARIANTS};
use worldgen::noise::smoothstep;
use worldgen::{Generator, Material};

/// Blocks a spot spans a side, as a power of two.
const SPOT_BITS: u32 = 3;
/// How many spots away a plant may meet another. A root stands anywhere in
/// its spot, so spots one more apart than this hold roots farther apart than
/// two of the widest plants reach.
const NEAR: i32 = 2;
const _: () =
    assert!((NEAR + 1) * (1 << SPOT_BITS) - ((1 << SPOT_BITS) - 1) > 2 * MAX_REACH as i32);
const SPOT: u64 = 0x5_2000;

/// The chance a spot holds a plant, a meadow's lone trees to a wood's crowd,
/// before what the place grows at all.
const MEADOW: f64 = 0.03;
const WOOD: f64 = 0.95;
/// Blocks between the points the slope under a root is measured across.
const SLOPE_BLOCKS: f64 = 2.0;
/// How upright the ground is, the cosine of its tilt: plants thin out from
/// the first to the second, and none grow past it.
const GENTLE: f64 = 0.9;
const STEEP: f64 = 0.78;

/// What a spot would grow, before its neighbours and its ground have a say.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    /// The spot it is drawn for, in its own sector.
    spot: Column,
    root: Column,
    shape: u16,
    turn: u8,
    reach: u32,
    /// Which of two that meet is sown first: the higher.
    rank: (u64, Column),
    /// Against how upright the ground must be, a share of the way from steep
    /// to gentle.
    footing: f64,
}

/// Every seedling whose root stands in a square of a sector, `side` blocks
/// from `corner`, in the order of their spots.
pub(crate) fn seedlings(
    generator: &Generator,
    flora: &Flora,
    sector: Sector,
    corner: [u32; 2],
    side: u32,
) -> Vec<Seedling> {
    let blocks = generator.sphere().blocks();
    let spots = blocks
        .coarsened(SPOT_BITS)
        .expect("a body holds a spot a side");
    let first = corner.map(|c| c >> SPOT_BITS);
    let last = corner.map(|c| ((c + side).min(blocks.side()).max(c + 1) - 1) >> SPOT_BITS);
    let mut placing = Placing {
        generator,
        flora,
        blocks,
        spots,
        field: Field::new(generator),
        known: Known::new(sector, first, last),
    };
    let inside = |c: Column| {
        let (u, v) = (u32::from(c.u), u32::from(c.v));
        (corner[0]..corner[0] + side).contains(&u) && (corner[1]..corner[1] + side).contains(&v)
    };
    let mut seedlings = Vec::new();
    for j in first[1]..=last[1] {
        for i in first[0]..=last[0] {
            let spot = Column::new(sector, i as u16, j as u16);
            let Some(c) = placing.candidate(spot) else {
                continue;
            };
            if inside(c.root) && placing.sown(c) {
                seedlings.push(Seedling {
                    root: c.root,
                    shape: c.shape,
                    turn: c.turn,
                    footing: c.footing,
                });
            }
        }
    }
    seedlings
}

/// A plant as sown, before the ground under it has a say: the seed and the
/// field alone decide it, so any reader of the ground, in full detail or as
/// a far patch holds it, roots the same seedlings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Seedling {
    /// The column its trunk rises in.
    pub root: Column,
    /// Which of [`Flora::shapes`] it has.
    pub shape: u16,
    /// Quarter turns about its root, from `u` toward `v`.
    pub turn: u8,
    /// Against how upright the ground must be, a share of the way from steep
    /// to gentle.
    footing: f64,
}

impl Seedling {
    /// Whether it takes on ground of a material, this high over the sea,
    /// whose tilt has this cosine: grass or forest, over the beach, upright
    /// enough.
    pub fn takes(&self, material: Material, height_m: f64, upright: f64) -> bool {
        matches!(material, Material::Grass | Material::Forest)
            && height_m >= BEACH_M
            && self.footing < smoothstep(STEEP, GENTLE, upright)
    }

    /// The plant it grows into on ground this high over the datum: its trunk
    /// rises from the block the ground runs through.
    pub fn rooted(&self, height_m: f64) -> Plant {
        Plant {
            root: Address::new(self.root, libm::floor(height_m / BLOCK_M) as i16),
            shape: self.shape,
            turn: self.turn,
        }
    }
}

/// The plant a seedling grows into, if the ground under it in full detail,
/// with what is built, takes it: so a plot grows nothing.
pub(crate) fn stand(generator: &Generator, seedling: &Seedling) -> Option<Plant> {
    let root = seedling.root;
    let blocks = generator.sphere().blocks();
    let radius_m = generator.sphere().radius_m();
    let at = |du: f64, dv: f64| {
        let point = SurfacePoint::new(
            root.sector,
            f64::from(root.u) + 0.5 + du,
            f64::from(root.v) + 0.5 + dv,
        );
        let direction = blocks.direction(point);
        let ground = generator.sample(direction);
        (
            direction,
            ground,
            vec3::scale(direction, radius_m + ground.height_m),
        )
    };
    let (up, ground, here) = at(0.0, 0.0);
    // The material first: most of the world is no meadow, and it is one
    // sample against three.
    if !seedling.takes(ground.material, ground.height_m, 1.0) {
        return None;
    }
    let (_, _, across) = at(SLOPE_BLOCKS, 0.0);
    let (_, _, along) = at(0.0, SLOPE_BLOCKS);
    let normal = vec3::cross(vec3::sub(across, here), vec3::sub(along, here));
    let upright = vec3::dot(normal, up) / vec3::length(normal);
    seedling
        .takes(ground.material, ground.height_m, upright)
        .then(|| seedling.rooted(ground.height_m))
}

struct Placing<'a> {
    generator: &'a Generator,
    flora: &'a Flora,
    blocks: Grid,
    spots: Grid,
    field: Field<'a>,
    known: Known,
}

impl Placing<'_> {
    /// Whether a candidate is sown: no neighbour that outranks it and meets
    /// it is. Each step of the question goes to a higher rank, so it ends,
    /// and it reads the same on every machine however far it goes.
    fn sown(&mut self, c: Candidate) -> bool {
        if let Some(sown) = self.known.at(c.spot).sown {
            return sown;
        }
        let mut sown = true;
        'ask: for dj in -NEAR..=NEAR {
            for di in -NEAR..=NEAR {
                let at = [i32::from(c.spot.u) + di, i32::from(c.spot.v) + dj];
                let Some(n) = self.candidate_at(c.spot.sector, at) else {
                    continue;
                };
                if n.rank > c.rank && meet(self.blocks, &c, &n) && self.sown(n) {
                    sown = false;
                    break 'ask;
                }
            }
        }
        self.known.at(c.spot).sown = Some(sown);
        sown
    }

    /// The candidate of the spot at a sector's spot coordinates, which may
    /// lie over a seam. Past a corner of the sector, both of them out, there
    /// is no face: nothing grows there.
    fn candidate_at(&mut self, sector: Sector, [i, j]: [i32; 2]) -> Option<Candidate> {
        let side = self.spots.side() as i32;
        let out = |x: i32| !(0..side).contains(&x);
        let spot = match (out(i), out(j)) {
            (false, false) => Column::new(sector, i as u16, j as u16),
            (true, true) => return None,
            _ => {
                let over = SurfacePoint::new(sector, f64::from(i) + 0.5, f64::from(j) + 0.5);
                self.spots.column_of(self.spots.wrapped(over))
            }
        };
        self.candidate(spot)
    }

    /// What a spot would grow, from its key and the place its root is in.
    fn candidate(&mut self, spot: Column) -> Option<Candidate> {
        if let Some(drawn) = self.known.at(spot).drawn {
            return drawn;
        }
        let drawn = self.draw(spot);
        self.known.at(spot).drawn = Some(drawn);
        drawn
    }

    fn draw(&mut self, spot: Column) -> Option<Candidate> {
        let seed = self.generator.recipe().seed;
        let mut stream = Stream::new(&[
            seed ^ SPOT,
            spot.sector.index() as u64,
            u64::from(spot.u),
            u64::from(spot.v),
        ]);
        let rank = (stream.next(), spot);
        let offset = [stream.below(1 << SPOT_BITS), stream.below(1 << SPOT_BITS)];
        let root = Column::new(
            spot.sector,
            ((u32::from(spot.u) << SPOT_BITS) + offset[0]) as u16,
            ((u32::from(spot.v) << SPOT_BITS) + offset[1]) as u16,
        );
        let place = self.field.at(
            root.sector,
            f64::from(root.u) + 0.5,
            f64::from(root.v) + 0.5,
        );
        if stream.unit() >= density(place) {
            return None;
        }
        let kind = kinds::pick_kind(place, stream.unit())?;
        let shape = kind * VARIANTS + stream.below(u32::from(VARIANTS)) as u16;
        Some(Candidate {
            spot,
            root,
            shape,
            turn: stream.below(4) as u8,
            reach: self.flora.shapes[usize::from(shape)].reach(),
            rank,
            footing: stream.unit(),
        })
    }
}

/// What is known of a spot so far: what it would grow, and whether that is
/// sown.
#[derive(Clone, Copy, Default)]
struct Spot {
    drawn: Option<Option<Candidate>>,
    sown: Option<bool>,
}

/// What is known of the spots a square asks about: its own and those
/// [`NEAR`] of them in a grid, any farther a question leads to, or over a
/// seam, by the way.
struct Known {
    sector: Sector,
    low: [i32; 2],
    width: usize,
    height: usize,
    near: Vec<Spot>,
    far: HashMap<Column, Spot>,
}

impl Known {
    fn new(sector: Sector, first: [u32; 2], last: [u32; 2]) -> Known {
        let low = first.map(|x| x as i32 - NEAR);
        let width = (last[0] - first[0]) as usize + 1 + 2 * NEAR as usize;
        let height = (last[1] - first[1]) as usize + 1 + 2 * NEAR as usize;
        Known {
            sector,
            low,
            width,
            height,
            near: vec![Spot::default(); width * height],
            far: HashMap::new(),
        }
    }

    fn at(&mut self, spot: Column) -> &mut Spot {
        let i = i32::from(spot.u) - self.low[0];
        let j = i32::from(spot.v) - self.low[1];
        let inside = |x: i32, n: usize| (0..n as i32).contains(&x);
        if spot.sector == self.sector && inside(i, self.width) && inside(j, self.height) {
            &mut self.near[j as usize * self.width + i as usize]
        } else {
            self.far.entry(spot).or_default()
        }
    }
}

/// Whether two plants would share a block: their roots are no farther
/// apart than their reaches together.
fn meet(blocks: Grid, a: &Candidate, b: &Candidate) -> bool {
    let reach = u64::from(a.reach + b.reach);
    blocks
        .distance_squared(a.root, b.root)
        .is_some_and(|d2| d2 <= reach * reach)
}

/// The chance a spot at a place holds a plant.
pub(crate) fn density(place: Place) -> f64 {
    place.growth * (MEADOW + (WOOD - MEADOW) * place.wild * place.wild)
}
