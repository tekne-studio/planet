//! Flora: the same plants and beds of flowers for every machine, only on
//! ground that grows them, and never two plants in one block, across a seam
//! as anywhere.

use std::collections::BTreeSet;

use gaia::flora::{Beds, Flora, Plant, Seedling, Tissue};
use topology::{Column, Dir, Sector, SurfacePoint};
use worldgen::{Generator, Material, Recipe, Stamp};

/// A world's ground, and what grows on it.
struct World {
    generator: Generator,
    flora: Flora,
}

impl World {
    fn new(seed: u64) -> World {
        let generator = Generator::new(Recipe::new(seed)).unwrap();
        let flora = Flora::grow(&generator);
        World { generator, flora }
    }

    fn plants(&self, sector: Sector, corner: [u32; 2], side: u32) -> Vec<Plant> {
        self.flora.plants(&self.generator, sector, corner, side)
    }

    fn seedlings(&self, sector: Sector, corner: [u32; 2], side: u32) -> Vec<Seedling> {
        self.flora.seedlings(&self.generator, sector, corner, side)
    }

    fn beds(&self, sector: Sector, corner: [u32; 2], side: u32) -> Beds {
        self.flora.beds(&self.generator, sector, corner, side)
    }

    fn side(&self) -> u32 {
        self.generator.sphere().blocks().side()
    }
}

/// The first square of a coarse lattice over a sector with at least `least`
/// plants in it.
fn grown(world: &World, sector: Sector, side: u32, least: usize) -> [u32; 2] {
    let stride = world.side() / 32;
    (1..31)
        .flat_map(|j| (1..31).map(move |i| [i * stride, j * stride]))
        .find(|&corner| world.plants(sector, corner, side).len() >= least)
        .expect("a sector with a wood somewhere")
}

fn sorted(mut plants: Vec<Plant>) -> Vec<Plant> {
    plants.sort_by_key(|p| (p.root.column, p.root.h, p.shape, p.turn));
    plants
}

#[test]
fn the_same_square_grows_the_same_plants() {
    let first = World::new(1);
    let corner = grown(&first, Sector::ALL[0], 64, 6);
    let plants = first.plants(Sector::ALL[0], corner, 64);
    assert_eq!(first.plants(Sector::ALL[0], corner, 64), plants);
    assert_eq!(World::new(1).plants(Sector::ALL[0], corner, 64), plants);
    assert_ne!(World::new(2).plants(Sector::ALL[0], corner, 64), plants);
}

#[test]
fn squares_agree_where_they_overlap() {
    let world = World::new(1);
    let sector = Sector::ALL[0];
    let [u, v] = grown(&world, sector, 128, 20);
    let whole = sorted(world.plants(sector, [u, v], 128));
    let quarters = sorted(
        [[0, 0], [64, 0], [0, 64], [64, 64]]
            .into_iter()
            .flat_map(|[du, dv]| world.plants(sector, [u + du, v + dv], 64))
            .collect(),
    );
    assert_eq!(quarters, whole);
    // A square that cuts across the spots, not along them.
    let shifted = sorted(world.plants(sector, [u + 17, v + 41], 64));
    let within: Vec<Plant> = whole
        .into_iter()
        .filter(|p| {
            let (pu, pv) = (u32::from(p.root.column.u), u32::from(p.root.column.v));
            (u + 17..u + 81).contains(&pu) && (v + 41..v + 105).contains(&pv)
        })
        .collect();
    assert_eq!(shifted, within);
}

#[test]
fn nothing_grows_but_on_grass_and_forest() {
    let world = World::new(1);
    let blocks = world.generator.sphere().blocks();
    let stride = blocks.side() / 12;
    let mut grown = 0;
    for sector in Sector::ALL {
        for j in 0..12 {
            for i in 0..12 {
                for plant in world.plants(sector, [i * stride, j * stride], 64) {
                    let direction = blocks.column_direction(plant.root.column);
                    let ground = world.generator.sample(direction);
                    assert!(
                        matches!(ground.material, Material::Grass | Material::Forest),
                        "{plant:?} on {:?}",
                        ground.material
                    );
                    grown += 1;
                }
            }
        }
    }
    assert!(grown > 100, "only {grown} plants grew");
}

/// Nothing roots on ground painted as beach: at the coast every plant
/// stands over the highest sand the ground is painted with, and woods
/// still come down to near it.
#[test]
fn nothing_grows_on_the_beach() {
    let world = World::new(1);
    let blocks = world.generator.sphere().blocks();
    let stride = blocks.side() / 96;
    let mut coastal = 0;
    for sector in Sector::ALL {
        for j in 0..96 {
            for i in 0..96 {
                let corner = [i * stride, j * stride];
                let middle = SurfacePoint::new(
                    sector,
                    f64::from(corner[0]) + 32.0,
                    f64::from(corner[1]) + 32.0,
                );
                let height_m = world.generator.sample(blocks.direction(middle)).height_m;
                if !(-4.0..16.0).contains(&height_m) {
                    continue;
                }
                for plant in world.plants(sector, corner, 64) {
                    let direction = blocks.column_direction(plant.root.column);
                    let ground_m = world.generator.sample(direction).height_m;
                    assert!(
                        ground_m >= f64::from(scene::SAND_HIGHEST_M),
                        "{plant:?} roots {ground_m} m over the sea"
                    );
                    coastal += usize::from(ground_m < 8.0);
                }
            }
        }
    }
    assert!(coastal > 10, "only {coastal} plants grew near the sea");
}

#[test]
fn a_plot_grows_nothing() {
    let mut world = World::new(1);
    let sector = Sector::ALL[0];
    let [u, v] = grown(&world, sector, 64, 6);
    let footprint = [f64::from(u) + 8.0, f64::from(u) + 56.0];
    let across = [f64::from(v) + 8.0, f64::from(v) + 56.0];
    let inside = |p: &Plant| {
        let (pu, pv) = (f64::from(p.root.column.u), f64::from(p.root.column.v));
        (footprint[0]..footprint[1]).contains(&pu) && (across[0]..across[1]).contains(&pv)
    };
    assert!(world.plants(sector, [u, v], 64).iter().any(inside));
    let sphere = world.generator.sphere();
    let middle = SurfacePoint::new(sector, f64::from(u) + 32.0, f64::from(v) + 32.0);
    let height_m = world
        .generator
        .sample(sphere.blocks().direction(middle))
        .height_m;
    let stamp = Stamp::new(sphere, sector, footprint, across, height_m, 4.0);
    world.generator.stamp(stamp);
    assert!(!world.plants(sector, [u, v], 64).iter().any(inside));
}

#[test]
fn no_two_plants_share_a_block() {
    let world = World::new(1);
    let sector = Sector::ALL[0];
    let corner = grown(&world, sector, 256, 120);
    let mut taken = BTreeSet::new();
    for plant in world.plants(sector, corner, 256) {
        let root = plant.root;
        for ([x, y, z], _) in world.flora.blocks(plant) {
            let block = (
                i32::from(root.column.u) + x,
                i32::from(root.column.v) + y,
                i32::from(root.h) + z,
            );
            assert!(taken.insert(block), "two plants in {block:?}");
        }
    }
}

/// Along every edge of the first sector, the plants on its two sides keep
/// as far apart as plants anywhere do.
#[test]
fn plants_keep_apart_across_a_seam() {
    let world = World::new(1);
    let blocks = world.generator.sphere().blocks();
    let side = blocks.side();
    let reach = |p: &Plant| u64::from(world.flora.shapes()[usize::from(p.shape)].reach());
    let mut pairs = 0;
    for k in (64..side - 64).step_by(side as usize / 24) {
        for (corner, edge, dir) in [
            (
                [side - 32, k],
                Column::new(Sector::ALL[0], (side - 1) as u16, k as u16),
                Dir::UPos,
            ),
            ([0, k], Column::new(Sector::ALL[0], 0, k as u16), Dir::UNeg),
            (
                [k, side - 32],
                Column::new(Sector::ALL[0], k as u16, (side - 1) as u16),
                Dir::VPos,
            ),
            ([k, 0], Column::new(Sector::ALL[0], k as u16, 0), Dir::VNeg),
        ] {
            let here = world.plants(Sector::ALL[0], corner, 32);
            let over = blocks.step(edge, dir).column;
            let low = |x: u16| u32::from(x).saturating_sub(32).min(side - 64);
            let there = world.plants(over.sector, [low(over.u), low(over.v)], 64);
            for a in &here {
                for b in &there {
                    let apart = blocks
                        .distance_squared(a.root.column, b.root.column)
                        .unwrap();
                    let reach = reach(a) + reach(b);
                    assert!(apart > reach * reach, "{a:?} and {b:?} meet over the seam");
                    pairs += 1;
                }
            }
        }
    }
    assert!(
        pairs > 100,
        "only {pairs} pairs faced each other over a seam"
    );
}

/// The first square of a coarse lattice over a sector with at least `least`
/// beds of flowers reaching into it.
fn flowered(world: &World, sector: Sector, side: u32, least: usize) -> [u32; 2] {
    let stride = world.side() / 32;
    (1..31)
        .flat_map(|j| (1..31).map(move |i| [i * stride, j * stride]))
        .find(|&corner| world.beds(sector, corner, side).iter().count() >= least)
        .expect("a sector with a meadow somewhere")
}

#[test]
fn the_same_square_sows_the_same_beds_and_squares_agree() {
    let first = World::new(1);
    let sector = Sector::ALL[0];
    let [u, v] = flowered(&first, sector, 128, 6);
    let beds = first.beds(sector, [u, v], 128);
    assert_eq!(World::new(1).beds(sector, [u, v], 128), beds);
    assert_ne!(World::new(2).beds(sector, [u, v], 128), beds);
    // Every bed that reaches into a quarter is one of the whole's.
    for [du, dv] in [[0, 0], [64, 0], [0, 64], [64, 64]] {
        let quarter = first.beds(sector, [u + du, v + dv], 64);
        for bed in quarter.iter() {
            assert!(beds.iter().any(|whole| whole == bed), "{bed:?}");
        }
    }
}

/// A point asks the beds about it, and none that blooms there is missed:
/// the same beds, in the same order, as asking them all.
#[test]
fn a_point_finds_every_bed_that_blooms_there_among_those_near() {
    let world = World::new(1);
    let sector = Sector::ALL[0];
    let corner = flowered(&world, sector, 128, 6);
    let beds = world.beds(sector, corner, 128);
    let mut blooming = 0;
    for j in 0..128 {
        for i in 0..128 {
            let at = [corner[0] + i, corner[1] + j].map(|n| f64::from(n) + 0.5);
            let all: Vec<_> = beds.iter().filter(|bed| bed.bloom(at) > 0.0).collect();
            let near: Vec<_> = beds.near(at).filter(|bed| bed.bloom(at) > 0.0).collect();
            assert_eq!(near, all, "at {at:?}");
            blooming += usize::from(!all.is_empty());
        }
    }
    assert!(blooming > 100, "{blooming} blooming blocks");
}

#[test]
fn plants_are_the_seedlings_their_ground_takes() {
    let world = World::new(1);
    let sector = Sector::ALL[0];
    let corner = grown(&world, sector, 64, 6);
    let plants = world.plants(sector, corner, 64);
    let seedlings = world.seedlings(sector, corner, 64);
    assert!(seedlings.len() >= plants.len());
    for plant in &plants {
        assert!(
            seedlings.iter().any(|s| s.root == plant.root.column
                && s.shape == plant.shape
                && s.turn == plant.turn),
            "{plant:?}"
        );
    }
}

/// A trunk is the wood the ground runs through, all of each run of it: every
/// tree has one rising from its root block, and nothing else is a trunk.
#[test]
fn a_trunk_is_the_wood_the_ground_runs_through() {
    let world = World::new(1);
    let shapes = world.flora.shapes();
    let wooden = |tissue: Option<Tissue>| matches!(tissue, Some(Tissue::Wood | Tissue::Bark));
    let trees = shapes.iter().filter(|shape| shape.size()[2] > 8).count();
    assert!(trees >= shapes.len() / 2, "{trees} trees");
    for shape in shapes {
        for &([x, y], ref layers) in shape.trunk() {
            assert!(layers.contains(&0), "{layers:?}");
            assert!(layers.clone().all(|z| wooden(shape.at([x, y, z]))));
            assert!(!wooden(shape.at([x, y, layers.start - 1])));
            assert!(!wooden(shape.at([x, y, layers.end])));
        }
        let rooted = shape.trunk().iter().find(|(column, _)| *column == [0, 0]);
        let tall = shape.size()[2] > 8;
        assert!(
            !tall || rooted.is_some_and(|(_, layers)| layers.end > 4),
            "{:?}",
            shape.size()
        );
    }
}
