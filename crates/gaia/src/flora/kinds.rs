//! What may grow, as data. A kind is the places it likes, how it branches
//! and how big, and what its blocks are made of; a flower is the places it
//! likes and its petals. A changed kind is a new [`crate::VERSION`].

use super::field::Place;
use super::lsystem::{Production, Rule};
use worldgen::noise::smoothstep;

/// What a block of a plant is made of. The world says how each looks.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Tissue {
    /// A trunk's or a branch's brown wood.
    Wood = 0,
    /// A birch's white bark.
    Bark = 1,
    /// Broad leaves, in the sun.
    Leaf = 2,
    /// Broad leaves of a deeper green.
    Shade = 3,
    /// A conifer's needles.
    Needle = 4,
    /// A buttercup's yellow petals.
    Buttercup = 5,
    /// A daisy's white petals.
    Daisy = 6,
    /// A bellflower's violet petals.
    Bellflower = 7,
    /// A campion's pink petals.
    Campion = 8,
}

/// Where on the field something grows, and how common it is there.
#[derive(Debug)]
pub(crate) struct Habitat {
    /// Meadow `0` to wood `1`: where it comes in, where it is most at home,
    /// and where it gives out. A bound past `0..=1` is never reached.
    pub wild: [f64; 3],
    /// How high it grows, shares of the relief: from the first to the
    /// second it comes in, from the third to the fourth it gives out.
    pub heights: [f64; 4],
    /// How common it is where it is most at home, against the others.
    pub weight: f64,
}

/// One kind of plant.
#[derive(Debug)]
pub(crate) struct Kind {
    pub habitat: Habitat,
    pub rule: Rule,
    /// How big it grows against its rule: most of its plants between the
    /// first two, and one variant of every world, its elder, the third.
    pub sizes: [f64; 3],
    pub look: Look,
}

/// One kind of flower, which blooms in beds.
#[derive(Debug)]
pub(crate) struct Flower {
    pub habitat: Habitat,
    pub petals: Tissue,
}

/// What a kind's blocks are made of.
#[derive(Debug)]
pub(crate) struct Look {
    pub wood: Tissue,
    /// Where its wood hides in its leaves, as a shrub's does, how deep its
    /// lowest masses of leaves sink into the ground, a share of their
    /// height: it stands on them, sunk in the grass, and meets the ground
    /// along a foot rather than at a point. `None` where its wood shows and
    /// its trunk stands it.
    pub sunk: Option<f64>,
    /// Its leaves, and the share of them in the second.
    pub leaves: [Tissue; 2],
    pub dapple: f64,
    /// How much of a gathering of leaves' edge the wind has taken, a share of
    /// its radius.
    pub ragged: f64,
    /// How much flatter than wide a gathering of leaves is.
    pub squash: f64,
}

impl Habitat {
    /// How likely it is to be the one that grows at a place, against the
    /// others.
    pub(crate) fn weight(&self, place: Place) -> f64 {
        let [low, home, high] = self.wild;
        let wild = if place.wild < home {
            smoothstep(low, home, place.wild)
        } else {
            1.0 - smoothstep(home, high, place.wild)
        };
        let [a, b, c, d] = self.heights;
        let height = smoothstep(a, b, place.height) * (1.0 - smoothstep(c, d, place.height));
        self.weight * wild * height
    }
}

/// Which kind grows at a place, by a roll in `0..1`; `None` where no kind
/// would.
pub(crate) fn pick_kind(place: Place, roll: f64) -> Option<u16> {
    pick(KINDS.iter().map(|kind| &kind.habitat), place, roll)
}

/// What a bed of flowers at a place blooms in, by a roll in `0..1`; `None`
/// where no flower would.
pub(crate) fn pick_bloom(place: Place, roll: f64) -> Option<Tissue> {
    let flower = pick(FLOWERS.iter().map(|flower| &flower.habitat), place, roll)?;
    Some(FLOWERS[usize::from(flower)].petals)
}

/// Which of some habitats a roll in `0..1` falls in at a place, each as
/// wide as its weight there.
fn pick<'a>(
    habitats: impl Iterator<Item = &'a Habitat> + Clone,
    place: Place,
    roll: f64,
) -> Option<u16> {
    let total: f64 = habitats.clone().map(|habitat| habitat.weight(place)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut left = roll * total;
    let mut last = None;
    for (index, habitat) in habitats.enumerate() {
        let weight = habitat.weight(place);
        if weight <= 0.0 {
            continue;
        }
        if left < weight {
            return Some(index as u16);
        }
        left -= weight;
        // Rounding can leave a sliver past the last weight: it is the last
        // that has one.
        last = Some(index as u16);
    }
    last
}

/// What grows: a broadleaf, a conifer, a birch and a shrub.
pub(crate) static KINDS: &[Kind] = &[BROADLEAF, CONIFER, BIRCH, SHRUB];
/// The flowers of the meadows.
pub(crate) static FLOWERS: &[Flower] = &[BUTTERCUP, DAISY, BELLFLOWER, CAMPION];

/// A round crown on a stout trunk, three boughs to a fork.
const BROADLEAF: Kind = Kind {
    habitat: Habitat {
        wild: [0.25, 0.85, 1.6],
        heights: [-1.0, 0.0, 0.16, 0.3],
        weight: 1.0,
    },
    rule: Rule {
        axiom: "FFA",
        productions: &[Production {
            letter: b'A',
            into: &[("F[&FLA]/[&FLA]/[&FLA]", 3), ("F[&FLA]/[&FLA]", 2)],
        }],
        steps: 3,
        step: 2.4,
        girth: 0.9,
        shorten: 0.74,
        thin: 0.6,
        pitch: 36.0,
        turn: 20.0,
        roll: 125.0,
        wander: 0.25,
        rise: 0.12,
        crown: 2.4,
    },
    sizes: [0.9, 1.4, 4.0],
    look: Look {
        wood: Tissue::Wood,
        sunk: None,
        leaves: [Tissue::Leaf, Tissue::Shade],
        dapple: 0.3,
        ragged: 0.35,
        squash: 0.8,
    },
};

/// A spire: a straight trunk in whorls of four, the lowest the longest.
const CONIFER: Kind = Kind {
    habitat: Habitat {
        wild: [0.3, 0.95, 1.6],
        heights: [0.04, 0.16, 0.34, 0.44],
        weight: 1.1,
    },
    rule: Rule {
        axiom: "FA",
        productions: &[
            Production {
                letter: b'A',
                into: &[("FL[&&B]/[&&B]/[&&B]/[&&B]A", 1)],
            },
            Production {
                letter: b'B',
                into: &[("FLB", 1)],
            },
        ],
        steps: 6,
        step: 2.6,
        girth: 0.5,
        shorten: 0.45,
        thin: 0.6,
        pitch: 40.0,
        turn: 15.0,
        roll: 90.0,
        wander: 0.2,
        rise: -0.04,
        crown: 1.5,
    },
    sizes: [0.9, 1.4, 3.0],
    look: Look {
        wood: Tissue::Wood,
        sunk: None,
        leaves: [Tissue::Needle, Tissue::Needle],
        dapple: 0.0,
        ragged: 0.3,
        squash: 0.7,
    },
};

/// A slender white trunk under a light, open crown.
const BIRCH: Kind = Kind {
    habitat: Habitat {
        wild: [0.05, 0.45, 0.95],
        heights: [-1.0, 0.0, 0.22, 0.34],
        weight: 0.6,
    },
    rule: Rule {
        axiom: "FFFA",
        productions: &[
            Production {
                letter: b'A',
                into: &[("F[&FLB]/[&FLB]/A", 2), ("F[&FLB]/A", 1)],
            },
            Production {
                letter: b'B',
                into: &[("FL", 2), ("", 1)],
            },
        ],
        steps: 4,
        step: 2.4,
        girth: 0.5,
        shorten: 0.8,
        thin: 0.6,
        pitch: 42.0,
        turn: 20.0,
        roll: 140.0,
        wander: 0.25,
        rise: 0.05,
        crown: 2.0,
    },
    sizes: [0.9, 1.3, 2.8],
    look: Look {
        wood: Tissue::Bark,
        sunk: None,
        leaves: [Tissue::Leaf, Tissue::Shade],
        dapple: 0.5,
        ragged: 0.4,
        squash: 0.9,
    },
};

/// A low bush of five stems, leafy to the ground.
const SHRUB: Kind = Kind {
    habitat: Habitat {
        wild: [-0.6, 0.45, 1.3],
        heights: [-1.0, 0.0, 0.3, 0.42],
        weight: 1.4,
    },
    rule: Rule {
        axiom: "F[&A]/[&A]/[&A]/[&A]/[&A]",
        productions: &[Production {
            letter: b'A',
            into: &[("FL[+A]", 1), ("FLA", 1)],
        }],
        steps: 2,
        step: 2.2,
        girth: 0.4,
        shorten: 0.65,
        thin: 0.8,
        pitch: 50.0,
        turn: 30.0,
        roll: 72.0,
        wander: 0.3,
        rise: 0.1,
        crown: 1.5,
    },
    sizes: [0.55, 0.85, 0.85],
    look: Look {
        wood: Tissue::Wood,
        sunk: Some(1.0 / 3.0),
        leaves: [Tissue::Shade, Tissue::Leaf],
        dapple: 0.25,
        ragged: 0.3,
        squash: 0.8,
    },
};

/// Yellow, low and everywhere in a meadow.
const BUTTERCUP: Flower = Flower {
    habitat: Habitat {
        wild: [-1.0, 0.1, 0.5],
        heights: [-1.0, 0.0, 0.2, 0.32],
        weight: 1.0,
    },
    petals: Tissue::Buttercup,
};

/// White, in the lowest meadows.
const DAISY: Flower = Flower {
    habitat: Habitat {
        wild: [-1.0, 0.0, 0.4],
        heights: [-1.0, 0.0, 0.14, 0.24],
        weight: 0.8,
    },
    petals: Tissue::Daisy,
};

/// Violet, up toward the treeline and the edge of the wood.
const BELLFLOWER: Flower = Flower {
    habitat: Habitat {
        wild: [-0.2, 0.3, 0.6],
        heights: [0.04, 0.14, 0.3, 0.42],
        weight: 0.7,
    },
    petals: Tissue::Bellflower,
};

/// Pink, where the meadow meets the shrubs.
const CAMPION: Flower = Flower {
    habitat: Habitat {
        wild: [-0.2, 0.25, 0.6],
        heights: [-1.0, 0.0, 0.18, 0.3],
        weight: 0.5,
    },
    petals: Tissue::Campion,
};
