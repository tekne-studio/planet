//! What grows on the ground: trees and shrubs where it is grass or forest,
//! grown from the world's seed. Every machine grows the same plants in the
//! same places, so nothing of them is stored or sent.
//!
//! - [`field`]: what kind of place a point is, a meadow, a shrubland or a
//!   wood, and how much grows there;
//! - [`kinds`]: what may grow, as data;
//! - [`bloom`]: where flowers bloom, in beds;
//! - [`lsystem`]: how a kind branches;
//! - [`shape`]: its blocks, carved from its branches;
//! - [`spot`]: where each plant stands.
//!
//! A world's kinds come in [`VARIANTS`] shapes each, grown once for it; a
//! plant is one of them, turned. Where it stands is read from the ground
//! the generator makes, before what is built over it.

mod bloom;
mod field;
mod kinds;
mod lsystem;
mod shape;
mod spot;
mod stream;

use core::ops::Range;

use topology::{Address, Sector};

use stream::Stream;
use worldgen::{Direction, Generator, Sample};

pub use bloom::{Bed, Beds};
pub use kinds::Tissue;
pub use lsystem::Segment;
pub use shape::{Puff, Shape, TALLEST};
pub use spot::Seedling;

/// Shapes grown of each kind.
pub const VARIANTS: u16 = 16;
const VARIANT: u64 = 0x5_3000;
const ELDER: u64 = 0x5_3100;
/// The share of the ground crowns cover where every spot holds a plant.
const CROWNS: f64 = 0.6;

/// A plant nature grows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Plant {
    /// The block its trunk rises from: the one the ground runs through.
    pub root: Address,
    /// Which of [`Flora::shapes`] it has.
    pub shape: u16,
    /// Quarter turns about its root, from `u` toward `v`.
    pub turn: u8,
}

impl Plant {
    /// Where a block of its shape stands, as an offset from its root block in
    /// address space: turned about the root block's middle.
    pub fn turned(self, [x, y, z]: [i32; 3]) -> [i32; 3] {
        match self.turn % 4 {
            0 => [x, y, z],
            1 => [-y, x, z],
            2 => [-x, -y, z],
            _ => [y, -x, z],
        }
    }
}

/// How much of the ground the crowns cover over a square of it
/// ([`Flora::canopy`]), at the points a reader samples it. The field turns
/// slowly, so its turns are read at a lattice of those points no finer than
/// its corners, and blended between; the ground leans it at each point, so
/// the wood follows the ground the reader draws.
pub struct Canopy<'a> {
    generator: &'a Generator,
    /// Samples between two lattice points, along each side.
    step: u32,
    /// Lattice cells along each side.
    cells: u32,
    /// The field's turn at each lattice point, row by row.
    turns: Vec<f64>,
}

impl<'a> Canopy<'a> {
    fn new(
        generator: &'a Generator,
        side_m: f64,
        samples: u32,
        direction: impl Fn([u32; 2]) -> Direction,
    ) -> Self {
        let samples = samples.max(1);
        // As many samples a step as fit in a corner's span, and a whole
        // number of steps along the side.
        let mut step = (f64::from(samples) * field::CORNER_M / side_m).max(1.0) as u32;
        step = step.min(samples);
        while samples % step != 0 {
            step -= 1;
        }
        let cells = samples / step;
        let turns = (0..=cells)
            .flat_map(|j| (0..=cells).map(move |i| [i * step, j * step]))
            .map(|at| field::turn(generator, direction(at)))
            .collect();
        Canopy {
            generator,
            step,
            cells,
            turns,
        }
    }

    /// The share covered at the reader's sample `[i, j]`, where the ground
    /// is as that sample says, `0` where nothing grows.
    pub fn at(&self, at: [u32; 2], ground: Sample) -> f64 {
        let (step, cells) = (self.step, self.cells);
        let [ci, cj] = at.map(|k| (k / step).min(cells - 1));
        let [fi, fj] = [(at[0], ci), (at[1], cj)]
            .map(|(k, c)| f64::from(k.saturating_sub(c * step)) / f64::from(step));
        let turn = |i: u32, j: u32| self.turns[(j * (cells + 1) + i) as usize];
        let low = turn(ci, cj) + (turn(ci + 1, cj) - turn(ci, cj)) * fi;
        let high = turn(ci, cj + 1) + (turn(ci + 1, cj + 1) - turn(ci, cj + 1)) * fi;
        let turn = low + (high - low) * fj;
        spot::density(field::placed(self.generator, turn, ground)) * CROWNS
    }
}

/// What grows on one world: the shapes of its kinds, grown from its seed.
#[derive(Clone, Debug)]
pub struct Flora {
    shapes: Vec<Shape>,
}

impl Flora {
    /// Grows every variant of every kind for the world a generator makes,
    /// each from its own stream of the seed.
    pub fn grow(generator: &Generator) -> Flora {
        let seed = generator.recipe().seed;
        let shapes = (0..kinds::KINDS.len() as u16)
            .flat_map(|kind| {
                let mut stream = Stream::new(&[seed ^ ELDER, u64::from(kind)]);
                let elder = stream.below(u32::from(VARIANTS)) as u16;
                (0..VARIANTS).map(move |variant| (kind, variant, variant == elder))
            })
            .map(|(kind, variant, elder)| {
                let mut stream =
                    Stream::new(&[seed ^ VARIANT, u64::from(kind), u64::from(variant)]);
                let kind = &kinds::KINDS[usize::from(kind)];
                let [least, most, eldest] = kind.sizes;
                let size = match elder {
                    true => eldest,
                    false => least + (most - least) * stream.unit(),
                };
                // A plant grows its size from the ground; its roots go as
                // deep whatever its size, its trunk carried down to them.
                let mut growth = lsystem::grow(&kind.rule, [0.5, 0.5, 0.0], size, &mut stream);
                // A plant whose wood hides in its leaves has no trunk to
                // stand on: it stands on its lowest masses, sunk into the
                // ground by its share of their height and none deeper.
                if let Some(sunk) = kind.look.sunk {
                    let crown = shape::crown(&growth.leaves, kind.look.squash);
                    let depth = crown
                        .iter()
                        .map(|puff| puff.middle[2] - puff.radii[1] * (1.0 - sunk))
                        .fold(f64::INFINITY, f64::min);
                    if depth.is_finite() {
                        growth.lower(depth);
                    }
                }
                if let Some(&first) = growth.segments.first() {
                    let roots = lsystem::Segment {
                        from: [0.5, 0.5, -f64::from(shape::ROOTS)],
                        to: first.from,
                        radius: first.radius,
                    };
                    growth.segments.insert(0, roots);
                }
                Shape::carve(&growth, &kind.look, &mut stream)
            })
            .collect();
        Flora { shapes }
    }

    /// Every shape a plant may have: [`VARIANTS`] of each kind, kind by kind.
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    /// The blocks of a plant where they stand: offsets from its root block
    /// in address space, turned, with what each is made of.
    pub fn blocks(&self, plant: Plant) -> impl Iterator<Item = ([i32; 3], Tissue)> + '_ {
        self.shapes[usize::from(plant.shape)]
            .blocks()
            .map(move |(p, tissue)| (plant.turned(p), tissue))
    }

    /// The trunk of a plant where it stands: each column of it as an offset
    /// from its root block in address space, turned, with the layers its
    /// wood fills, from the root block's.
    pub fn trunk(&self, plant: Plant) -> impl Iterator<Item = ([i32; 2], Range<i32>)> + '_ {
        let trunk = self.shapes[usize::from(plant.shape)].trunk().iter();
        trunk.map(move |([x, y], layers)| {
            let [u, v, _] = plant.turned([*x, *y, 0]);
            ([u, v], layers.clone())
        })
    }

    /// Every plant whose root stands in a square of a sector, `side` blocks
    /// from `corner`, in the order of their spots. The same square asks the
    /// same plants of every machine, and two squares agree where they
    /// overlap.
    pub fn plants(
        &self,
        generator: &Generator,
        sector: Sector,
        corner: [u32; 2],
        side: u32,
    ) -> Vec<Plant> {
        let seedlings = self.seedlings(generator, sector, corner, side);
        seedlings
            .into_iter()
            .filter_map(|seedling| spot::stand(generator, &seedling))
            .collect()
    }

    /// Every seedling whose root stands in a square of a sector, as
    /// [`Flora::plants`] asks of them before the ground: for a reader that
    /// holds the ground itself.
    pub fn seedlings(
        &self,
        generator: &Generator,
        sector: Sector,
        corner: [u32; 2],
        side: u32,
    ) -> Vec<Seedling> {
        spot::seedlings(generator, self, sector, corner, side)
    }

    /// How much of the ground the crowns cover over a square of it,
    /// `side_m` across, for a reader that samples it at `samples + 1`
    /// points along each side, `[i, j]` toward `direction([i, j])`: one too
    /// far off to draw the plants. The field there, never the plants
    /// themselves.
    pub fn canopy<'a>(
        &self,
        generator: &'a Generator,
        side_m: f64,
        samples: u32,
        direction: impl Fn([u32; 2]) -> Direction,
    ) -> Canopy<'a> {
        Canopy::new(generator, side_m, samples, direction)
    }

    /// Every bed of flowers of a sector that reaches into a square, `side`
    /// blocks from `corner`.
    pub fn beds(&self, generator: &Generator, sector: Sector, corner: [u32; 2], side: u32) -> Beds {
        bloom::beds(generator, sector, corner, side)
    }
}

#[cfg(test)]
mod tests {
    use super::field::Field;
    use super::shape::MAX_REACH;
    use super::spot::density;
    use super::*;
    use worldgen::{Params, Recipe};

    /// The canopy over a square is the field itself at the points its
    /// lattice reads, and blended between them it strays from the field by
    /// a sliver of a cover that reaches 0.6: the wood far off looks as it
    /// would read point by point.
    #[test]
    fn the_canopy_is_the_field_at_its_lattice() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let grid = generator.sphere().blocks();
        let samples = 32;
        // A wood about a kilometre across, on the default seed.
        let wood = topology::place(grid, "0-9AR0000").unwrap().column;
        for side_m in [64.0, 256.0] {
            let side_blocks = side_m / topology::BLOCK_M;
            let direction = |[i, j]: [u32; 2]| {
                let [u, v] = [i, j].map(|k| f64::from(k) / f64::from(samples) * side_blocks);
                grid.direction(topology::SurfacePoint::new(
                    wood.sector,
                    f64::from(wood.u) + u,
                    f64::from(wood.v) + v,
                ))
            };
            let canopy = Canopy::new(&generator, side_m, samples, direction);
            let step = canopy.step;
            assert!(
                step > 1,
                "a {side_m} m square reads its field at fewer points"
            );
            let (mut worst, mut covered): (f64, f64) = (0.0, 0.0);
            for j in 0..=samples {
                for i in 0..=samples {
                    let d = direction([i, j]);
                    let ground = generator.sample(d);
                    let exact = density(field::place(&generator, d, ground)) * CROWNS;
                    let read = canopy.at([i, j], ground);
                    if i % step == 0 && j % step == 0 {
                        assert_eq!(read, exact, "at the lattice point {i}, {j}");
                    }
                    worst = worst.max((read - exact).abs());
                    covered = covered.max(exact);
                }
            }
            assert!(covered > 0.3, "{side_m} m: no wood to read, {covered}");
            assert!(
                worst < 0.04,
                "{side_m} m: strays {worst} between lattice points"
            );
        }
    }

    /// The field, and the chance of a plant it gives, stay in their bounds
    /// wherever they are read and however tall the world is made.
    #[test]
    fn the_field_stays_in_its_bounds() {
        for seed in [0, 1, 0xdead_beef, u64::MAX] {
            for relief_m in [100.0, 1400.0, 5000.0] {
                let recipe = Recipe {
                    params: Params {
                        relief_m,
                        ..Params::default()
                    },
                    ..Recipe::new(seed)
                };
                let generator = Generator::new(recipe).unwrap();
                let side = f64::from(generator.sphere().blocks().side());
                let mut field = Field::new(&generator);
                let mut stream = Stream::new(&[seed]);
                for _ in 0..2000 {
                    let sector = Sector::ALL[stream.below(6) as usize];
                    let place = field.at(sector, stream.unit() * side, stream.unit() * side);
                    assert!((0.0..=1.0).contains(&place.wild), "{place:?}");
                    assert!((0.0..=1.0).contains(&place.growth), "{place:?}");
                    assert!(place.height.is_finite(), "{place:?}");
                    assert!((0.0..=1.0).contains(&density(place)), "{place:?}");
                    let kinds = kinds::KINDS.iter().map(|kind| &kind.habitat);
                    let flowers = kinds::FLOWERS.iter().map(|flower| &flower.habitat);
                    for habitat in kinds.chain(flowers) {
                        let weight = habitat.weight(place);
                        assert!(weight.is_finite() && weight >= 0.0, "{place:?}");
                    }
                }
            }
        }
    }

    /// Every shape rises from its root block, in wood, and no block of it
    /// stands past its reach, nor its reach past what placement keeps apart.
    /// One whose wood hides stands on its leaves: no mass of them sinks
    /// deeper than its kind's share of its height, and one sinks that deep.
    #[test]
    fn every_shape_stands_on_its_root_within_its_reach() {
        for seed in [0, 1, 0xdead_beef, u64::MAX] {
            let generator = Generator::new(Recipe::new(seed)).unwrap();
            for (index, shape) in Flora::grow(&generator).shapes().iter().enumerate() {
                let kind = &kinds::KINDS[index / usize::from(VARIANTS)];
                assert!(shape.reach() <= MAX_REACH);
                assert!(shape.low()[2] + shape.size()[2] as i32 <= TALLEST as i32);
                if let Some(sunk) = kind.look.sunk {
                    let deepest = shape
                        .crown()
                        .iter()
                        .map(|puff| (puff.radii[1] - puff.middle[2]) / puff.radii[1])
                        .fold(f64::NEG_INFINITY, f64::max);
                    assert!(
                        (deepest - sunk).abs() < 1e-9,
                        "its masses sink {deepest} deep"
                    );
                }
                assert!(matches!(
                    shape.at([0, 0, 0]),
                    Some(Tissue::Wood | Tissue::Bark)
                ));
                let reach = shape.reach() as i32;
                for ([x, y, _], _) in shape.blocks() {
                    assert!(x * x + y * y <= reach * reach);
                }
            }
        }
    }
}
