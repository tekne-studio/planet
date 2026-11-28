//! The trunks of the plants near the body, as its collision meets them.
//!
//! A plant stops a body by its trunk alone: its leaves and branches let it
//! through. A trunk is met from its side as a column of cells is, a wall
//! to a body lower than a step under its top, and a step up onto it to one
//! within one. Its top is no perch: a body on it, coming down on it or
//! found in its wood goes out of it by the side, down to the ground beside
//! it ([`crate::shed`]). It stops a body wherever its
//! plant is drawn, in a volume or out: both ask [`Rooting::grows`].

use std::collections::HashMap;
use std::sync::LazyLock;

use gaia::flora::Flora;
use protocol::Body;
use topology::{BLOCK_M, Column, Grid, Sector, SurfacePoint};
use worldgen::Generator;

use crate::cells::{Cells, Seat};
use crate::collision::{self, Footing, STEP_M};
use crate::plants::Rooting;

/// Blocks a side of the squares trunks are kept by, as a power of two: a
/// finest patch's.
const SQUARE_BITS: u32 = 5;

/// The trunks of the plants rooted in the squares about the one a body is
/// over, kept as it moves.
#[derive(Default)]
pub struct Trunks {
    /// Each column a trunk stands in, from its bottom to its top, metres
    /// over the datum, and where its plant stands.
    columns: HashMap<Column, ([f64; 2], Rooting)>,
    /// The squares kept, each with the columns its plants' trunks stand in.
    squares: HashMap<(Sector, [u32; 2]), Vec<Column>>,
}

impl Trunks {
    /// Keeps the trunks of the squares about the column a body is over,
    /// across a seam as anywhere, and forgets the rest. Off the planet, or
    /// with nothing growing, nothing is kept.
    pub fn around(
        &mut self,
        generator: &Generator,
        flora: Option<&Flora>,
        seat: Seat,
        point: SurfacePoint,
    ) {
        let (Seat::Sector(sector), Some(flora)) = (seat, flora) else {
            self.columns.clear();
            self.squares.clear();
            return;
        };
        let grid = generator.sphere().blocks();
        let side = f64::from(1u32 << SQUARE_BITS);
        let middle = [point.u, point.v].map(|at| ((at / side).floor() + 0.5) * side);
        let about: Vec<(Sector, [u32; 2])> = (-1..=1)
            .flat_map(|j| (-1..=1).map(move |i| [i, j]))
            .map(|[i, j]| {
                let at = [
                    middle[0] + f64::from(i) * side,
                    middle[1] + f64::from(j) * side,
                ];
                let column = column_at(grid, sector, at);
                let square = [column.u, column.v].map(|n| u32::from(n) >> SQUARE_BITS);
                (column.sector, square)
            })
            .collect();
        let Trunks { columns, squares } = self;
        squares.retain(|key, kept| {
            let keep = about.contains(key);
            if !keep {
                for column in kept.iter() {
                    columns.remove(column);
                }
            }
            keep
        });
        for key in about {
            squares
                .entry(key)
                .or_insert_with(|| grown(generator, flora, key, columns));
        }
    }

    /// What the trunks under a standing body's width are to it, at a column
    /// of a seat with its feet at `feet_m`: `None` where none stands, or
    /// where its plant does not grow for what is built.
    pub fn footing(
        &self,
        generator: &Generator,
        cells: &Cells,
        seat: Seat,
        point: SurfacePoint,
        feet_m: f64,
    ) -> Option<Footing> {
        self.footings(generator, cells, seat, point, feet_m)
            .reduce(Footing::with)
    }

    /// [`Trunks::footing`] column by column: what the trunk in each column
    /// the body's width spans is to it. A trunk over its feet's step is a
    /// roof; else its top is the floor, a wall's top when over the step.
    pub fn footings<'a>(
        &'a self,
        generator: &'a Generator,
        cells: &'a Cells,
        seat: Seat,
        point: SurfacePoint,
        feet_m: f64,
    ) -> impl Iterator<Item = Footing> + 'a {
        (self.rooted(generator, cells, seat, point, feet_m)).map(|(footing, _)| footing)
    }

    /// [`Trunks::footings`], each with the column its tree is rooted in:
    /// which tree a body meets.
    pub fn rooted<'a>(
        &'a self,
        generator: &'a Generator,
        cells: &'a Cells,
        seat: Seat,
        point: SurfacePoint,
        feet_m: f64,
    ) -> impl Iterator<Item = (Footing, Column)> + 'a {
        let sector = match seat {
            Seat::Sector(sector) => Some(sector),
            _ => None,
        };
        let grid = generator.sphere().blocks();
        let middle = |[x, y]: [i32; 2]| [x, y].map(|n| f64::from(n) + 0.5);
        sector.into_iter().flat_map(move |sector| {
            collision::columns(point)
                .map(move |at| column_at(grid, sector, middle(at)))
                .filter_map(move |column| self.met(cells, column, feet_m))
        })
    }

    /// These trunks as this machine's own body meets them: all of them
    /// while it draws the trees, else none, since a wall nobody sees is
    /// worse than a walk through a tree a peer sees.
    pub fn drawn(&self, trees: bool) -> &Trunks {
        static NONE: LazyLock<Trunks> = LazyLock::new(Trunks::default);
        if trees { self } else { &NONE }
    }

    /// The trunk in a column, a wall or a floor to a body with its feet at
    /// `feet_m`, never a roof: its top, metres over the datum, and the
    /// column its tree is rooted in. `None` with none.
    pub fn wood(&self, cells: &Cells, column: Column, feet_m: f64) -> Option<(f64, Column)> {
        let (footing, root) = self.met(cells, column, feet_m)?;
        footing.floor_m.map(|top_m| (top_m, root))
    }

    /// What the trunk in a column is to a body with its feet at `feet_m`,
    /// where one stands and its plant grows for what is built: a roof
    /// where it is over the feet's step, else its top a floor; and the
    /// column its tree is rooted in.
    fn met(&self, cells: &Cells, column: Column, feet_m: f64) -> Option<(Footing, Column)> {
        let &([bottom_m, top_m], stand) = self.columns.get(&column)?;
        let grows = stand.grows(&|sector, column, span_m| cells.covered(sector, column, span_m));
        let footing = match feet_m + STEP_M >= bottom_m {
            true => Footing {
                floor_m: Some(top_m),
                ceiling_m: None,
            },
            false => Footing {
                floor_m: None,
                ceiling_m: Some(bottom_m),
            },
        };
        grows.then_some((footing, stand.root()))
    }

    /// The top of the trunk in a column, metres over the datum, if one stands there.
    #[cfg(test)]
    pub(crate) fn top_of(&self, column: Column) -> Option<f64> {
        self.columns.get(&column).map(|(span, _)| span[1])
    }

    /// Whether a trunk is kept on a body: only ever the planet.
    pub fn stands_on(&self, body: Body) -> bool {
        body == Body::Planet && !self.columns.is_empty()
    }
}

/// The column of a grid at a point of a sector, in blocks, which may lie
/// past the sector's edge: it is the neighbour's there.
fn column_at(grid: Grid, sector: Sector, [u, v]: [f64; 2]) -> Column {
    grid.column_of(grid.wrapped(SurfacePoint::new(sector, u, v)))
}

/// The trunks of the plants rooted in a square, each column kept in
/// `columns`; the columns, for the square to forget them by.
fn grown(
    generator: &Generator,
    flora: &Flora,
    (sector, square): (Sector, [u32; 2]),
    columns: &mut HashMap<Column, ([f64; 2], Rooting)>,
) -> Vec<Column> {
    let grid = generator.sphere().blocks();
    let corner = square.map(|n| n << SQUARE_BITS);
    let plants = flora.plants(generator, sector, corner, 1 << SQUARE_BITS);
    let mut kept = Vec::new();
    for plant in plants {
        let root = plant.root;
        let stand = Rooting::of(flora, plant);
        let metres = |layer: i32| f64::from(i32::from(root.h) + layer) * BLOCK_M;
        for ([du, dv], layers) in flora.trunk(plant) {
            let at = [(root.column.u, du), (root.column.v, dv)]
                .map(|(n, d)| f64::from(i32::from(n) + d) + 0.5);
            let column = column_at(grid, sector, at);
            columns.insert(column, ([metres(layers.start), metres(layers.end)], stand));
            kept.push(column);
        }
    }
    kept
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::controller::{Built, Controller, Wish};
    use crate::hover::BOARD;
    use crate::parts::Parts;
    use crate::way::{Profile, Steering};
    use gaia::flora::Plant;
    use worldgen::Recipe;

    /// How a body goes.
    #[derive(Clone, Copy)]
    enum Going {
        Walking,
        Flying(f64),
        Riding,
    }

    /// Where a body set off from `from` toward `+u` ends after three
    /// seconds, along `u`.
    fn go(generator: &Generator, from: SurfacePoint, trunks: &Trunks, going: Going) -> f64 {
        let grid = generator.sphere().blocks();
        let mut controller = Controller::spawn(from, generator);
        let height_m = match going {
            Going::Flying(height_m) => Some(height_m),
            Going::Walking | Going::Riding => None,
        };
        controller.stand_at(height_m, generator);
        controller.set_pitch(0.0);
        let along = grid.tangents(from).du;
        controller.face(topology::bearing_deg(grid.direction(from), along).unwrap());
        if let Going::Riding = going {
            let board = Profile::Hover(BOARD);
            assert!(controller.take_way("test.board", &board, Steering::Steered, 0, generator));
        }
        let wish = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks,
        };
        for _ in 0..180 {
            controller.update(1.0 / 60.0, wish, generator, built);
        }
        controller.point().u
    }

    /// The trees in the middle of sector 4 whose trunk rises over a body's
    /// head, 2.5 m over the ground about their root: each plant, with the
    /// trunks about it.
    fn tall_trees<'a>(
        generator: &'a Generator,
        flora: &'a Flora,
    ) -> impl Iterator<Item = (Plant, Trunks)> + 'a {
        let side = generator.sphere().blocks().side();
        let sector = Sector::ALL[4];
        let corners =
            (8..24).flat_map(move |j| (8..24).map(move |i| [i, j].map(|n| n * side / 32)));
        let plants = corners.flat_map(move |corner| flora.plants(generator, sector, corner, 32));
        plants.filter_map(move |plant| {
            let root = plant.root.column;
            let ground_m = f64::from(plant.root.h + 1) * BLOCK_M;
            let mut trunks = Trunks::default();
            let point = SurfacePoint::center_of(root);
            trunks.around(generator, Some(flora), Seat::Sector(sector), point);
            let tall = trunks.columns.get(&root)?.0[1] > ground_m + 2.5;
            tall.then_some((plant, trunks))
        })
    }

    /// A tree whose trunk rises over a body's head, with open ground a
    /// walker crosses along `u` toward it and past it: its plant, the
    /// trunks about it, and the first column a trunk stands in on the way.
    pub(crate) fn a_tree(generator: &Generator) -> (Plant, Trunks, u16) {
        let flora = Flora::grow(generator);
        let none = Trunks::default();
        tall_trees(generator, &flora)
            .find_map(|(plant, trunks)| {
                let root = plant.root.column;
                let sector = root.sector;
                let first = (root.u - 10..=root.u).find(|&u| {
                    (root.v - 1..=root.v + 1)
                        .any(|v| trunks.columns.contains_key(&Column::new(sector, u, v)))
                })?;
                let from =
                    SurfacePoint::new(sector, f64::from(root.u) - 7.5, f64::from(root.v) + 0.5);
                let open = go(generator, from, &none, Going::Walking) > f64::from(root.u) + 2.0;
                (first + 1 >= root.u && open).then_some((plant, trunks, first))
            })
            .expect("a tree on open ground in the middle of sector 4")
    }

    #[test]
    fn a_trunk_stops_a_walker_a_rider_and_a_low_flyer_and_one_over_it_flies_on() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let (plant, trunks, first) = a_tree(&generator);
        let root = plant.root.column;
        let from = SurfacePoint::new(
            root.sector,
            f64::from(root.u) - 7.5,
            f64::from(root.v) + 0.5,
        );
        let ground_m = f64::from(plant.root.h + 1) * BLOCK_M;
        let top_m = trunks.columns[&root].0[1];
        let short = |u: f64| u > from.u + 3.0 && u < f64::from(first);
        let walked = go(&generator, from, &trunks, Going::Walking);
        assert!(
            short(walked),
            "a walker ends at {walked}, the trunk at {first}"
        );
        let ridden = go(&generator, from, &trunks, Going::Riding);
        assert!(
            short(ridden),
            "a rider ends at {ridden}, the trunk at {first}"
        );
        let free = go(&generator, from, &Trunks::default(), Going::Riding);
        assert!(
            free > f64::from(root.u) + 2.0,
            "a rider with no trunk ends at {free}"
        );
        let low = go(&generator, from, &trunks, Going::Flying(ground_m + 1.0));
        assert!(
            short(low),
            "a low flyer ends at {low}, the trunk at {first}"
        );
        let high = go(
            &generator,
            from,
            &trunks,
            Going::Flying(top_m.max(ground_m + 6.0) + 2.0),
        );
        assert!(
            high > f64::from(root.u) + 2.0,
            "a high flyer ends at {high}"
        );
    }

    /// Feet over the datum, metres.
    fn feet_m(controller: &Controller, generator: &Generator) -> f64 {
        controller.position().length() - generator.sphere().radius_m()
    }

    /// The ground under a point, metres over the datum.
    fn ground_m(generator: &Generator, point: SurfacePoint) -> f64 {
        generator
            .sample(generator.sphere().blocks().direction(point))
            .height_m
    }

    /// A walker at `from` facing along `toward`, blocks along `u` and `v`.
    fn walker(generator: &Generator, from: SurfacePoint, toward: [f64; 2]) -> Controller {
        let mut controller = Controller::spawn(from, generator);
        controller.set_pitch(0.0);
        face(&mut controller, generator, toward);
        controller
    }

    /// Turns a body to face along `toward`, blocks along `u` and `v`.
    fn face(controller: &mut Controller, generator: &Generator, [du, dv]: [f64; 2]) {
        let (grid, point) = (generator.sphere().blocks(), controller.point());
        let along = grid.tangents(point);
        let heading = glam::DVec3::from(along.du) * du + glam::DVec3::from(along.dv) * dv;
        let bearing = topology::bearing_deg(grid.direction(point), heading.to_array());
        controller.face(bearing.unwrap());
    }

    /// Turns a body away from a root's column and walks it on a second:
    /// how far it went, metres.
    fn walks_off(
        controller: &mut Controller,
        generator: &Generator,
        root: Column,
        built: Built<'_>,
    ) -> f64 {
        let point = controller.point();
        let away = [(point.u, root.u), (point.v, root.v)].map(|(at, n)| at - f64::from(n) - 0.5);
        face(controller, generator, away);
        let from = controller.position();
        let ahead = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        for _ in 0..60 {
            controller.update(1.0 / 60.0, ahead, generator, built);
        }
        (controller.position() - from).length()
    }

    /// A walker let fall onto a tree, over its trunk's middle and its
    /// edges, from just over the top and from high, still or walking on:
    /// it is never in the wood, and it ends on the ground, never on the
    /// top. Still, it comes straight down to rest beside the trunk within
    /// two seconds, holds there, and walks off at once.
    #[test]
    fn a_walker_let_fall_on_a_tree_comes_down_beside_its_trunk() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let (plant, trunks, _) = a_tree(&generator);
        let root = plant.root.column;
        let top_m = trunks.columns[&root].0[1];
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        for [du, dv] in [[0.5, 0.5], [0.1, 0.9], [1.4, 0.5], [-0.5, 1.2]] {
            for over_m in [0.2, 3.0, 12.0] {
                for walking in [false, true] {
                    let case = format!("at {du} {dv}, {over_m} m over the top, walking {walking}");
                    let at = SurfacePoint::new(
                        root.sector,
                        f64::from(root.u) + du,
                        f64::from(root.v) + dv,
                    );
                    let mut controller = walker(&generator, at, [1.0, 0.0]);
                    controller.stand_at(Some(top_m + over_m), &generator);
                    controller.walk();
                    let wish = Wish {
                        movement: [0.0, f32::from(u8::from(walking))],
                        ..Wish::default()
                    };
                    let mut was_m = feet_m(&controller, &generator);
                    for frame in 0..180 {
                        controller.update(1.0 / 60.0, wish, &generator, built);
                        let feet = feet_m(&controller, &generator);
                        let ground = ground_m(&generator, controller.point());
                        let in_wood = controller.in_wood(&generator, built);
                        assert!(!in_wood, "{case}, frame {frame}: in the wood at {feet}");
                        if !walking {
                            assert!(feet <= was_m, "{case}, frame {frame}: up to {feet}");
                        }
                        if frame >= 120 && walking {
                            assert!(feet < top_m - 1.0, "{case}, frame {frame}: on the top");
                        } else if frame >= 120 {
                            let on = controller.grounded() && (feet - ground).abs() < 1e-6;
                            assert!(on, "{case}, frame {frame}: at {feet}, ground at {ground}");
                        }
                        was_m = feet;
                    }
                    if !walking {
                        let rest = (controller.position(), controller.leaving());
                        controller.update(1.0 / 60.0, wish, &generator, built);
                        assert_eq!(rest, (controller.position(), false), "{case}: rests");
                        let off_m = walks_off(&mut controller, &generator, root, built);
                        assert!(off_m > 2.0, "{case}: walked {off_m} m off");
                    }
                }
            }
        }
    }

    /// A body standing in a trunk's column where its machine drew no
    /// trees, when they come back, still or with a key held: it goes out
    /// by the side at a walk's stride on the ground, never lifted to the
    /// top, and is out within a second; once out, it rests and walks off.
    #[test]
    fn a_body_in_a_trunk_as_its_trees_come_back_goes_out_beside_it() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let (plant, trunks, _) = a_tree(&generator);
        let root = plant.root.column;
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        let blind = Built {
            trunks: trunks.drawn(false),
            ..built
        };
        for [du, dv] in [[0.5, 0.5], [0.2, 0.8], [0.9, 0.1]] {
            for movement in [[0.0, 0.0], [0.0, 1.0], [0.0, -1.0], [1.0, 0.0]] {
                let case = format!("at {du} {dv}, keys {movement:?}");
                let at =
                    SurfacePoint::new(root.sector, f64::from(root.u) + du, f64::from(root.v) + dv);
                let mut controller = walker(&generator, at, [1.0, 0.0]);
                controller.update(1.0 / 60.0, Wish::default(), &generator, blind);
                assert!(controller.in_wood(&generator, built), "{case}: in the wood");
                let wish = Wish {
                    movement,
                    ..Wish::default()
                };
                let mut out = None;
                for frame in 0..120 {
                    let was = controller.position();
                    controller.update(1.0 / 60.0, wish, &generator, built);
                    let feet = feet_m(&controller, &generator);
                    let ground = ground_m(&generator, controller.point());
                    assert!(
                        feet < ground + STEP_M,
                        "{case}, frame {frame}: lifted to {feet}"
                    );
                    let stride_m = (controller.position() - was).length();
                    assert!(
                        stride_m < 0.15,
                        "{case}, frame {frame}: a stride of {stride_m} m"
                    );
                    match controller.in_wood(&generator, built) {
                        true => assert_eq!(out, None, "{case}, frame {frame}: in again"),
                        false => out = out.or(Some(frame)),
                    }
                }
                assert!(
                    out.is_some_and(|frame| frame < 60),
                    "{case}: out at {out:?}"
                );
                if movement == [0.0, 0.0] {
                    assert!(!controller.leaving(), "{case}: still leaving");
                    let off_m = walks_off(&mut controller, &generator, root, built);
                    assert!(off_m > 2.0, "{case}: walked {off_m} m off");
                }
            }
        }
    }

    /// The trunks of a stand: the one in `root`'s column as `tree` has it
    /// and, with a `gap`, a tree every `gap` columns about it out to five
    /// each way, each its own plant, four to nine metres tall.
    fn stand(generator: &Generator, tree: &Trunks, root: Column, gap: Option<i32>) -> Trunks {
        let grid = generator.sphere().blocks();
        let mut trunks = Trunks::default();
        trunks.columns.insert(root, tree.columns[&root]);
        let about = gap.into_iter().flat_map(|gap| {
            (-5..=5).flat_map(move |j| (-5..=5).map(move |i| [i, j].map(|n| n * gap)))
        });
        for [i, j] in about.filter(|&at| at != [0, 0]) {
            let middle = [(root.u, i), (root.v, j)].map(|(n, d)| f64::from(i32::from(n) + d) + 0.5);
            let column = column_at(grid, root.sector, middle);
            let ground = ground_m(generator, SurfacePoint::center_of(column));
            let tall_m = 4.0 + f64::from((i * 7 + j * 13).rem_euclid(9)) * 0.6;
            let span = [ground - 1.5, ground + tall_m];
            let plant = Rooting::standing(column, [span[0], span[1] + 3.0]);
            trunks.columns.insert(column, (span, plant));
        }
        trunks
    }

    /// Cells laid in boxes, each from one corner to the other, blocks along
    /// `u` and `v` from `column`'s and up from the datum.
    fn laid(generator: &Generator, column: Column, boxes: &[[[i32; 3]; 2]]) -> Cells {
        let mut cells = Cells::default();
        let (seat, middle) = (Seat::Sector(column.sector), SurfacePoint::center_of(column));
        cells.open(generator, seat, middle).expect("a volume");
        let corner = [i32::from(column.u), i32::from(column.v), 0];
        let at = |offset: [i32; 3]| std::array::from_fn(|axis| corner[axis] + offset[axis]);
        let gestures: Vec<voxel::Gesture> = (boxes.iter())
            .map(|&[low, high]| voxel::Gesture::Create {
                span: voxel::Span::between(at(low), at(high)),
                paint: 0,
            })
            .collect();
        assert!(cells.apply(generator, column.sector, &gestures));
        cells
    }

    /// A body of `who` at `at` facing along `toward`, blocks along `u` and
    /// `v`, its feet `feet_m` over the datum: a walker, a horse, a pegasus
    /// coming down to land, or a board.
    fn body(
        generator: &Generator,
        built: Built<'_>,
        who: &str,
        (at, toward): (SurfacePoint, [f64; 2]),
        feet_m: f64,
    ) -> Controller {
        let mut body = walker(generator, at, toward);
        let paces = |profile: fn() -> crate::paces::Paces| Profile::Paces(Box::new(profile()));
        match who {
            "walker" => {}
            "horse" => {
                let horse = paces(crate::paces::tests::horse);
                assert!(body.take_way("test.horse", &horse, Steering::Steered, 1, generator));
            }
            "pegasus" => {
                let pegasus = paces(crate::paces::tests::pegasus);
                assert!(body.take_way("test.horse", &pegasus, Steering::Steered, 1, generator));
            }
            _ => {
                let board = Profile::Hover(BOARD);
                assert!(body.take_way("test.board", &board, Steering::Steered, 0, generator));
            }
        }
        body.stand_at(Some(feet_m), generator);
        if who == "walker" {
            body.walk();
        }
        if who == "pegasus" {
            assert_eq!(body.take_off(true, generator, built), Ok(()));
            assert_eq!(body.land(generator, built), Ok(()));
        }
        body
    }

    /// Bodies on a floor higher than a trunk's top, with the trunk under
    /// their width: a deck laid over a tree, a wall's top beside a trunk
    /// and a slope over a lower one, standing, jumping and coming down on
    /// it, facing away from the trunk where a jump would carry them off
    /// the floor over it. Each goes frame for frame as it goes with no
    /// tree, and never leaves the wood: a top under a higher floor is no
    /// floor of theirs.
    #[test]
    fn a_body_on_a_floor_over_a_trunk_goes_as_with_no_tree() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        /// A floor over the wood, how high it is over the datum, and where
        /// bodies are set on it, each facing a way.
        struct Floor {
            name: String,
            trunks: Trunks,
            cells: Cells,
            floor_m: f64,
            points: Vec<(SurfacePoint, [f64; 2])>,
        }
        let g = &generator;
        let (plant, tree, _) = a_tree(g);
        let root = plant.root.column;
        let top_m = tree.columns[&root].0[1];
        let ground = ground_m(g, SurfacePoint::center_of(root));
        let (none, parts) = (Trunks::default(), Parts::default());
        let at = |column: Column, [du, dv]: [f64; 2]| {
            SurfacePoint::new(
                column.sector,
                f64::from(column.u) + du,
                f64::from(column.v) + dv,
            )
        };
        let layer = |metres: f64| (metres / BLOCK_M).floor() as i32;
        let still = Wish::default();
        let wishes = [(60, still), (1, Wish { up: true, ..still }), (179, still)];
        let mut floors = Vec::new();
        for over_m in [3.0, 6.0] {
            let z = layer(top_m + over_m) - 1;
            floors.push(Floor {
                name: format!("a deck {over_m} m over a tree"),
                trunks: stand(g, &tree, root, None),
                cells: laid(g, root, &[[[-12, -12, z], [12, 12, z]]]),
                floor_m: f64::from(z + 1) * BLOCK_M,
                points: vec![
                    (at(root, [0.5, 0.5]), [1.0, 0.0]),
                    (at(root, [1.3, 0.5]), [1.0, 0.0]),
                ],
            });
        }
        let wall = [[-8, -1, layer(ground) - 2], [-1, 1, layer(top_m + 1.0) - 1]];
        floors.push(Floor {
            name: "a wall's top beside a trunk".into(),
            trunks: stand(g, &tree, root, None),
            cells: laid(g, root, &[wall]),
            floor_m: f64::from(wall[1][2] + 1) * BLOCK_M,
            points: vec![(at(root, [-0.45, 0.5]), [-1.0, 0.0])],
        });
        // The steepest rise from a column to the next east near the tree,
        // over the sea: a stump in the low column, its top half way up to
        // the ground beside it, where a body stands with the stump under
        // its width.
        let middle_m = |column: Column| ground_m(g, SurfacePoint::center_of(column));
        let rise = |column: Column| {
            middle_m(Column::new(column.sector, column.u + 1, column.v)) - middle_m(column)
        };
        let low = (0..64)
            .flat_map(|dv| {
                (0..64).map(move |du| Column::new(root.sector, root.u + du, root.v + dv))
            })
            .filter(|&column| middle_m(column) > 1.0)
            .max_by(|&one, &other| rise(one).total_cmp(&rise(other)))
            .expect("a column over the sea");
        let on = at(low, [1.45, 0.5]);
        let (low_m, on_m) = (middle_m(low), ground_m(g, on));
        let span = [low_m - 1.5, (low_m + on_m) / 2.0];
        assert!(
            on_m - span[1] > 0.05,
            "the ground at {on_m}, the top at {}",
            span[1]
        );
        let mut trunks = Trunks::default();
        trunks.columns.insert(
            low,
            (span, Rooting::standing(low, [span[0], span[1] + 2.0])),
        );
        floors.push(Floor {
            name: "a slope over a lower top".into(),
            trunks,
            cells: Cells::default(),
            floor_m: on_m,
            points: vec![(on, [1.0, 0.0])],
        });
        for floor in &floors {
            let with = Built {
                cells: &floor.cells,
                parts: &parts,
                trunks: &floor.trunks,
            };
            let without = Built {
                trunks: &none,
                ..with
            };
            for &(point, toward) in &floor.points {
                for (who, over_m) in [
                    ("walker", 0.0),
                    ("horse", 0.0),
                    ("walker", 3.0),
                    ("pegasus", 3.0),
                    ("pegasus", 8.0),
                ] {
                    let case = format!("{}, a {who} {over_m} m over it at {point:?}", floor.name);
                    let mut body = body(g, with, who, (point, toward), floor.floor_m + over_m);
                    let mut alone = body.clone();
                    let mut frame = 0;
                    for (frames, wish) in wishes {
                        for _ in 0..frames {
                            body.update(1.0 / 60.0, wish, g, with);
                            alone.update(1.0 / 60.0, wish, g, without);
                            assert!(!body.leaving(), "{case}, frame {frame}: leaving");
                            assert_eq!(body.position(), alone.position(), "{case}, frame {frame}");
                            frame += 1;
                        }
                    }
                }
            }
        }
    }

    /// A board brought down on a tree, over its trunk's middle, its edge
    /// and beside it, from just over the top and from high: it is never in
    /// the wood, and glides off the top to ride over the ground beside it,
    /// as slowly as a board comes down from high.
    #[test]
    fn a_board_brought_down_on_a_tree_rides_off_it() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let g = &generator;
        let (plant, trunks, _) = a_tree(g);
        let root = plant.root.column;
        let top_m = trunks.columns[&root].0[1];
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &trunks,
        };
        for [du, dv] in [[0.5, 0.5], [0.9, 0.5], [1.4, 0.5]] {
            for over_m in [0.3, 3.0, 12.0] {
                let case = format!("at {du} {dv}, {over_m} m over the top");
                let at =
                    SurfacePoint::new(root.sector, f64::from(root.u) + du, f64::from(root.v) + dv);
                let mut board = walker(g, at, [1.0, 0.0]);
                let way = Profile::Hover(BOARD);
                assert!(board.take_way("test.board", &way, Steering::Steered, 0, g));
                board.stand_at(Some(top_m + over_m), g);
                for frame in 0..1800 {
                    board.update(1.0 / 60.0, Wish::default(), g, built);
                    let in_wood = board.in_wood(g, built);
                    assert!(!in_wood, "{case}, frame {frame}: in the wood");
                }
                let over_m = feet_m(&board, g) - ground_m(g, board.point());
                assert!(over_m < 1.0, "{case}: {over_m} m over the ground");
                assert!(!board.leaving(), "{case}: still leaving");
            }
        }
    }

    /// A walker, a horse, a pegasus and a board brought down on the middle
    /// of each tall tree in the middle of a sector, from just over its
    /// top, among the trees the flora grows about it: each ends out of its
    /// way and nearer the ground than a top could hold it, each top here
    /// 2.5 m or more over it and a board riding a metre over it. The ones
    /// on feet rest; a board glides down a slope, as it does with no tree.
    #[test]
    fn a_body_brought_down_on_any_tree_of_a_forest_ends_on_the_ground() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let g = &generator;
        let flora = Flora::grow(g);
        let (cells, parts) = (Cells::default(), Parts::default());
        let (dt, still) = (1.0 / 20.0, Wish::default());
        let mut trees = 0;
        for (plant, trunks) in tall_trees(g, &flora) {
            trees += 1;
            let root = plant.root.column;
            let top_m = trunks.columns[&root].0[1];
            let built = Built {
                cells: &cells,
                parts: &parts,
                trunks: &trunks,
            };
            for who in ["walker", "horse", "pegasus", "board"] {
                let case = format!("a {who} on the tree rooted in {root:?}");
                let at = (SurfacePoint::center_of(root), [1.0, 0.0]);
                let mut body = body(g, built, who, at, top_m + 0.3);
                for _ in 0..(20.0 / dt) as usize {
                    body.update(dt, still, g, built);
                }
                let over_m = feet_m(&body, g) - ground_m(g, body.point());
                assert!(over_m < 1.5, "{case}: {over_m} m over the ground");
                assert!(!body.leaving(), "{case}: still leaving");
                if who != "board" {
                    let rest = body.position();
                    body.update(dt, still, g, built);
                    assert_eq!(rest, body.position(), "{case}: rests");
                }
            }
        }
        assert!(trees > 200, "{trees} tall trees");
    }

    /// A walker, a horse, a pegasus and a board brought down from just
    /// over the top of a trunk walled in by what is built two columns out
    /// on every side, the walls 5 m over its top, facing every way: none
    /// has a way out, and each is held on the top as what is built holds
    /// it, a board on its cushion. A pegasus comes down with its nose and
    /// rump over the walls: its way out is walked a cell at a time, as on
    /// feet, so it never goes through a wall nor up onto its top.
    #[test]
    fn a_body_brought_down_on_a_walled_in_tree_is_held_on_its_top() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let g = &generator;
        let (plant, tree, _) = a_tree(g);
        let root = plant.root.column;
        let top_m = tree.columns[&root].0[1];
        let layer = |metres: f64| (metres / BLOCK_M).floor() as i32;
        let (low, high) = (
            layer(ground_m(g, SurfacePoint::center_of(root))) - 3,
            layer(top_m + 5.0),
        );
        let walls = [
            [[2, -2, low], [2, 2, high]],
            [[-2, -2, low], [-2, 2, high]],
            [[-2, 2, low], [2, 2, high]],
            [[-2, -2, low], [2, -2, high]],
        ];
        let (cells, parts) = (laid(g, root, &walls), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &tree,
        };
        let (dt, still) = (1.0 / 60.0, Wish::default());
        let at = SurfacePoint::center_of(root);
        for who in ["walker", "horse", "pegasus", "board"] {
            for toward in [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]] {
                let case = format!("a {who} facing {toward:?}");
                let mut body = body(g, built, who, (at, toward), top_m + 0.3);
                for frame in 0..(10.0 / dt) as usize {
                    body.update(dt, still, g, built);
                    // A board's cushion springs it a metre over the top
                    // and back, as over any floor; the walls' top is 5 m.
                    let over_m = feet_m(&body, g) - top_m;
                    assert!(
                        over_m < 2.0,
                        "{case}, frame {frame}: {over_m} m over the top"
                    );
                }
                let [du, dv] = [body.point().u - at.u, body.point().v - at.v];
                assert!(du.hypot(dv) < 1.0, "{case}: went {du} {dv} through a wall");
                assert!(!body.leaving(), "{case}: still leaving");
                // On the top, a board on its cushion.
                let on_m = match who {
                    "board" => BOARD.height_m,
                    _ => 0.0,
                };
                let over_m = feet_m(&body, g) - top_m;
                assert!(
                    (over_m - on_m).abs() < 1e-3,
                    "{case}: {over_m} m over the top"
                );
            }
        }
    }

    /// A walker, a horse and a pegasus brought down on a tree in a stand,
    /// its neighbours three and four columns apart, over its trunk's middle
    /// and about it, from over its top and from high, facing every way, at
    /// 60 and 20 frames a second, left to go as its way out takes it: on
    /// it, each is stopped by every trunk but the one it leaves, so it is
    /// never in the wood of any deeper than a step up onto its top.
    #[test]
    fn a_body_brought_down_in_a_stand_is_never_in_a_neighbour() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let g = &generator;
        let (plant, tree, _) = a_tree(g);
        let root = plant.root.column;
        let top_m = tree.columns[&root].0[1];
        let (cells, parts) = (Cells::default(), Parts::default());
        let still = Wish::default();
        let ways = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [0.7, 0.7]];
        for gap in [3, 4] {
            let trunks = stand(g, &tree, root, Some(gap));
            let built = Built {
                cells: &cells,
                parts: &parts,
                trunks: &trunks,
            };
            for who in ["walker", "horse", "pegasus"] {
                for [du, dv] in [-0.45, 0.0, 0.45]
                    .map(|du| [-0.45, 0.0, 0.45].map(|dv| [du, dv]))
                    .concat()
                {
                    let at = SurfacePoint::new(
                        root.sector,
                        f64::from(root.u) + 0.5 + du,
                        f64::from(root.v) + 0.5 + dv,
                    );
                    for over_m in [3.0, 12.0] {
                        for dt in [1.0 / 60.0, 1.0 / 20.0] {
                            for toward in ways {
                                let case = format!(
                                    "every {gap}, a {who} at {du} {dv} facing {toward:?}, {over_m} m over, {dt} s a frame"
                                );
                                let mut body = body(g, built, who, (at, toward), top_m + over_m);
                                for frame in 0..(4.0 / dt) as usize {
                                    body.update(dt, still, g, built);
                                    let sunk_m = body.sunk_m(g, built);
                                    assert!(
                                        sunk_m <= STEP_M,
                                        "{case}, frame {frame}: {sunk_m} m in the wood"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// A walker, a horse and a pegasus brought down on a tree in a ring of
    /// taller trees two columns out, each its own, closed and with a gap
    /// three wide, facing every way, at 60 and 20 frames a second: a walker
    /// from just over its top, and each from over the ring's, so a long
    /// body's spine is never in the ring as it comes. Each goes out by the
    /// gap or stays on the top, never through the ring: it is never in the
    /// wood of any deeper than a step up onto its top. By the gap, a walker
    /// ends on the ground.
    #[test]
    fn a_body_brought_down_in_a_ring_of_taller_trees_never_goes_through_it() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let g = &generator;
        let grid = g.sphere().blocks();
        let (plant, tree, _) = a_tree(g);
        let root = plant.root.column;
        let top_m = tree.columns[&root].0[1];
        let (cells, parts) = (Cells::default(), Parts::default());
        let at = SurfacePoint::center_of(root);
        let bodies = [
            ("walker", 0.3),
            ("walker", 3.0),
            ("horse", 3.0),
            ("pegasus", 3.0),
        ];
        let ways = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [0.7, 0.7]];
        for gapped in [false, true] {
            let mut trunks = Trunks::default();
            trunks.columns.insert(root, tree.columns[&root]);
            let ring = (-2..=2).flat_map(|j: i32| (-2..=2).map(move |i: i32| [i, j]));
            let gap = |[i, j]: [i32; 2]| gapped && i == -2 && j.abs() <= 1;
            for [i, j] in ring.filter(|&[i, j]| i.abs().max(j.abs()) == 2 && !gap([i, j])) {
                let middle =
                    [(root.u, i), (root.v, j)].map(|(n, d)| f64::from(i32::from(n) + d) + 0.5);
                let column = column_at(grid, root.sector, middle);
                let span = [
                    ground_m(g, SurfacePoint::center_of(column)) - 1.5,
                    top_m + 2.0,
                ];
                trunks.columns.insert(
                    column,
                    (span, Rooting::standing(column, [span[0], span[1] + 3.0])),
                );
            }
            let built = Built {
                cells: &cells,
                parts: &parts,
                trunks: &trunks,
            };
            for (who, over_m) in bodies {
                for dt in [1.0 / 60.0, 1.0 / 20.0] {
                    for toward in ways {
                        let case = format!(
                            "a {who} in a ring gapped {gapped}, facing {toward:?}, {over_m} m over, {dt} s a frame"
                        );
                        let mut body = body(g, built, who, (at, toward), top_m + over_m);
                        for frame in 0..(6.0 / dt) as usize {
                            body.update(dt, Wish::default(), g, built);
                            let sunk_m = body.sunk_m(g, built);
                            assert!(
                                sunk_m <= STEP_M,
                                "{case}, frame {frame}: {sunk_m} m in the wood"
                            );
                        }
                        if gapped && who == "walker" {
                            let over_m = feet_m(&body, g) - ground_m(g, body.point());
                            assert!(over_m < STEP_M, "{case}: {over_m} m over the ground");
                            assert!(!body.leaving(), "{case}: still leaving");
                        }
                    }
                }
            }
        }
    }

    /// A plant and its trunk answer what is built over them alike: an open
    /// volume with nothing over the root leaves both, and a cell high in the
    /// root's column, within the plant's height, takes both away.
    #[test]
    fn a_trunk_stops_a_body_where_its_plant_is_drawn() {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let flora = Flora::grow(&generator);
        let (plant, trunks, _) = a_tree(&generator);
        let root = plant.root.column;
        let (seat, at) = (Seat::Sector(root.sector), SurfacePoint::center_of(root));
        let feet_m = f64::from(plant.root.h + 1) * BLOCK_M;
        let mut cells = Cells::default();
        let stand = Rooting::of(&flora, plant);
        let drawn = |cells: &Cells| stand.grows(&|s, c, h| cells.covered(s, c, h));
        let met = |cells: &Cells| trunks.footing(&generator, cells, seat, at, feet_m);
        assert!(drawn(&cells));
        assert!(met(&cells).unwrap().floor_m.unwrap() > feet_m + STEP_M);
        cells.open(&generator, seat, at).unwrap();
        assert!(drawn(&cells), "an open volume with nothing built");
        assert!(met(&cells).is_some(), "an open volume with nothing built");
        // A cell over the trunk's top, under the crown's: a floor laid
        // through the tree.
        let top_m = trunks.columns[&root].0[1];
        let x = |n: u16| i32::from(n);
        let cell = [x(root.u), x(root.v), (top_m / BLOCK_M) as i32 + 2];
        let gesture = voxel::Gesture::Create {
            span: voxel::Span::between(cell, cell),
            paint: 1,
        };
        assert!(cells.apply(&generator, seat, &[gesture]));
        assert!(!drawn(&cells), "a floor through the tree");
        assert_eq!(met(&cells), None, "a floor through the tree");
    }

    /// Every frame of a body folded into one number, each length to the
    /// millimetre: a sine's last bit is its machine's, and the number is
    /// every machine's.
    struct Fold(u64);

    impl Fold {
        fn put(&mut self, bits: u64) {
            self.0 ^= bits;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }

        fn body(&mut self, body: &Controller) {
            let (position, facing) = (body.position(), body.facing());
            for value in [
                position.x,
                position.y,
                position.z,
                body.height_m(),
                facing.x,
                facing.y,
                facing.z,
                body.speed_mps(),
            ] {
                self.put((value * 1000.0).round() as i64 as u64);
            }
            self.put(u64::from(body.grounded()));
            self.put(u64::from(body.aloft()));
        }

        fn run(
            &mut self,
            body: &mut Controller,
            (frames, dt): (usize, f64),
            wish: Wish,
            generator: &Generator,
            built: Built<'_>,
        ) {
            for _ in 0..frames {
                body.update(dt, wish, generator, built);
                self.body(body);
            }
        }
    }

    /// Bodies meeting trees from outside: walkers head on, grazing,
    /// slanting, running at 20 frames a second and jumping; flyers level
    /// into a trunk and its crown; a board, a horse and a pegasus ridden at
    /// one, jumped at it, nosed, slid, backed and turned against it; a
    /// pegasus taking off toward one from a halt, a walk and a gallop; a
    /// rider stepping down beside one and mounting from flight; walkers
    /// and a rider under a bough over their heads; trunks across a sector's
    /// seam; wide trees met from eight ways; decks laid flush with a top
    /// and half way up a trunk, and a wall beside one. Each frame of each,
    /// as [`Fold`] folds it.
    fn met_from_outside() -> u64 {
        let generator = Generator::new(Recipe::new(1)).unwrap();
        let g = &generator;
        let grid = g.sphere().blocks();
        let (plant, tree, _) = a_tree(g);
        let root = plant.root.column;
        let like = tree.columns[&root].1;
        let (cells, parts) = (Cells::default(), Parts::default());
        let built = Built {
            cells: &cells,
            parts: &parts,
            trunks: &tree,
        };
        let at = |du: f64, dv: f64| {
            SurfacePoint::new(root.sector, f64::from(root.u) + du, f64::from(root.v) + dv)
        };
        let ground = |point: SurfacePoint| g.sample_at(grid.direction(point), 0.0).height_m;
        let spans = |spans: &[(Column, [f64; 2])]| {
            let mut trunks = Trunks::default();
            for &(column, span) in spans {
                trunks.columns.insert(column, (span, like));
            }
            trunks
        };
        let top_m = tree.columns[&root].0[1];
        let ground_m = ground(SurfacePoint::center_of(root));
        let forward = Wish {
            movement: [0.0, 1.0],
            ..Wish::default()
        };
        let (fps60, fps20) = (1.0 / 60.0, 1.0 / 20.0);
        let horse = Profile::Paces(Box::new(crate::paces::tests::horse()));
        let pegasus = Profile::Paces(Box::new(crate::paces::tests::pegasus()));
        let board = Profile::Hover(BOARD);
        let mut fold = Fold(0xcbf2_9ce4_8422_2325);

        // Walkers.
        for dv in [-2.0, -1.2, -0.6, 0.0, 0.4, 1.0, 1.6, 2.0] {
            for (toward, wish, dt) in [
                ([1.0, 0.0], forward, fps60),
                ([1.0, 0.3], forward, fps60),
                ([1.0, 1.0], forward, fps60),
                ([1.0, -1.0], forward, fps60),
                (
                    [1.0, 0.0],
                    Wish {
                        sprint: true,
                        ..forward
                    },
                    fps20,
                ),
                (
                    [1.0, 0.0],
                    Wish {
                        up: true,
                        ..forward
                    },
                    fps60,
                ),
                (
                    [1.0, 0.0],
                    Wish {
                        up: true,
                        sprint: true,
                        ..forward
                    },
                    fps20,
                ),
            ] {
                let mut body = walker(g, at(-4.5, 0.5 + dv), toward);
                fold.run(&mut body, ((4.0 / dt) as usize, dt), wish, g, built);
            }
        }

        // Flyers level.
        for over_m in [
            0.5,
            1.0,
            top_m - ground_m - 0.6,
            top_m - ground_m + 0.1,
            top_m - ground_m + 2.0,
        ] {
            for dv in [0.5, 1.2, -0.6] {
                let mut body = walker(g, at(-6.5, dv), [1.0, 0.0]);
                body.stand_at(Some(ground_m + over_m), g);
                fold.run(&mut body, (120, fps60), forward, g, built);
            }
        }

        // A horse and a pegasus ridden at it, and jumped at it.
        for paces in [&horse, &pegasus] {
            for dv in [0.5, 1.3, -0.4] {
                for wish in [
                    forward,
                    Wish {
                        sprint: true,
                        ..forward
                    },
                ] {
                    for dt in [fps60, fps20] {
                        let mut body = walker(g, at(-14.5, dv), [1.0, 0.0]);
                        body.stand_at(None, g);
                        assert!(body.take_way("test.horse", paces, Steering::Steered, 1, g));
                        let struck = Wish {
                            struck: [true, false],
                            ..wish
                        };
                        fold.run(&mut body, (1, dt), struck, g, built);
                        fold.run(&mut body, ((6.0 / dt) as usize, dt), wish, g, built);
                        let back = Wish {
                            movement: [0.6, -1.0],
                            struck: [false, true],
                            ..Wish::default()
                        };
                        fold.run(&mut body, ((3.0 / dt) as usize, dt), back, g, built);
                    }
                }
                for far in [6.5, 8.5, 10.5] {
                    let mut body = walker(g, at(-far, dv), [1.0, 0.0]);
                    body.stand_at(None, g);
                    assert!(body.take_way("test.horse", paces, Steering::Steered, 1, g));
                    let go = Wish {
                        sprint: true,
                        ..forward
                    };
                    fold.run(
                        &mut body,
                        (1, fps60),
                        Wish {
                            struck: [true, false],
                            ..go
                        },
                        g,
                        built,
                    );
                    fold.run(&mut body, (30, fps60), go, g, built);
                    fold.run(&mut body, (30, fps60), Wish { up: true, ..go }, g, built);
                    fold.run(&mut body, (120, fps60), go, g, built);
                }
            }
        }

        // A pegasus taking off toward it from a halt and a walk.
        for far in [4.5, 6.5, 8.5, 12.5] {
            for moving in [false, true] {
                let mut body = walker(g, at(-far, 0.5), [1.0, 0.0]);
                body.stand_at(None, g);
                assert!(body.take_way("test.horse", &pegasus, Steering::Steered, 1, g));
                let wish = if moving { forward } else { Wish::default() };
                if moving {
                    fold.run(
                        &mut body,
                        (1, fps60),
                        Wish {
                            struck: [true, false],
                            ..forward
                        },
                        g,
                        built,
                    );
                    fold.run(&mut body, (20, fps60), forward, g, built);
                }
                fold.put(u64::from(body.take_off(true, g, built).is_ok()));
                fold.run(&mut body, (240, fps60), wish, g, built);
            }
        }

        // A pegasus galloping and taking off with the tree ahead.
        for ahead in [2.5, 4.0, 10.0, 30.0] {
            for sprint in [false, true] {
                let mut body = walker(g, at(-60.5, 0.5), [1.0, 0.0]);
                assert!(body.take_way("test.horse", &pegasus, Steering::Steered, 1, g));
                let mut took = false;
                for frame in 0..600 {
                    let wish = Wish {
                        movement: [0.0, 1.0],
                        sprint,
                        struck: [frame < 40 && frame % 10 == 0, false],
                        ..Wish::default()
                    };
                    if !took && f64::from(root.u) - body.point().u <= ahead {
                        took = true;
                        fold.put(u64::from(body.take_off(true, g, built).is_ok()));
                    }
                    fold.run(&mut body, (1, fps60), wish, g, built);
                }
            }
        }

        // A board ridden into it.
        for dv in [0.5, 1.2, -0.4, 1.9] {
            for sprint in [false, true] {
                let mut body = walker(g, at(-8.5, dv), [1.0, 0.0]);
                assert!(body.take_way("test.board", &board, Steering::Steered, 0, g));
                let wish = Wish { sprint, ..forward };
                fold.run(
                    &mut body,
                    (1, fps60),
                    Wish {
                        struck: [true, false],
                        ..wish
                    },
                    g,
                    built,
                );
                fold.run(&mut body, (300, fps60), wish, g, built);
                let turn = Wish {
                    movement: [1.0, 0.5],
                    ..Wish::default()
                };
                fold.run(&mut body, (120, fps60), turn, g, built);
            }
        }

        // A horse against it: nosed, slid, backed and turned.
        for (start, toward, name) in [
            ([-6.5, 0.5], [1.0, 0.0], "nose"),
            ([-6.5, 0.5], [1.0, 0.15], "slide"),
            ([7.5, 0.5], [1.0, 0.0], "back"),
            ([-5.4, 0.5], [1.0, 0.0], "turn"),
            ([0.5, -3.2], [1.0, 0.0], "turn"),
        ] {
            for dt in [fps60, fps20] {
                let mut body = walker(g, at(start[0], start[1]), toward);
                assert!(body.take_way("test.horse", &horse, Steering::Steered, 1, g));
                let first = match name {
                    "back" => Wish {
                        movement: [0.0, -1.0],
                        struck: [false, true],
                        ..Wish::default()
                    },
                    "turn" => Wish {
                        movement: [1.0, 0.0],
                        ..Wish::default()
                    },
                    _ => Wish {
                        struck: [true, false],
                        ..forward
                    },
                };
                fold.run(&mut body, (1, dt), first, g, built);
                let keep = Wish {
                    struck: [false, false],
                    ..first
                };
                fold.run(&mut body, ((5.0 / dt) as usize, dt), keep, g, built);
                let right = Wish {
                    movement: [1.0, 0.3],
                    ..Wish::default()
                };
                fold.run(&mut body, ((3.0 / dt) as usize, dt), right, g, built);
                let left = Wish {
                    movement: [-1.0, -0.3],
                    ..Wish::default()
                };
                fold.run(&mut body, ((3.0 / dt) as usize, dt), left, g, built);
            }
        }

        // A rider stepping down beside it, and mounting from flight there.
        for (du, dv, toward) in [
            (-1.2, 0.5, [0.0, 1.0]),
            (-1.2, 0.5, [0.0, -1.0]),
            (0.5, -1.2, [1.0, 0.0]),
            (0.5, 2.2, [-1.0, 0.0]),
            (-1.1, 1.6, [1.0, 1.0]),
        ] {
            let mut body = walker(g, at(du, dv), toward);
            body.stand_at(None, g);
            assert!(body.take_way("test.horse", &horse, Steering::Steered, 1, g));
            fold.run(&mut body, (5, fps60), Wish::default(), g, built);
            body.step_off(g, built);
            fold.body(&body);
            fold.run(&mut body, (60, fps60), Wish::default(), g, built);
            let point = at(du, dv);
            let animal = crate::animals::Near {
                number: 1,
                kind: &gaia::fauna::horse::HORSE,
                at: glam::DVec3::from(grid.direction(point))
                    * (g.sphere().radius_m() + ground(point)),
                heading: glam::DVec3::from(grid.tangents(point).du).normalize(),
                ridden: false,
                keeping: crate::animals::Keeping::Wild,
                restless: 0.5,
                speed_share: 1.0,
                size: 1.0,
            };
            for over_m in [top_m + 2.0, top_m - 1.0, ground(point) + 2.0] {
                let mut flyer = walker(g, at(du - 3.0, dv), toward);
                flyer.stand_at(Some(over_m), g);
                let mounted =
                    flyer.mount("test.horse", &horse, Steering::Steered, &animal, g, built);
                fold.put(u64::from(mounted));
                fold.body(&flyer);
            }
        }

        // Walkers and a rider under a bough over their heads.
        for bottom_m in [2.0, 2.4, 3.0] {
            let next = Column::new(root.sector, root.u + 1, root.v);
            let span = [ground_m + bottom_m, ground_m + 5.0];
            let bough = spans(&[(root, span), (next, span)]);
            let built = Built {
                trunks: &bough,
                ..built
            };
            for wish in [
                forward,
                Wish {
                    up: true,
                    ..forward
                },
                Wish {
                    sprint: true,
                    ..forward
                },
            ] {
                let mut body = walker(g, at(-4.5, 0.5), [1.0, 0.0]);
                fold.run(&mut body, (240, fps60), wish, g, built);
            }
            let mut body = walker(g, at(-6.5, 0.5), [1.0, 0.0]);
            assert!(body.take_way("test.horse", &horse, Steering::Steered, 1, g));
            fold.run(
                &mut body,
                (1, fps60),
                Wish {
                    struck: [true, false],
                    ..forward
                },
                g,
                built,
            );
            fold.run(&mut body, (240, fps60), forward, g, built);
        }

        // Trunks across a sector's seam.
        let side = f64::from(grid.side());
        for share in [0.3, 0.7] {
            let from = SurfacePoint::new(root.sector, side - 3.5, side * share);
            let past: Vec<(Column, [f64; 2])> = (-1..=1)
                .map(|dv| {
                    let point = SurfacePoint::new(root.sector, side + 1.5, from.v + f64::from(dv));
                    let column = grid.column_of(grid.wrapped(point));
                    let ground_m = ground(SurfacePoint::center_of(column));
                    (column, [ground_m - 0.5, ground_m + 6.0])
                })
                .collect();
            let seam = spans(&past);
            let built = Built {
                trunks: &seam,
                ..built
            };
            for (toward, dv) in [
                ([1.0, 0.0], 0.0),
                ([1.0, 0.0], 1.5),
                ([1.0, 0.0], -1.6),
                ([1.0, 0.5], -2.0),
            ] {
                let start = SurfacePoint::new(root.sector, from.u, from.v + dv);
                let mut body = walker(g, start, toward);
                fold.run(&mut body, (240, fps60), forward, g, built);
                let mut body = walker(g, start, toward);
                body.stand_at(Some(ground(from) + 1.5), g);
                fold.run(&mut body, (120, fps60), forward, g, built);
            }
        }

        // Wide trees met from eight ways, alone: walking, ridden, flown low
        // and jumped.
        let flora = Flora::grow(g);
        for columns in [5, 29] {
            let sector = Sector::ALL[4];
            let side = grid.side();
            let corners = (4..28).flat_map(|j| (4..28).map(move |i| [i, j].map(|n| n * side / 32)));
            let wide = (corners.flat_map(|corner| flora.plants(g, sector, corner, 32)))
                .find(|&plant| flora.trunk(plant).count() >= columns)
                .expect("a wide tree");
            let middle = SurfacePoint::center_of(wide.root.column);
            let metres = |layer: i32| f64::from(i32::from(wide.root.h) + layer) * BLOCK_M;
            let mut trunks = Trunks::default();
            for ([du, dv], layers) in flora.trunk(wide) {
                let column = Column::new(
                    sector,
                    (i32::from(wide.root.column.u) + du) as u16,
                    (i32::from(wide.root.column.v) + dv) as u16,
                );
                let span = [metres(layers.start), metres(layers.end)];
                trunks
                    .columns
                    .insert(column, (span, Rooting::of(&flora, wide)));
            }
            let built = Built {
                trunks: &trunks,
                ..built
            };
            for way in (0..16).step_by(2) {
                let angle = std::f64::consts::TAU * f64::from(way) / 16.0;
                let toward = [angle.cos(), angle.sin()];
                let from = SurfacePoint::new(
                    sector,
                    middle.u - toward[0] * 12.0,
                    middle.v - toward[1] * 12.0,
                );
                for mode in 0..4 {
                    let mut body = walker(g, from, toward);
                    let mut wish = forward;
                    match mode {
                        0 => {}
                        1 => {
                            assert!(body.take_way("test.horse", &horse, Steering::Steered, 1, g));
                            let struck = Wish {
                                struck: [true, false],
                                ..forward
                            };
                            fold.run(&mut body, (1, fps60), struck, g, built);
                        }
                        2 => body.stand_at(Some(ground(from) + 1.2), g),
                        _ => {
                            wish = Wish {
                                up: true,
                                ..forward
                            }
                        }
                    }
                    fold.run(&mut body, (300, fps60), wish, g, built);
                    let turn = Wish {
                        movement: [1.0, 0.4],
                        ..Wish::default()
                    };
                    fold.run(&mut body, (120, fps60), turn, g, built);
                }
            }
        }

        // Built about it: a deck flush with its top and one half way up
        // it, walked over it and into it, jumped and ridden, and brought
        // down on over the flush one; a wall a column off it, walked and
        // ridden along it into the gap.
        let layer = |metres: f64| (metres / BLOCK_M).floor() as i32;
        let [low, flush, half] = [ground_m, top_m, (ground_m + top_m) / 2.0].map(layer);
        let deck = |z: i32| [[-12, -12, z - 1], [12, 12, z - 1]];
        let floor = |z: i32| Some(f64::from(z) * BLOCK_M);
        for (boxes, floor_m) in [
            (deck(flush), floor(flush)),
            (deck(half), floor(half)),
            ([[-6, -2, low - 2], [6, -2, flush + 1]], None),
        ] {
            let cells = laid(g, root, &[boxes]);
            let built = Built {
                cells: &cells,
                ..built
            };
            for dv in [-0.5, 0.5, 1.5] {
                for wish in [
                    forward,
                    Wish {
                        up: true,
                        ..forward
                    },
                ] {
                    let mut body = walker(g, at(-4.5, dv), [1.0, 0.0]);
                    body.stand_at(floor_m, g);
                    fold.run(&mut body, (240, fps60), wish, g, built);
                }
                let mut body = walker(g, at(-8.5, dv), [1.0, 0.0]);
                body.stand_at(floor_m, g);
                assert!(body.take_way("test.horse", &horse, Steering::Steered, 1, g));
                let struck = Wish {
                    struck: [true, false],
                    ..forward
                };
                fold.run(&mut body, (1, fps60), struck, g, built);
                fold.run(&mut body, (300, fps60), forward, g, built);
            }
            if boxes == deck(flush) {
                for paces in [None, Some(&horse)] {
                    let mut body = walker(g, at(0.5, 0.5), [1.0, 0.0]);
                    if let Some(paces) = paces {
                        assert!(body.take_way("test.horse", paces, Steering::Steered, 1, g));
                    }
                    body.stand_at(Some(top_m + 3.0), g);
                    fold.run(&mut body, (120, fps60), Wish::default(), g, built);
                }
            }
        }
        fold.0
    }

    /// A trunk met from outside, walked, ridden, flown or jumped into or
    /// past, or built about, is a wall or a step to a body and nothing
    /// more: no way out moves it. [`MET_FROM_OUTSIDE`] holds every frame
    /// of that, folded.
    #[test]
    fn a_body_meeting_a_trunk_from_outside_goes_as_it_did() {
        let got = met_from_outside();
        assert_eq!(got, MET_FROM_OUTSIDE, "{got:#x}");
    }

    const MET_FROM_OUTSIDE: u64 = 0x436c_80d2_f914_e72c;
}
