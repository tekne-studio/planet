//! Where flowers bloom: in beds, each one kind of flower, where the field
//! says meadow. The address is cut into spots of sixteen blocks; each draws
//! from its key whether it holds a bed, where its middle is, how wide it is
//! and which flower. A flower is smaller than a block, so a bed is no cell
//! and nothing of it is stored: the grass in it blooms.

use topology::Sector;

use super::field::{Field, Place};
use super::kinds::{self, Tissue};
use super::stream::Stream;
use worldgen::Generator;
use worldgen::noise::smoothstep;

/// Blocks a bed's spot spans a side, as a power of two.
const BED_BITS: u32 = 4;
const BED: u64 = 0x5_4000;
/// How wide a bed is, blocks from its middle to its rim.
const RADIUS: [f64; 2] = [3.0, 7.5];
const _: () = assert!(RADIUS[1] < (1 << BED_BITS) as f64);
/// The chance a spot holds a bed in the heart of a meadow.
const BEDS: f64 = 0.55;
/// Where on the field beds give out, meadow `0` toward wood `1`.
const MEADOW: [f64; 2] = [0.15, 0.5];

/// A bed of flowers on a sector, in blocks.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bed {
    pub middle: [f64; 2],
    pub radius: f64,
    /// What its flowers' petals are made of.
    pub petals: Tissue,
}

impl Bed {
    /// How much of the grass at a point of its sector blooms: all of it in
    /// the middle, none past the rim.
    pub fn bloom(&self, [u, v]: [f64; 2]) -> f64 {
        let d = libm::hypot(u - self.middle[0], v - self.middle[1]) / self.radius;
        1.0 - smoothstep(0.45, 1.0, d)
    }
}

/// The beds of a sector that reach into a square, kept by the spot each
/// stands on: a bed is narrower than a spot, so a point asks only the spots
/// about its own.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Beds {
    /// The first spot of the square's grid, and how many it spans each way.
    first: [u32; 2],
    span: [u32; 2],
    /// Each spot's bed, where it holds one, `u` fastest.
    spots: Vec<Option<Bed>>,
}

impl Beds {
    /// Every bed, spot by spot, `u` fastest.
    pub fn iter(&self) -> impl Iterator<Item = &Bed> {
        self.spots.iter().flatten()
    }

    /// The beds that may reach a point of the sector, in blocks: those of
    /// its spot and of the spots about it, in the order [`Beds::iter`]
    /// gives them.
    pub fn near(&self, [u, v]: [f64; 2]) -> impl Iterator<Item = &Bed> {
        let spot = [(u, 0), (v, 1)]
            .map(|(at, k)| (libm::floor(at) as i64 >> BED_BITS) - i64::from(self.first[k]));
        let span = self.span.map(i64::from);
        (-1..=1)
            .flat_map(move |dj| (-1..=1).map(move |di| [spot[0] + di, spot[1] + dj]))
            .filter(move |&[i, j]| (0..span[0]).contains(&i) && (0..span[1]).contains(&j))
            .filter_map(move |[i, j]| self.spots[(j * span[0] + i) as usize].as_ref())
    }
}

/// Every bed of a sector that reaches into a square, `side` blocks from
/// `corner`. A bed is the sector's own: one by a seam is cut there.
pub(crate) fn beds(generator: &Generator, sector: Sector, corner: [u32; 2], side: u32) -> Beds {
    let blocks = generator.sphere().blocks().side();
    let reach = RADIUS[1] as u32 + 1;
    let first = corner.map(|c| c.saturating_sub(reach) >> BED_BITS);
    let last = corner.map(|c| ((c + side + reach).min(blocks) - 1) >> BED_BITS);
    let span = [0, 1].map(|k| last[k] - first[k] + 1);
    let mut field = Field::new(generator);
    let seed = generator.recipe().seed;
    let mut spots = Vec::with_capacity((span[0] * span[1]) as usize);
    for j in first[1]..=last[1] {
        for i in first[0]..=last[0] {
            spots.push(None);
            let mut stream = Stream::new(&[
                seed ^ BED,
                sector.index() as u64,
                u64::from(i),
                u64::from(j),
            ]);
            let spot = 1u32 << BED_BITS;
            let middle = [i, j].map(|n| f64::from(n * spot) + stream.unit() * f64::from(spot));
            let place = field.at(sector, middle[0], middle[1]);
            if stream.unit() >= chance(place) {
                continue;
            }
            let Some(petals) = kinds::pick_bloom(place, stream.unit()) else {
                continue;
            };
            let radius = RADIUS[0] + (RADIUS[1] - RADIUS[0]) * stream.unit();
            let near =
                |m: f64, c: u32| m + radius > f64::from(c) && m - radius < f64::from(c + side);
            if near(middle[0], corner[0]) && near(middle[1], corner[1]) {
                *spots.last_mut().expect("this spot's") = Some(Bed {
                    middle,
                    radius,
                    petals,
                });
            }
        }
    }
    Beds { first, span, spots }
}

/// The chance a spot at a place holds a bed.
fn chance(place: Place) -> f64 {
    place.growth * BEDS * (1.0 - smoothstep(MEADOW[0], MEADOW[1], place.wild))
}
