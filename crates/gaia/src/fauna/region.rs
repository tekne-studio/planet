//! Where a herd may stand, and an animal's number: its kind, its herd's
//! region and its place in the herd. Whoever knows a world's seed finds
//! the herd a region holds from it, a client drawing it and the world
//! keeping a pet alike.

use glam::DVec2;
use topology::Sector;

use crate::fauna::herd::Herd;
use crate::fauna::kind::Kind;
use crate::fauna::random::Random;

/// How an animal's number is laid out, from its lowest bit: its place in
/// its herd, its region's `j` and `i` in its sector, the sector, and its
/// kind's number. Fifty-three bits in all, so a number is whole wherever
/// JSON is read.
pub const PLACE_BITS: u32 = 6;
pub const REGION_BITS: u32 = 18;
const SECTOR_BITS: u32 = 3;
const KIND_BITS: u32 = 8;
const J_SHIFT: u32 = PLACE_BITS;
const I_SHIFT: u32 = J_SHIFT + REGION_BITS;
const SECTOR_SHIFT: u32 = I_SHIFT + REGION_BITS;
const KIND_SHIFT: u32 = SECTOR_SHIFT + SECTOR_BITS;

/// Spots a region tries for its herd's home.
pub const HOMES: usize = 4;

/// Where a herd may stand: a region of a sector, by a kind, the side of a
/// region its kind's `region_m`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Region {
    pub kind: u8,
    pub sector: Sector,
    pub i: i64,
    pub j: i64,
}

impl Region {
    /// The number of the animal at `place` of this region's herd.
    pub fn number(self, place: usize) -> u64 {
        let field = |value: u64, bits: u32, shift: u32| (value & ((1 << bits) - 1)) << shift;
        field(u64::from(self.kind), KIND_BITS, KIND_SHIFT)
            | field(self.sector.index() as u64, SECTOR_BITS, SECTOR_SHIFT)
            | field(self.i as u64, REGION_BITS, I_SHIFT)
            | field(self.j as u64, REGION_BITS, J_SHIFT)
            | field(place as u64, PLACE_BITS, 0)
    }

    /// The region of the herd a number names, and the animal's place in
    /// it; none for a number no herd gives.
    pub fn of(number: u64) -> Option<(Region, usize)> {
        let field = |bits: u32, shift: u32| (number >> shift) & ((1 << bits) - 1);
        if number >> (KIND_SHIFT + KIND_BITS) != 0 {
            return None;
        }
        let region = Region {
            kind: field(KIND_BITS, KIND_SHIFT) as u8,
            sector: Sector::new(field(SECTOR_BITS, SECTOR_SHIFT) as u8)?,
            i: field(REGION_BITS, I_SHIFT) as i64,
            j: field(REGION_BITS, J_SHIFT) as i64,
        };
        Some((region, field(PLACE_BITS, 0) as usize))
    }

    /// Its kind, by its number among [`crate::fauna::KINDS`].
    pub fn kind(self) -> Option<&'static Kind> {
        crate::fauna::KINDS
            .into_iter()
            .find(|kind| kind.number == self.kind)
    }

    /// The herd this region holds by a world's seed, when it holds one,
    /// and the spots it tries for its home in turn, as shares of the
    /// region along `i` and `j`: the first of them an animal may stand on
    /// is its home, and with none it holds no herd after all.
    pub fn herd(self, world_seed: u64) -> Option<(Herd, [DVec2; HOMES])> {
        let kind = self.kind()?;
        let parts = [
            world_seed,
            u64::from(kind.number),
            self.sector.index() as u64,
            self.i as u64,
            self.j as u64,
        ];
        let mut random = Random::of(&parts);
        if !random.chance(kind.herding.chance) {
            return None;
        }
        let seed = random.draw();
        let homes = core::array::from_fn(|_| DVec2::new(random.unit(), random.unit()));
        Some((Herd::new(kind, seed), homes))
    }

    /// The coat the animal of a number is born with wings in, by a world's
    /// seed, as its herd draws it; none for one born without, and for a
    /// number of no herd.
    pub fn wings(number: u64, world_seed: u64) -> Option<&'static str> {
        let (region, place) = Region::of(number)?;
        region.herd(world_seed)?.0.wings_of(place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_names_its_region_and_place() {
        for (kind, sector, i, j, place) in [
            (1, 0, 0, 0, 0),
            (255, 5, 262_143, 262_143, 63),
            (1, 3, 77, 1_024, 6),
        ] {
            let region = Region {
                kind,
                sector: Sector::new(sector).unwrap(),
                i,
                j,
            };
            let n = region.number(place);
            assert!(n < 1 << 53, "{n:x}");
            assert_eq!(Region::of(n), Some((region, place)));
        }
        assert_eq!(Region::of(1 << 53), None);
        assert_eq!(Region::of(7 << SECTOR_SHIFT), None);
    }

    /// Every kind's herd fits the number.
    #[test]
    fn every_herd_has_a_number_for_each_animal() {
        for kind in crate::fauna::KINDS {
            assert!(kind.herding.size.1 <= 1 << PLACE_BITS, "{}", kind.name);
        }
    }
}
