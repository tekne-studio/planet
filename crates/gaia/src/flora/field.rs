//! One slow field says what kind of place a point is: a meadow, a shrubland
//! or a wood, and how much grows there at all. It is worked out at the
//! corners of the plot lattice, where asking the ground too is cheap, and
//! read between them.

use std::collections::BTreeMap;

use topology::{Sector, SurfacePoint};

use worldgen::noise::{fbm, smoothstep};
use worldgen::{Direction, Generator, Material, Sample};

/// Blocks between the corners the field is worked out at, as a power of
/// two: a plot's side.
const CORNER_BITS: u32 = 6;
/// Metres between the corners: the finest the field's wild turns are ever
/// read, whoever reads them.
pub(crate) const CORNER_M: f64 = (1u32 << CORNER_BITS) as f64 * topology::BLOCK_M;
const WILD: u64 = 0x5_1000;
/// The field's turns per radius of the reference body: a glade or a copse
/// every few hundred metres.
const WILD_CYCLES: f64 = 45.0;
/// How far the field moves a place from the middle, either way.
const WILD_SPREAD: f64 = 0.9;
/// How far the ground leans a place: a forest toward the wood, a grassland
/// toward the meadow.
const FOREST_LEAN: f64 = 0.4;
const GRASS_LEAN: f64 = -0.25;
/// Metres over the sea of the highest sand the ground is painted with:
/// nothing roots under it, so nothing of a wood stands on a beach.
pub(crate) const BEACH_M: f64 = scene::SAND_HIGHEST_M as f64;
/// Reference metres over the sea across which growth comes in: from where
/// the ground stops being sand, and a root takes only over [`BEACH_M`].
const SHORE_M: [f64; 2] = [2.0, 7.0];
/// Shares of the relief across which growth gives out: the treeline, under
/// the rock that begins at `0.45`.
const TREELINE: [f64; 2] = [0.3, 0.42];

/// What kind of place a point is, for what grows there.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Place {
    /// A meadow at `0`, a shrubland about the middle, a wood at `1`.
    pub wild: f64,
    /// How much of what the place would hold grows, `0..=1`: nothing on
    /// sand, rock, snow or water, little at the shore and the treeline.
    pub growth: f64,
    /// How high it is, a share of the relief.
    pub height: f64,
}

/// The field over one generator's ground, its corners kept as they are
/// worked out.
pub(crate) struct Field<'a> {
    generator: &'a Generator,
    corners: BTreeMap<(Sector, u32, u32), Place>,
    /// The cell last read and its four corners: the next point asked is
    /// most often in it.
    last: Option<((Sector, u32, u32), [Place; 4])>,
}

impl<'a> Field<'a> {
    pub(crate) fn new(generator: &'a Generator) -> Field<'a> {
        Field {
            generator,
            corners: BTreeMap::new(),
            last: None,
        }
    }

    /// The place at a point of a sector, in blocks: its four corners
    /// blended. Each sector reads its own corners, so a point means one
    /// place whoever asks.
    pub(crate) fn at(&mut self, sector: Sector, u: f64, v: f64) -> Place {
        let side = self.generator.sphere().blocks().side();
        let step = (1u32 << CORNER_BITS).min(side);
        let last = side / step - 1;
        let (su, sv) = (u / f64::from(step), v / f64::from(step));
        let (cu, cv) = (
            (libm::floor(su) as u32).min(last),
            (libm::floor(sv) as u32).min(last),
        );
        let (tu, tv) = (su - f64::from(cu), sv - f64::from(cv));
        let cell = (sector, cu, cv);
        let [a, b, c, d] = match self.last {
            Some((last, corners)) if last == cell => corners,
            _ => {
                let mut corner = |du: u32, dv: u32| {
                    let key = (sector, (cu + du) * step, (cv + dv) * step);
                    *self
                        .corners
                        .entry(key)
                        .or_insert_with(|| worked_out(self.generator, key))
                };
                let corners = [corner(0, 0), corner(1, 0), corner(0, 1), corner(1, 1)];
                self.last = Some((cell, corners));
                corners
            }
        };
        let blend = |f: fn(Place) -> f64| {
            let low = f(a) + (f(b) - f(a)) * tu;
            let high = f(c) + (f(d) - f(c)) * tu;
            low + (high - low) * tv
        };
        Place {
            wild: blend(|p| p.wild),
            growth: blend(|p| p.growth),
            height: blend(|p| p.height),
        }
    }
}

/// The place at one corner, from the ground as the recipe made it: what
/// was built since never moves the field.
fn worked_out(generator: &Generator, (sector, u, v): (Sector, u32, u32)) -> Place {
    let point = SurfacePoint::new(sector, f64::from(u), f64::from(v));
    let d = generator.sphere().blocks().direction(point);
    place(generator, d, generator.reference_sample(d, 0.0))
}

/// The place at a direction where the ground is as a sample of it says:
/// what a corner is worked out from, and what a reader too far off for the
/// corners to tell reads at its own samples.
pub(crate) fn place(generator: &Generator, d: Direction, ground: Sample) -> Place {
    placed(generator, turn(generator, d), ground)
}

/// The field's turn at a direction, before the ground leans it: the one
/// costly part of a place, slow enough to be read between points
/// [`CORNER_M`] apart.
pub(crate) fn turn(generator: &Generator, d: Direction) -> f64 {
    fbm(
        generator.recipe().seed ^ WILD,
        d.map(|x| x * WILD_CYCLES),
        3,
        0.5,
    )
}

/// The place where the field turns as `turn` says and the ground is as a
/// sample of it says.
pub(crate) fn placed(generator: &Generator, turn: f64, ground: Sample) -> Place {
    let relief = generator.recipe().params.relief_m.max(1.0);
    let height = ground.height_m / relief;
    let lean = match ground.material {
        Material::Forest => FOREST_LEAN,
        Material::Grass => GRASS_LEAN,
        _ => {
            return Place {
                wild: 0.0,
                growth: 0.0,
                height,
            };
        }
    };
    Place {
        wild: (0.5 + turn * WILD_SPREAD + lean).clamp(0.0, 1.0),
        growth: smoothstep(SHORE_M[0], SHORE_M[1], ground.height_m)
            * (1.0 - smoothstep(TREELINE[0], TREELINE[1], height)),
        height,
    }
}
