//! Golden hashes over what lives, for fixed seeds. A failure here means a
//! change moved a plant or an animal or changed how a plant grows: raise
//! `gaia::VERSION` and write the new hashes in the same change. These run
//! natively and on wasm32 under node (`.cargo/config.toml`), and must agree:
//! `cargo test -p gaia --target wasm32-unknown-unknown --test golden`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use gaia::fauna::horse::HORSE;
use gaia::fauna::{Herd, Land};
use gaia::flora::Flora;
use glam::DVec2;
use topology::Sector;
use worldgen::{Generator, Recipe};

const SEEDS: [u64; 4] = [0, 1, 0x0000_0000_dead_beef, u64::MAX];

/// FNV-1a over the flora a seed grows: every shape's blocks, the masses its
/// crown is painted as and the branches it grew, and the plants and beds of
/// a square of each sector.
fn flora_fingerprint(seed: u64) -> u64 {
    let generator = Generator::new(Recipe::new(seed)).unwrap();
    let flora = Flora::grow(&generator);
    let mut bytes = Vec::new();
    for shape in flora.shapes() {
        for (p, tissue) in shape.blocks() {
            bytes.extend(p.iter().flat_map(|x| x.to_le_bytes()));
            bytes.push(tissue as u8);
        }
        let bits = |numbers: &[f64]| {
            numbers
                .iter()
                .flat_map(|x| x.to_bits().to_le_bytes())
                .collect::<Vec<_>>()
        };
        for puff in shape.crown() {
            bytes.extend(bits(&puff.middle));
            bytes.extend(bits(&puff.radii));
        }
        for segment in shape.segments() {
            bytes.extend(bits(&segment.from));
            bytes.extend(bits(&segment.to));
            bytes.extend(bits(&[segment.radius]));
        }
    }
    let side = generator.sphere().blocks().side();
    for sector in Sector::ALL {
        let corner = [side / 3, side / 5].map(|x| x + 64 * sector.index() as u32);
        for plant in flora.plants(&generator, sector, corner, 512) {
            let root = plant.root;
            bytes.extend(root.column.u.to_le_bytes());
            bytes.extend(root.column.v.to_le_bytes());
            bytes.extend(root.h.to_le_bytes());
            bytes.extend(plant.shape.to_le_bytes());
            bytes.push(plant.turn);
        }
        for bed in flora.beds(&generator, sector, corner, 512).iter() {
            bytes.extend(bed.middle.iter().flat_map(|x| x.to_le_bytes()));
            bytes.extend(bed.radius.to_le_bytes());
            bytes.push(bed.petals as u8);
        }
    }
    fnv(&bytes)
}

/// FNV-1a.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A meadow with a pond in it, where a herd turns away.
struct Meadow;

impl Land for Meadow {
    fn walkable(&self, at: DVec2) -> bool {
        at.distance(DVec2::new(30.0, 10.0)) > 12.0
    }
}

/// FNV-1a over the animals a seed makes: the horse's tables and its herd's
/// poses at clocks far apart, and every few seconds over ten minutes, as
/// its animals move and string out. Poses alone, worked in `libm`: how an
/// animal is drawn is each client's, in platform math, and no world
/// depends on its bits.
fn fauna_fingerprint(seed: u64) -> u64 {
    let mut bytes = format!("{HORSE:?}").into_bytes();
    let mut herd = Herd::new(&HORSE, seed);
    for animal in 0..herd.len() {
        bytes.extend(herd.wings_of(animal).unwrap_or("").bytes().chain([0]));
    }
    let dense = (0..200).map(|step| 1e5 + 3.0 * f64::from(step));
    for t in [0.0, 37.5, 1e3, 1e7].into_iter().chain(dense) {
        for animal in 0..herd.len() {
            let pose = herd.pose(animal, t, &Meadow);
            let numbers = [pose.at.x, pose.at.y, pose.heading.x, pose.heading.y]
                .into_iter()
                .chain([pose.speed_mps, pose.turn_rps, pose.graze]);
            bytes.extend(numbers.flat_map(|x| x.to_bits().to_le_bytes()));
        }
    }
    assert!(!herd.is_empty(), "a herd");
    fnv(&bytes)
}

#[test]
fn flora_is_frozen() {
    assert_eq!(gaia::VERSION, 1, "a new version writes its own hashes");
    let got: Vec<u64> = SEEDS.map(flora_fingerprint).to_vec();
    assert_eq!(got, GOLDEN_FLORA, "the flora of gaia 1 changed");
}

#[test]
fn fauna_is_frozen() {
    assert_eq!(gaia::VERSION, 1, "a new version writes its own hashes");
    let got: Vec<u64> = SEEDS.map(fauna_fingerprint).to_vec();
    assert_eq!(got, GOLDEN_FAUNA, "the fauna of gaia 1 changed");
}

const GOLDEN_FAUNA: [u64; 4] = [
    0xcbf3_b091_3a7f_8390,
    0x9b66_1285_e360_6767,
    0xf240_8b20_e38d_850a,
    0x2bde_0905_2d5a_e665,
];

const GOLDEN_FLORA: [u64; 4] = [
    0xa043_87e8_d845_f01f,
    0xd616_c853_1699_4f3b,
    0x4a3a_e8c1_d734_a6a7,
    0x0e9c_7721_7b6a_a8ae,
];
