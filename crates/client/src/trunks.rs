//! The trunks of the plants near the body, as its collision meets them.
//!
//! A plant stops a body by its trunk alone: its leaves and branches let it
//! through. A trunk is met as a column of cells is: a wall to a body lower
//! than a step under its top, a floor to one over it. It stops a body
//! wherever its plant is drawn, in a volume or out: both ask
//! [`Rooting::grows`].

use std::collections::HashMap;

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
        let sector = match seat {
            Seat::Sector(sector) => Some(sector),
            _ => None,
        };
        let grid = generator.sphere().blocks();
        let middle = |[x, y]: [i32; 2]| [x, y].map(|n| f64::from(n) + 0.5);
        sector.into_iter().flat_map(move |sector| {
            collision::columns(point)
                .map(move |at| column_at(grid, sector, middle(at)))
                .filter_map(|column| self.columns.get(&column))
                .filter(|(_, stand)| {
                    stand.grows(&|sector, column, span_m| cells.covered(sector, column, span_m))
                })
                .map(
                    move |&([bottom_m, top_m], _)| match feet_m + STEP_M >= bottom_m {
                        true => Footing {
                            floor_m: Some(top_m),
                            ceiling_m: None,
                        },
                        false => Footing {
                            floor_m: None,
                            ceiling_m: Some(bottom_m),
                        },
                    },
                )
        })
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

    /// A tree whose trunk rises over a body's head, with open ground a
    /// walker crosses along `u` toward it and past it: its plant, the
    /// trunks about it, and the first column a trunk stands in on the way.
    pub(crate) fn a_tree(generator: &Generator) -> (Plant, Trunks, u16) {
        let flora = Flora::grow(generator);
        let side = generator.sphere().blocks().side();
        let sector = Sector::ALL[4];
        let none = Trunks::default();
        let corners = (8..24).flat_map(|j| (8..24).map(move |i| [i, j].map(|n| n * side / 32)));
        let mut plants = corners.flat_map(|corner| flora.plants(generator, sector, corner, 32));
        plants
            .find_map(|plant| {
                let root = plant.root.column;
                let ground_m = f64::from(plant.root.h + 1) * BLOCK_M;
                let mut trunks = Trunks::default();
                trunks.around(
                    generator,
                    Some(&flora),
                    Seat::Sector(sector),
                    SurfacePoint::center_of(root),
                );
                let tall = trunks.columns.get(&root)?.0[1] > ground_m + 2.5;
                let first = (root.u - 10..=root.u).find(|&u| {
                    (root.v - 1..=root.v + 1)
                        .any(|v| trunks.columns.contains_key(&Column::new(sector, u, v)))
                })?;
                let from =
                    SurfacePoint::new(sector, f64::from(root.u) - 7.5, f64::from(root.v) + 0.5);
                let open = go(generator, from, &none, Going::Walking) > f64::from(root.u) + 2.0;
                (tall && first + 1 >= root.u && open).then_some((plant, trunks, first))
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
}
